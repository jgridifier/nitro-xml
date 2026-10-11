// SPDX-License-Identifier: MIT
/**
 * xlsx Stage-3 workbook-slice comparison folded from
 * tools/model-compare/gates/issue-15.sh (#131).
 *
 * IR: sheets list, defined names (incl. Print_Area lift), date1904, theme,
 * styles / namedStyles / xfMap / dxfs / palette, shared strings, warnings.
 * Full CellItem/CellView view IR remains TODO once the grid path lands (#16/#18).
 */

import { existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import type { Comparison, CorpusResult, Manifest } from '../types.ts';
import { repoRoot, selectCorpus } from '../corpus.ts';
import { dumpXlsxWorkbookOracle } from '../oracle/xlsx.ts';
import { dumpXlsxWorkbookRust, ensureExample } from '../rust/dump.ts';
import { normalizeJson } from '../normalize.ts';
import { upstreamDir } from '../paths.ts';

const XLSX_EXT = /\.(xlsx|xlsm|xltx|xltm)$/i;

function listXlsxFiles(root: string): string[] {
	const out: string[] = [];
	const walk = (dir: string) => {
		if (!existsSync(dir)) return;
		for (const name of readdirSync(dir, { withFileTypes: true })) {
			const p = join(dir, name.name);
			if (name.isDirectory()) walk(p);
			else if (XLSX_EXT.test(name.name)) out.push(p);
		}
	};
	// Same corpus as issue-15 (missing on-demand L/XL files are skipped below).
	walk(join(root, 'fixtures/real/xlsx'));
	walk(join(root, 'fixtures/opc/allowed'));
	walk(join(root, 'fixtures/upstream/xlsx'));
	walk(join(root, 'fixtures/generated/xlsx'));
	return out.filter((p) => XLSX_EXT.test(p)).sort();
}

function dumpOracleSubprocess(file: string): { ok: boolean; json: unknown } {
	const root = repoRoot();
	const script = join(root, 'tools/model-compare/gates/ref/xlsx_ref.ts');
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

async function runXlsxCorpus(): Promise<CorpusResult> {
	const root = repoRoot();
	const notes: string[] = [];
	ensureExample('xlsx_dump', 'nitro-xml-xlsx');

	let n = 0;
	let bad = 0;
	for (const f of listXlsxFiles(root)) {
		if (!existsSync(f)) continue;
		// Skip on-demand XL names that may be absent (fixtures/verify --skip-on-demand).
		if (/\b(xl_|cells_1m)/.test(f)) continue;
		n++;
		const ref = dumpOracleSubprocess(f);
		const got = dumpXlsxWorkbookRust(f);
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
	notes.push(`workbook differential: ${n - bad}/${n} files match upstream`);
	return { compared: n, mismatched: bad, notes };
}

export const xlsxComparison: Comparison = {
	id: 'xlsx',
	label: 'xlsx workbook slice (issue #15)',
	ready: true,
	todo: undefined,
	async oracle(fixturePath: string) {
		return dumpXlsxWorkbookOracle(fixturePath);
	},
	async rust(fixturePath: string) {
		const { json } = dumpXlsxWorkbookRust(fixturePath);
		return json;
	},
	selectFixtures(manifest: Manifest) {
		return selectCorpus(manifest, {
			format: 'xlsx',
			skipOnDemand: true,
			tiers: ['S', 'M'],
		});
	},
	runCorpus: runXlsxCorpus,
};
