// SPDX-License-Identifier: MIT
/**
 * Selection shell checks (#43): Rust `shellAction(select/copy)` vs TS
 * `copyRange` / selectionRef for ready xlsx cases (no Playwright).
 */

import { readFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { readyCases } from './registry.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');

function upstreamDir() {
	return process.env.UPSTREAM_DIR || join(ROOT, 'upstream');
}

/** Normalize Excel TSV (CRLF + trailing newline) and Rust LF TSV for compare. */
export function normalizeCopy(text) {
	return String(text ?? '')
		.replace(/\r\n/g, '\n')
		.replace(/\n$/, '');
}

function ensureWasm() {
	const glue = join(ROOT, 'packages/nitro-xml/pkg/nitro_xml_wasm.js');
	const wasm = join(ROOT, 'packages/nitro-xml/pkg/nitro_xml_wasm_bg.wasm');
	if (existsSync(glue) && existsSync(wasm)) return;
	const r = spawnSync('bash', [join(ROOT, 'scripts/build-wasm.sh')], {
		cwd: ROOT,
		encoding: 'utf8',
		maxBuffer: 32 * 1024 * 1024,
	});
	if (r.status !== 0) throw new Error(`build-wasm: ${r.stderr || r.stdout}`);
}

async function rustShell(fixtureRel, actions) {
	ensureWasm();
	const { default: init, Handle } = await import(
		join(ROOT, 'packages/nitro-xml/pkg/nitro_xml_wasm.js')
	);
	await init();
	const bytes = new Uint8Array(readFileSync(join(ROOT, fixtureRel)));
	const h = new Handle(bytes);
	h.openDocument(fixtureRel.split('/').pop() || 'doc.xlsx');
	let selection = '';
	let copy = '';
	for (const a of actions) {
		if (a.op === 'select-ref') {
			const r = JSON.parse(h.shellAction('select', a.ref || 'A1'));
			selection = r.ui?.selection ?? a.ref;
		} else if (a.op === 'copy') {
			const r = JSON.parse(h.shellAction('copy', ''));
			copy = r.c ?? r.copyText ?? '';
			selection = r.ui?.selection ?? selection;
		}
	}
	h.free();
	return { selection, copy };
}

async function tsShell(fixtureRel, actions) {
	const up = upstreamDir();
	const { loadXlsx } = await import(`${up}/src/core/xlsx/read/load.ts`);
	const { copyRange } = await import(`${up}/src/core/xlsx/edit/clipboard.ts`);
	const { parseRange } = await import(`${up}/src/core/xlsx/address.ts`);
	const { selectionRef } = await import(`${up}/src/core/xlsx/ui/selection.ts`);
	const bytes = new Uint8Array(readFileSync(join(ROOT, fixtureRel)));
	const workbook = await loadXlsx(bytes);
	const sheet = workbook.activeSheet ?? 0;
	let selection = '';
	let copy = '';
	let ranges = [];
	let active = { row: 0, col: 0 };
	for (const a of actions) {
		if (a.op === 'select-ref') {
			const range = parseRange(a.ref || 'A1');
			if (!range) throw new Error(`bad ref ${a.ref}`);
			ranges = [range];
			active = { ...range.start };
			selection = selectionRef({ sheet, active, ranges });
		} else if (a.op === 'copy') {
			const range = ranges[0] ?? parseRange('A1');
			const payload = copyRange(workbook, sheet, range);
			copy = payload.tsv ?? '';
			selection = selectionRef({ sheet, active, ranges });
		}
	}
	return { selection, copy };
}

/**
 * @returns {Promise<{ ok: boolean, rows: object[], summary: string }>}
 */
export async function runShellChecks() {
	const rows = [];
	let failed = 0;
	for (const c of readyCases()) {
		if (!c.shell || c.format !== 'xlsx') continue;
		const want = new Set(c.compare || ['selection', 'copy']);
		const rust = await rustShell(c.fixture, c.actions || []);
		const ts = await tsShell(c.fixture, c.actions || []);
		const selOk = !want.has('selection') || rust.selection === ts.selection;
		const copyOk =
			!want.has('copy') || normalizeCopy(rust.copy) === normalizeCopy(ts.copy);
		const ok = selOk && copyOk;
		if (!ok) failed++;
		rows.push({
			case: c.id,
			kind: 'xlsx-shell',
			ok,
			selOk,
			copyOk,
			rust,
			ts: { selection: ts.selection, copy: normalizeCopy(ts.copy) },
		});
	}
	const summary = [
		`selection shell: ${failed === 0 ? 'PASS' : 'FAIL'} (${rows.filter((r) => r.ok).length}/${rows.length} xlsx)`,
		...rows.map((r) => {
			const tag = r.ok ? 'OK' : 'FAIL';
			return `  ${tag} ${r.case} sel=${JSON.stringify(r.rust.selection)} copy=${JSON.stringify(normalizeCopy(r.rust.copy))}`;
		}),
	].join('\n');
	return { ok: failed === 0, rows, summary };
}

if (import.meta.main) {
	const { ok, summary } = await runShellChecks();
	console.log(summary);
	process.exit(ok ? 0 : 1);
}
