// SPDX-License-Identifier: MIT
/**
 * pptx Stage-5 presentation/slide/master/theme + placeholder inheritance
 * comparison for issue #24. IR matches tools/model-compare/gates/ref/pptx_ref.ts
 * / pptx_dump (ported element kinds only).
 */

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { basename, join } from 'node:path';
import { spawnSync } from 'node:child_process';
import type { Comparison, CorpusResult, Manifest } from '../types.ts';
import { loadManifest, repoRoot, selectCorpus } from '../corpus.ts';
import { dumpPptxModelRust, dumpFormatViewIrRust, dumpViewerShellRust, ensureExample } from '../rust/dump.ts';
import { diffIr, filterKnownDiffs } from '../diff.ts';
import { normalizeJson } from '../normalize.ts';
import { dumpPptxViewIr, dumpPptxShellOracle } from '../oracle/pptx.ts';
import { upstreamDir } from '../paths.ts';

const PPTX_EXT = /\.(pptx|pptm|potx|potm)$/i;

function listPptxFiles(root: string): string[] {
	const out: string[] = [];
	const walk = (dir: string) => {
		if (!existsSync(dir)) return;
		for (const name of readdirSync(dir, { withFileTypes: true })) {
			const p = join(dir, name.name);
			if (name.isDirectory()) walk(p);
			else if (PPTX_EXT.test(name.name)) out.push(p);
		}
	};
	walk(join(root, 'fixtures/real/pptx'));
	walk(join(root, 'fixtures/opc/allowed'));
	walk(join(root, 'fixtures/upstream/pptx'));
	walk(join(root, 'fixtures/generated/pptx'));
	return out.filter((p) => PPTX_EXT.test(p)).sort();
}

function dumpOracleSubprocess(file: string): { ok: boolean; json: unknown } {
	const root = repoRoot();
	const script = join(root, 'tools/model-compare/gates/ref/pptx_ref.ts');
	const r = spawnSync('bun', [script, file], {
		encoding: 'utf8',
		env: { ...process.env, UPSTREAM_DIR: upstreamDir(root) },
		maxBuffer: 64 * 1024 * 1024,
	});
	let json: unknown = null;
	try {
		json = JSON.parse((r.stdout || '').trim() || 'null');
	} catch {
		json = { error: 'parse' };
	}
	return { ok: r.status === 0, json };
}

function fixtureIdForPath(manifest: Manifest, absPath: string, root: string): string {
	const rel = absPath.startsWith(root) ? absPath.slice(root.length + 1) : absPath;
	const row = manifest.fixtures.find((f) => f.path === rel);
	return row?.id ?? basename(absPath);
}

function loadKnownDiffs(root: string) {
	const path = join(root, 'parity/known-diffs.json');
	if (!existsSync(path)) return { entries: [] };
	return JSON.parse(readFileSync(path, 'utf8')) as {
		entries: import('../types.ts').KnownDiffEntry[];
	};
}

async function runPptxCorpus(): Promise<CorpusResult> {
	const root = repoRoot();
	const manifest = loadManifest(root);
	const known = loadKnownDiffs(root);
	const notes: string[] = [];
	ensureExample('pptx_dump', 'nitro-xml-pptx');
	ensureExample('pptx_view_ir_dump', 'nitro-xml-pptx');
	ensureExample('viewer_shell_dump', 'nitro-xml-wasm');

	let n = 0;
	let matched = 0;
	let allowListedFixtures = 0;
	let unexpected = 0;
	let badViewIr = 0;
	let badShell = 0;
	let shellN = 0;
	let negativeChecked = false;
	for (const f of listPptxFiles(root)) {
		if (!existsSync(f)) continue;
		if (/\b(xl_|slides_200)/.test(f)) continue;
		n++;
		const fixtureId = fixtureIdForPath(manifest, f, root);
		const ref = dumpOracleSubprocess(f);
		const got = dumpPptxModelRust(f);
		if (got.status === 101 || got.status === 124 || (got.status !== null && got.status > 128)) {
			unexpected++;
			if (notes.length < 20) notes.push(`panic/timeout/crash rc=${got.status} on ${f}`);
			continue;
		}
		if (!ref.ok || !got.ok) {
			if (!ref.ok && !got.ok) continue;
			unexpected++;
			if (notes.length < 20) notes.push(`accept/reject mismatch on ${f}`);
			continue;
		}
		const diffs = diffIr(ref.json, got.json);

		shellN++;
		const oracleShell = dumpPptxShellOracle(f);
		const rustShell = dumpViewerShellRust(f);
		if (rustShell.status === 101 || rustShell.status === 124 || (rustShell.status !== null && rustShell.status > 128)) {
			badShell++;
			if (notes.length < 20) notes.push(`shell panic/timeout/crash rc=${rustShell.status} on ${fixtureId}`);
		} else if (!oracleShell.ok || !rustShell.ok) {
			if (oracleShell.ok !== rustShell.ok) {
				badShell++;
				if (notes.length < 20) notes.push(`shell accept/reject mismatch on ${fixtureId}`);
			}
		} else if (normalizeJson(oracleShell.json) !== normalizeJson(rustShell.json)) {
			badShell++;
			if (notes.length < 20) {
				const path = diffIr(oracleShell.json, rustShell.json)[0]?.path ?? '';
				notes.push(`shell differs on ${fixtureId} ${path}`);
			}
		} else if (!negativeChecked) {
			negativeChecked = true;
			const neg = dumpViewerShellRust(f, true);
			if (!neg.ok || normalizeJson(oracleShell.json) === normalizeJson(neg.json)) {
				badShell++;
				notes.push('pptx shell negative-control unexpectedly passed');
			} else {
				notes.push('pptx shell negative-control FAIL as expected');
			}
		}

		if (diffs.length === 0) {
			matched++;
			continue;
		}
		const { unexpected: bad } = filterKnownDiffs(diffs, 'pptx', fixtureId, known.entries);
		if (bad.length === 0) {
			allowListedFixtures++;
		} else {
			unexpected++;
			if (notes.length < 20) {
				notes.push(`unexpected diff on ${fixtureId} (${bad.length} path(s))`);
			}
		}

		try {
			const oracleIr = await dumpPptxViewIr(f);
			const rustIr = dumpFormatViewIrRust('pptx', f, '#27');
			if (normalizeJson(oracleIr) !== normalizeJson(rustIr)) {
				badViewIr++;
				unexpected++;
				if (notes.length < 20) notes.push(`view-IR differs on ${fixtureId}`);
			}
		} catch (e) {
			badViewIr++;
			unexpected++;
			if (notes.length < 20) notes.push(`view-IR error on ${fixtureId}: ${e}`);
		}

	}
	if (!negativeChecked) {
		badShell++;
		notes.push('pptx shell negative-control did not run');
	}
	notes.push(
		`pptx presentation differential: ${matched}/${n} exact match; ${allowListedFixtures} allow-listed; ${unexpected} unexpected`,
	);
	notes.push(`pptx view-IR differential: ${n - badViewIr}/${n} files match upstream`);
	notes.push(`pptx shell differential: ${shellN - badShell}/${shellN} files match upstream`);
	return { compared: n, mismatched: unexpected + badShell, notes };
}

export const pptxComparison: Comparison = {
	id: 'pptx',
	label: 'pptx presentation + view-IR + shell (#24, #27, #29)',
	ready: true,
	todo: undefined,
	async oracle(fixturePath: string) {
		const ref = dumpOracleSubprocess(fixturePath);
		return ref.json;
	},
	async rust(fixturePath: string) {
		const { json } = dumpPptxModelRust(fixturePath);
		return json;
	},
	selectFixtures(manifest: Manifest) {
		return selectCorpus(manifest, {
			format: 'pptx',
			skipOnDemand: true,
			tiers: ['S', 'M'],
		});
	},
	runCorpus: runPptxCorpus,
};
