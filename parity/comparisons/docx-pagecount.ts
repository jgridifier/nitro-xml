// SPDX-License-Identifier: MIT
/**
 * docx page-count differential (#22): Chromium canvas measurer vs Rust
 * `docx_pagecount_dump` fed by the same formulas.
 */

import { existsSync, readdirSync, mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { Comparison, CorpusResult, Manifest } from '../types.ts';
import { repoRoot } from '../corpus.ts';
import { upstreamDir } from '../paths.ts';
import { dumpDocxPagecountRust, ensureExample } from '../rust/dump.ts';

const DOCX_EXT = /\.(docx|docm|dotx|dotm)$/i;

function listDocxFiles(root: string): string[] {
	const out: string[] = [];
	const walk = (dir: string) => {
		if (!existsSync(dir)) return;
		for (const name of readdirSync(dir, { withFileTypes: true })) {
			const p = join(dir, name.name);
			if (name.isDirectory()) walk(p);
			else if (DOCX_EXT.test(name.name)) out.push(p);
		}
	};
	walk(join(root, 'fixtures/real/docx'));
	return out.filter((p) => DOCX_EXT.test(p)).sort();
}

type PendingProbe = {
	key: string;
	family: string;
	sizePx: number;
	bold: boolean;
	italic: boolean;
	kerning: string | null;
	ligatures: string | null;
	smallCaps: boolean;
	text: string;
	kind: 'width' | 'lineHeight' | 'ascent';
};

type CacheEntry = { width?: number; lineHeight?: number; ascent?: number };

function oracleEntry(root: string): string {
	const up = upstreamDir(root);
	const parse = join(up, 'src/core/docx/parse.ts');
	const layout = join(up, 'src/core/docx/layout/layout.ts');
	const canvas = join(up, 'src/ui/src/docx/canvas-measurer.ts');
	return `
import { loadDocx } from ${JSON.stringify(parse)};
import { layoutDocumentModel } from ${JSON.stringify(layout)};
import { createCanvasMeasurer } from ${JSON.stringify(canvas)};

const fontOf = (item) => ({
	family: item.family,
	sizePx: item.sizePx,
	bold: !!item.bold,
	italic: !!item.italic,
	kerning: item.kerning ?? undefined,
	ligatures: item.ligatures ?? undefined,
	smallCaps: !!item.smallCaps,
});

export async function pageCount(b64) {
	await document.fonts.ready;
	const bin = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
	const loaded = await loadDocx(bin);
	const result = layoutDocumentModel(loaded.model, createCanvasMeasurer());
	return result.pages.length;
}

export async function measurePending(items) {
	await document.fonts.ready;
	const measurer = createCanvasMeasurer();
	return items.map((item) => {
		const font = fontOf(item);
		const out = {};
		if (item.kind === 'width') out.width = measurer.widthOf(item.text ?? '', font);
		if (item.kind === 'lineHeight') out.lineHeight = measurer.lineHeightOf(font);
		if (item.kind === 'ascent') out.ascent = measurer.ascentOf(font);
		return out;
	});
}
`;
}

async function loadPlaywright(root: string) {
	const entry = join(root, 'parity/visual/node_modules/playwright/index.js');
	if (!existsSync(entry)) {
		throw new Error(
			'playwright missing; run: cd parity/visual && bun install && bunx playwright install chromium',
		);
	}
	return import(entry);
}

function buildOracleBundle(root: string, dir: string): string {
	const entry = join(dir, 'oracle-entry.ts');
	const outfile = join(dir, 'bundle.js');
	writeFileSync(entry, oracleEntry(root));
	const bun = process.env.BUN ?? 'bun';
	const build = spawnSync(bun, ['build', entry, '--target=browser', '--outfile', outfile], {
		encoding: 'utf8',
		env: { ...process.env, PATH: `${process.env.HOME}/.bun/bin:${process.env.PATH}` },
	});
	if (build.status !== 0) {
		throw new Error(`oracle bundle failed: ${build.stderr || build.stdout}`);
	}
	return readFileSync(outfile, 'utf8');
}

function toB64(path: string): string {
	return readFileSync(path).toString('base64');
}

async function runDocxPagecountCorpus(): Promise<CorpusResult> {
	const root = repoRoot();
	const notes: string[] = [];
	ensureExample('docx_pagecount_dump', 'nitro-xml-docx');
	const files = listDocxFiles(root);
	const dir = mkdtempSync(join(tmpdir(), 'docx-pagecount-'));
	let browser: { close: () => Promise<void> } | undefined;
	let n = 0;
	let bad = 0;
	try {
		const bundle = buildOracleBundle(root, dir);
		const pw = await loadPlaywright(root);
		browser = await pw.chromium.launch({ headless: true });
		const page = await browser.newPage();
		page.setDefaultTimeout(180_000);
		await page.route('https://pagecount.local/bundle.js', (route) => {
			route.fulfill({ status: 200, contentType: 'text/javascript', body: bundle });
		});
		await page.setContent(
			`<!doctype html><html><body><script type="module">
import { pageCount, measurePending } from 'https://pagecount.local/bundle.js';
window.__pageCount = pageCount;
window.__measurePending = measurePending;
window.__boot = true;
</script></body></html>`,
			{ waitUntil: 'networkidle' },
		);
		await page.waitForFunction('window.__boot === true');
		await page.evaluate(async () => {
			await document.fonts.ready;
		});

		for (const file of files) {
			n++;
			let oracleCount: number;
			try {
				oracleCount = await page.evaluate(async (b64) => {
					await document.fonts.ready;
					return globalThis.__pageCount(b64);
				}, toB64(file));
			} catch (err) {
				bad++;
				if (notes.length < 20) notes.push(`oracle failed on ${file}: ${err}`);
				continue;
			}
			const cachePath = join(dir, `cache-${n}.json`);
			const cache: Record<string, CacheEntry> = {};
			let rustCount: number | undefined;
			let failed = false;
			for (let pass = 0; pass < 24; pass++) {
				writeFileSync(cachePath, JSON.stringify(cache));
				const got = dumpDocxPagecountRust(file, cachePath);
				if (got.status === 101 || got.status === 124 || (got.status !== null && got.status > 128)) {
					failed = true;
					if (notes.length < 20) notes.push(`panic/timeout/crash rc=${got.status} on ${file}`);
					break;
				}
				if (!got.ok) {
					failed = true;
					if (notes.length < 20) notes.push(`rust dump failed on ${file}: ${JSON.stringify(got.json)}`);
					break;
				}
				const body = got.json as { done?: boolean; pageCount?: number; pending?: PendingProbe[] };
				if (body.done) {
					rustCount = body.pageCount;
					break;
				}
				const pending = body.pending ?? [];
				if (pending.length === 0) {
					failed = true;
					if (notes.length < 20) notes.push(`rust page-count stalled on ${file}`);
					break;
				}
				let measured: CacheEntry[];
				try {
					measured = await page.evaluate(async (items) => {
						await document.fonts.ready;
						return globalThis.__measurePending(items);
					}, pending);
				} catch (err) {
					failed = true;
					if (notes.length < 20) notes.push(`measure failed on ${file}: ${err}`);
					break;
				}
				pending.forEach((probe, index) => {
					const prev = cache[probe.key] ?? {};
					const sample = measured[index] ?? {};
					cache[probe.key] = { ...prev, ...sample };
				});
			}
			if (failed) {
				bad++;
				continue;
			}
			if (rustCount === undefined) {
				bad++;
				if (notes.length < 20) notes.push(`rust page-count did not converge on ${file}`);
				continue;
			}
			if (rustCount !== oracleCount) {
				bad++;
				if (notes.length < 20) {
					notes.push(`pageCount rust=${rustCount} oracle=${oracleCount} on ${file}`);
				}
			}
		}
	} catch (err) {
		bad += Math.max(1, files.length - n);
		notes.push(`page-count harness failed: ${err}`);
		if (n === 0) n = files.length;
	} finally {
		if (browser) await browser.close();
		rmSync(dir, { recursive: true, force: true });
	}
	notes.push(`docx page-count differential: ${n - bad}/${n} files match upstream`);
	return { compared: n, mismatched: bad, notes };
}

export const docxPagecountComparison: Comparison = {
	id: 'docx-pagecount',
	label: 'docx page count, real fonts (#22)',
	ready: true,
	async oracle(fixturePath: string) {
		return { fixture: fixturePath };
	},
	async rust(fixturePath: string) {
		return { fixture: fixturePath };
	},
	selectFixtures(_manifest: Manifest) {
		return listDocxFiles(repoRoot()) as unknown as Manifest['fixtures'];
	},
	runCorpus: runDocxPagecountCorpus,
};
