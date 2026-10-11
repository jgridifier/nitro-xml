// SPDX-License-Identifier: MIT
/**
 * Bidi visual checks (#44): reuse the #45 headless host/runner for ready
 * xlsx view-IR cases (Arabic/Hebrew fixtures).
 */

import { mkdirSync, writeFileSync, existsSync, rmSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { startVisualServer } from '../../parity/visual/server.mjs';
import { comparePngBuffers } from '../../parity/visual/compare.mjs';
import { THRESHOLDS } from '../../parity/visual/thresholds.mjs';
import { readyCases } from './registry.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const VISUAL = join(ROOT, 'parity/visual');
const OUT = join(ROOT, 'tests/bidi/out');
const SHOTS = join(OUT, 'shots');
const DIFFS = join(OUT, 'diffs');
const PAYLOADS = join(VISUAL, 'out/payloads');

async function loadChromium() {
	const mod = await import(pathToFileURL(join(VISUAL, 'node_modules/playwright/index.mjs')).href);
	return mod.chromium;
}

function ensureExample(name, crate) {
	const bin = join(ROOT, 'rs/target/release/examples', name);
	if (existsSync(bin)) return bin;
	const r = spawnSync(
		'cargo',
		['build', '--release', '-q', '-p', crate, '--example', name],
		{ cwd: join(ROOT, 'rs'), encoding: 'utf8' },
	);
	if (r.status !== 0) throw new Error(`build ${name}: ${r.stderr || r.stdout}`);
	return bin;
}

async function prepareViewIr(c) {
	mkdirSync(PAYLOADS, { recursive: true });
	const file = join(ROOT, c.fixture);
	if (c.format === 'docx') {
		const { dumpDocxViewIr } = await import('../../parity/oracle/docx.ts');
		const tsIr = await dumpDocxViewIr(file);
		writeFileSync(join(PAYLOADS, `${c.id}-ts.json`), JSON.stringify(tsIr));
		const bin = ensureExample('docx_layout_dump', 'nitro-xml-docx');
		const rust = spawnSync(bin, [file], {
			encoding: 'utf8',
			maxBuffer: 64 * 1024 * 1024,
		});
		if (rust.status !== 0) throw new Error(`docx layout rust: ${rust.stderr || rust.stdout}`);
		writeFileSync(join(PAYLOADS, `${c.id}-rust.json`), JSON.stringify(JSON.parse(rust.stdout)));
		return;
	}
	const { dumpXlsxViewIr } = await import('../../parity/oracle/xlsx.ts');
	const tsIr = await dumpXlsxViewIr(file);
	writeFileSync(join(PAYLOADS, `${c.id}-ts.json`), JSON.stringify(tsIr));
	const bin = ensureExample('xlsx_view_ir_dump', 'nitro-xml-xlsx');
	const rust = spawnSync(bin, [file], {
		encoding: 'utf8',
		maxBuffer: 64 * 1024 * 1024,
	});
	if (rust.status !== 0) throw new Error(`view-IR rust: ${rust.stderr || rust.stdout}`);
	writeFileSync(join(PAYLOADS, `${c.id}-rust.json`), JSON.stringify(JSON.parse(rust.stdout)));
}

function hostUrl(base, c, side) {
	const q = new URLSearchParams({
		kind: c.format === 'docx' ? 'docx-layout-ir' : 'xlsx-view-ir',
		side,
		id: c.id,
		width: String(c.width || 800),
		height: String(c.height || 240),
	});
	return `${base}/pages/host.html?${q}`;
}

async function waitReady(page, timeoutMs = 30_000) {
	await page.waitForFunction(() => {
		const el = document.getElementById('stage');
		return el && (el.dataset.ready === '1' || el.dataset.ready === 'err');
	}, null, { timeout: timeoutMs });
	const ready = await page.evaluate(() => document.getElementById('stage')?.dataset.ready);
	if (ready !== '1') {
		const body = await page.evaluate(() => document.getElementById('stage')?.textContent?.slice(0, 400));
		throw new Error(`host not ready: ${body || ''}`);
	}
}

async function screenshotSide(browser, base, c, side, dpr) {
	const context = await browser.newContext({
		viewport: { width: c.width || 800, height: c.height || 240 },
		deviceScaleFactor: dpr,
	});
	const page = await context.newPage();
	await page.goto(hostUrl(base, c, side), { waitUntil: 'networkidle' });
	await waitReady(page);
	const buf = await page.locator('#stage').screenshot({ type: 'png' });
	await context.close();
	return buf;
}

/**
 * @returns {Promise<{ ok: boolean, rows: object[], summary: string }>}
 */
export async function runVisualChecks() {
	const cases = readyCases().filter(
		(c) => c.visual && (c.format === 'xlsx' || c.format === 'docx'),
	);
	mkdirSync(SHOTS, { recursive: true });
	mkdirSync(DIFFS, { recursive: true });
	if (existsSync(join(OUT, 'summary.txt'))) rmSync(join(OUT, 'summary.txt'));

	for (const c of cases) {
		await prepareViewIr(c);
	}

	const chromium = await loadChromium();
	const server = await startVisualServer();
	const browser = await chromium.launch({ headless: true });
	const rows = [];
	let failed = 0;

	try {
		for (const c of cases) {
			for (const dpr of [1, 2]) {
				const ts = await screenshotSide(browser, server.url, c, 'ts', dpr);
				const rust = await screenshotSide(browser, server.url, c, 'rust', dpr);
				writeFileSync(join(SHOTS, `${c.id}-ts-dpr${dpr}.png`), ts);
				writeFileSync(join(SHOTS, `${c.id}-rust-dpr${dpr}.png`), rust);
				const cmp = comparePngBuffers(ts, rust, {
					format: c.format,
					diffPath: join(DIFFS, `${c.id}-ts-vs-rust-dpr${dpr}.png`),
				});
				if (!cmp.ok) failed++;
				rows.push({
					case: c.id,
					mode: 'ts-vs-rust',
					dpr,
					ok: cmp.ok,
					ratio: cmp.ratio,
					mismatched: cmp.mismatched,
					total: cmp.total,
					threshold: THRESHOLDS[c.format],
					reason: cmp.reason,
				});
			}
		}
	} finally {
		await browser.close();
		await server.close();
	}

	const deferred = readyCases()
		.filter((c) => !c.visual)
		.map((c) => `${c.id}: ${c.todo || 'no visual'}`);
	const summaryLines = [
		`bidi visual: ${failed === 0 ? 'PASS' : 'FAIL'} (${rows.filter((r) => r.ok).length}/${rows.length})`,
		...rows.map((r) => {
			const tag = r.ok ? 'OK' : 'FAIL';
			return `  ${tag} ${r.case} ${r.mode} dpr=${r.dpr} mismatch=${(r.ratio * 100).toFixed(4)}% (limit ${(r.threshold * 100).toFixed(2)}%)${r.reason ? ` ${r.reason}` : ''}`;
		}),
		...(deferred.length ? ['deferred visual:', ...deferred.map((d) => `  - ${d}`)] : []),
	];
	const summary = summaryLines.join('\n');
	writeFileSync(join(OUT, 'summary.txt'), summary + '\n');
	writeFileSync(join(OUT, 'results.json'), JSON.stringify({ failed, rows }, null, 2));
	return { ok: failed === 0, rows, summary };
}

if (import.meta.main) {
	const { ok, summary } = await runVisualChecks();
	console.log(summary);
	process.exit(ok ? 0 : 1);
}
