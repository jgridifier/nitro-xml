// SPDX-License-Identifier: MIT
/**
 * xlsx comparisons: Stage-16 worksheet, Stage-18 view-IR, Stage-19 engine API.
 */

import { existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import type { Comparison, CorpusResult, Manifest } from '../types.ts';
import { repoRoot, selectCorpus } from '../corpus.ts';
import {
	dumpXlsxSheetOracle,
	dumpXlsxViewIr,
	dumpXlsxEngineOracle,
	dumpXlsxShellOracle,
} from '../oracle/xlsx.ts';
import {
	dumpXlsxWorkbookRust,
	dumpFormatViewIrRust,
	dumpXlsxEngineRust,
	dumpXlsxShellRust,
	ensureExample,
} from '../rust/dump.ts';
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
	walk(join(root, 'fixtures/real/xlsx'));
	walk(join(root, 'fixtures/opc/allowed'));
	walk(join(root, 'fixtures/upstream/xlsx'));
	walk(join(root, 'fixtures/generated/xlsx'));
	return out.filter((p) => XLSX_EXT.test(p)).sort();
}

function dumpOracleSubprocess(file: string): { ok: boolean; json: unknown } {
	const root = repoRoot();
	const script = join(root, 'tools/model-compare/gates/ref/xlsx_sheet_ref.ts');
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
	ensureExample('xlsx_view_ir_dump', 'nitro-xml-xlsx');
	ensureExample('xlsx_engine_dump', 'nitro-xml-xlsx');
	ensureExample('xlsx_shell_dump', 'nitro-xml-xlsx');

	let n = 0;
	let badWorksheet = 0;
	let badViewIr = 0;
	let badEngine = 0;
	let badShell = 0;
	let badAny = 0;
	for (const f of listXlsxFiles(root)) {
		if (!existsSync(f)) continue;
		if (/\b(xl_|cells_1m)/.test(f)) continue;
		n++;
		let fileBad = false;
		const ref = dumpOracleSubprocess(f);
		const got = dumpXlsxWorkbookRust(f);
		if (got.status === 101 || got.status === 124 || (got.status !== null && got.status > 128)) {
			badWorksheet++;
			badViewIr++;
			badEngine++;
			badShell++;
			fileBad = true;
			if (notes.length < 20) notes.push(`panic/timeout/crash rc=${got.status} on ${f}`);
			continue;
		}
		if (!ref.ok || !got.ok) {
			if (!ref.ok && !got.ok) continue;
			badWorksheet++;
			fileBad = true;
			if (notes.length < 20) notes.push(`worksheet accept/reject mismatch on ${f}`);
		} else if (normalizeJson(ref.json) !== normalizeJson(got.json)) {
			badWorksheet++;
			fileBad = true;
			if (notes.length < 20) notes.push(`worksheet output differs on ${f}`);
		}

		try {
			const oracleIr = await dumpXlsxViewIr(f);
			const rustIr = dumpFormatViewIrRust('xlsx', f, '#18');
			if (normalizeJson(oracleIr) !== normalizeJson(rustIr)) {
				badViewIr++;
				fileBad = true;
				if (notes.length < 20) notes.push(`view-IR differs on ${f}`);
			}
		} catch (e) {
			badViewIr++;
			fileBad = true;
			if (notes.length < 20) notes.push(`view-IR error on ${f}: ${e}`);
		}

		try {
			const oracleEng = dumpXlsxEngineOracle(f);
			const rustEng = dumpXlsxEngineRust(f);
			if (!oracleEng.ok || !rustEng.ok) {
				if (!oracleEng.ok && !rustEng.ok) {
					// both reject — accept/reject parity (minimal/encrypted fixtures)
				} else {
					badEngine++;
					fileBad = true;
					if (notes.length < 20) {
						notes.push(`engine API accept/reject mismatch on ${f}`);
					}
				}
			} else if (normalizeJson(oracleEng.json) !== normalizeJson(rustEng.json)) {
				badEngine++;
				fileBad = true;
				if (notes.length < 20) notes.push(`engine API differs on ${f}`);
			}
		} catch (e) {
			badEngine++;
			fileBad = true;
			if (notes.length < 20) notes.push(`engine API error on ${f}: ${e}`);
		}

		try {
			const oracleShell = dumpXlsxShellOracle(f);
			const rustShell = dumpXlsxShellRust(f);
			if (!oracleShell.ok || !rustShell.ok) {
				if (!oracleShell.ok && !rustShell.ok) {
					// both reject
				} else {
					badShell++;
					fileBad = true;
					if (notes.length < 20) {
						notes.push(`shell accept/reject mismatch on ${f}`);
					}
				}
			} else if (normalizeJson(oracleShell.json) !== normalizeJson(rustShell.json)) {
				badShell++;
				fileBad = true;
				if (notes.length < 20) notes.push(`shell differs on ${f}`);
			}
		} catch (e) {
			badShell++;
			fileBad = true;
			if (notes.length < 20) notes.push(`shell error on ${f}: ${e}`);
		}

		if (fileBad) badAny++;
	}
	notes.push(`worksheet differential: ${n - badWorksheet}/${n} files match upstream`);
	notes.push(`view-IR differential: ${n - badViewIr}/${n} files match upstream`);
	notes.push(`engine API differential: ${n - badEngine}/${n} files match upstream`);
	notes.push(`shell differential: ${n - badShell}/${n} files match upstream`);
	return { compared: n, mismatched: badAny, notes };
}

export const xlsxComparison: Comparison = {
	id: 'xlsx',
	label: 'xlsx worksheet + view-IR + engine API + shell (#16, #18, #19, #29)',
	ready: true,
	todo: undefined,
	async oracle(fixturePath: string) {
		return dumpXlsxSheetOracle(fixturePath);
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
