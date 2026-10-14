// SPDX-License-Identifier: MIT
/**
 * docx Stage-4 styles + body model comparison (#20 + #21).
 *
 * IR: style catalogs, numbering, theme, settings, resolved paragraphs, list
 * labels, plus readPackage body fields (blocks, sections, notes, warnings,
 * mediaPartNames). Chart deep fields are stripped until #28.
 */

import { existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import type { Comparison, CorpusResult, Manifest } from '../types.ts';
import { repoRoot, selectCorpus } from '../corpus.ts';
import { dumpDocxStylesOracle, dumpDocxShellOracle } from '../oracle/docx.ts';
import { dumpDocxStylesRust, dumpViewerShellRust, ensureExample } from '../rust/dump.ts';
import { diffIr } from '../diff.ts';
import { normalizeJson } from '../normalize.ts';
import { upstreamDir } from '../paths.ts';

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
	// Same corpus as issue-20 (real Word packages + minimal OPC allow-list).
	walk(join(root, 'fixtures/real/docx'));
	for (const name of ['minimal.docx', 'minimal.docm', 'minimal.dotx', 'minimal.dotm']) {
		const p = join(root, 'fixtures/opc/allowed', name);
		if (existsSync(p)) out.push(p);
	}
	return out.filter((p) => DOCX_EXT.test(p)).sort();
}

function dumpOracleSubprocess(file: string): { ok: boolean; json: unknown } {
	const root = repoRoot();
	const script = join(root, 'tools/model-compare/gates/ref/docx_styles_ref.ts');
	const r = spawnSync('bun', [script, file], {
		encoding: 'utf8',
		env: { ...process.env, UPSTREAM_DIR: upstreamDir(root) },
		maxBuffer: 32 * 1024 * 1024,
	});
	let json: unknown = null;
	try {
		json = JSON.parse((r.stdout || '').trim() || 'null');
	} catch {
		json = { error: 'parse' };
	}
	return { ok: r.status === 0, json };
}

async function runDocxCorpus(): Promise<CorpusResult> {
	const root = repoRoot();
	const notes: string[] = [];
	ensureExample('docx_styles_dump', 'nitro-xml-docx');
	ensureExample('viewer_shell_dump', 'nitro-xml-wasm');
	// Regenerate minimal.* packages when absent (same as issue-20 gate).
	spawnSync('python3', ['tools/model-compare/gates/make_opc_fixtures.py'], {
		cwd: root,
		encoding: 'utf8',
	});

	let n = 0;
	let bad = 0;
	let badShell = 0;
	let shellN = 0;
	let negativeChecked = false;
	for (const f of listDocxFiles(root)) {
		if (!existsSync(f)) continue;
		n++;
		const ref = dumpOracleSubprocess(f);
		const got = dumpDocxStylesRust(f);
		if (got.status === 101 || got.status === 124 || (got.status !== null && got.status > 128)) {
			bad++;
			if (notes.length < 20) notes.push(`panic/timeout/crash rc=${got.status} on ${f}`);
			continue;
		}
		if (!ref.ok || !got.ok) {
			if (!ref.ok && !got.ok) continue;
			bad++;
			if (notes.length < 20) notes.push(`accept/reject mismatch on ${f}`);
			continue;
		}
		if (normalizeJson(ref.json) !== normalizeJson(got.json)) {
			bad++;
			if (notes.length < 20) notes.push(`output differs on ${f}`);
		}

		shellN++;
		const oracleShell = dumpDocxShellOracle(f);
		const rustShell = dumpViewerShellRust(f);
		if (rustShell.status === 101 || rustShell.status === 124 || (rustShell.status !== null && rustShell.status > 128)) {
			badShell++;
			if (notes.length < 20) notes.push(`shell panic/timeout/crash rc=${rustShell.status} on ${f}`);
		} else if (oracleShell.ok !== rustShell.ok) {
			badShell++;
			if (notes.length < 20) notes.push(`shell accept/reject mismatch on ${f}`);
		} else if (!oracleShell.ok) {
			// both reject
		} else if (normalizeJson(oracleShell.json) !== normalizeJson(rustShell.json)) {
			badShell++;
			if (notes.length < 20) {
				const path = diffIr(oracleShell.json, rustShell.json)[0]?.path ?? '';
				notes.push(`shell differs on ${f} ${path}`);
			}
		} else if (!negativeChecked) {
			negativeChecked = true;
			const neg = dumpViewerShellRust(f, true);
			if (!neg.ok || normalizeJson(oracleShell.json) === normalizeJson(neg.json)) {
				badShell++;
				notes.push('docx shell negative-control unexpectedly passed');
			} else {
				notes.push('docx shell negative-control FAIL as expected');
			}
		}
	}
	if (!negativeChecked) {
		badShell++;
		notes.push('docx shell negative-control did not run');
	}
	notes.push(`docx differential: ${n - bad}/${n} files match upstream`);
	notes.push(`docx shell differential: ${shellN - badShell}/${shellN} files match upstream`);
	return { compared: n, mismatched: bad + badShell, notes };
}

export const docxComparison: Comparison = {
	id: 'docx',
	label: 'docx styles + body model + shell (#20, #21, #29)',
	ready: true,
	todo: undefined,
	async oracle(fixturePath: string) {
		return dumpDocxStylesOracle(fixturePath);
	},
	async rust(fixturePath: string) {
		const { json } = dumpDocxStylesRust(fixturePath);
		return json;
	},
	selectFixtures(manifest: Manifest) {
		return selectCorpus(manifest, {
			format: 'docx',
			skipOnDemand: true,
			tiers: ['S', 'M'],
		});
	},
	runCorpus: runDocxCorpus,
};
