// SPDX-License-Identifier: MIT
/**
 * docx Stage-4 styles/numbering/theme/settings comparison folded from
 * tools/model-compare/gates/issue-20.sh (#20).
 *
 * IR: paragraph/character style catalogs, numbering catalog, list labels,
 * theme (+ clrSchemeMapping), settings, and resolveParagraphFormatting per
 * body paragraph. Full LayoutResult view IR remains for a later painter gate.
 */

import { existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import type { Comparison, CorpusResult, Manifest } from '../types.ts';
import { repoRoot, selectCorpus } from '../corpus.ts';
import { dumpDocxStylesOracle } from '../oracle/docx.ts';
import { dumpDocxStylesRust, ensureExample } from '../rust/dump.ts';
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
	// Regenerate minimal.* packages when absent (same as issue-20 gate).
	spawnSync('python3', ['tools/model-compare/gates/make_opc_fixtures.py'], {
		cwd: root,
		encoding: 'utf8',
	});

	let n = 0;
	let bad = 0;
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
	}
	notes.push(`styles differential: ${n - bad}/${n} files match upstream`);
	return { compared: n, mismatched: bad, notes };
}

export const docxComparison: Comparison = {
	id: 'docx',
	label: 'docx styles/numbering/theme/settings (issue #20)',
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
