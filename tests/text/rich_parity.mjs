// SPDX-License-Identifier: MIT
/**
 * Rich multi-paragraph surface parity (#61).
 * Drives richTextSurfaceEval scripts: cross-¶ selection, style-aware insert,
 * table cell nav, and a readonly negative control.
 *
 *   bun tests/text/rich_parity.mjs
 */
import { existsSync, mkdirSync, writeFileSync, readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const OUT = join(ROOT, 'tests/text/out');
const CARET_TOL = 1;

/** @type {{ id: string, script: object, expect: (out: any) => string|null }[]} */
const CASES = [
	{
		id: 'cross-paragraph-shift',
		script: {
			paragraphs: ['ab', 'cd'],
			anchor: { kind: 'block', block: 0, offset: 1 },
			focus: { kind: 'block', block: 0, offset: 1 },
			ops: [
				{ op: 'key', key: 'ArrowRight', shift: true },
				{ op: 'key', key: 'ArrowRight', shift: true },
				{ op: 'key', key: 'ArrowRight', shift: true },
			],
			charWidth: 8,
			lineHeight: 16,
		},
		expect: (out) => {
			if (!out.crossParagraph) return 'expected crossParagraph';
			if (out.selected !== 'b\nc') return `selected=${JSON.stringify(out.selected)}`;
			if (Math.abs(out.caret.y - 16) > CARET_TOL) return `caret.y=${out.caret.y}`;
			return null;
		},
	},
	{
		id: 'style-aware-insert',
		script: {
			paragraphs: [{ runs: [{ text: 'Hi', bold: true }] }],
			anchor: { kind: 'block', block: 0, offset: 2 },
			focus: { kind: 'block', block: 0, offset: 2 },
			ops: [{ op: 'insert', text: '!' }],
		},
		expect: (out) => {
			if (out.plain !== 'Hi!') return `plain=${out.plain}`;
			if (!out.styleAtCaret.bold) return 'caret style lost bold';
			return null;
		},
	},
	{
		id: 'table-cell-caret',
		script: {
			table: [
				['A', 'B'],
				['C', 'D'],
			],
			anchor: { kind: 'cell', block: 0, row: 0, col: 0, para: 0, offset: 1 },
			focus: { kind: 'cell', block: 0, row: 0, col: 0, para: 0, offset: 1 },
			ops: [{ op: 'key', key: 'ArrowRight' }],
			charWidth: 8,
			lineHeight: 16,
		},
		expect: (out) => {
			if (out.focus.kind !== 'cell') return 'focus not in cell';
			if (out.focus.col !== 1 || out.focus.row !== 0) {
				return `expected cell (0,1) got r=${out.focus.row} c=${out.focus.col}`;
			}
			if (out.focus.offset !== 0) return `offset=${out.focus.offset}`;
			return null;
		},
	},
	{
		id: 'negative-readonly-noop',
		script: {
			readOnly: true,
			paragraphs: ['stay'],
			anchor: { kind: 'block', block: 0, offset: 2 },
			focus: { kind: 'block', block: 0, offset: 2 },
			ops: [{ op: 'insert', text: 'X' }, { op: 'key', key: 'Backspace' }],
		},
		expect: (out) => {
			if (out.plain !== 'stay') return `mutated plain=${out.plain}`;
			if (out.focus.offset !== 2) return `offset moved to ${out.focus.offset}`;
			return null;
		},
	},
];

function ensureWasm() {
	const glue = join(ROOT, 'packages/nitro-xml/pkg/nitro_xml_wasm.js');
	if (existsSync(glue)) return;
	const r = spawnSync('bash', [join(ROOT, 'scripts/build-wasm.sh')], {
		cwd: ROOT,
		encoding: 'utf8',
		maxBuffer: 32 * 1024 * 1024,
	});
	if (r.status !== 0) throw new Error(`build-wasm: ${r.stderr || r.stdout}`);
}

async function loadWasm() {
	ensureWasm();
	const pkgDir = join(ROOT, 'packages/nitro-xml/pkg');
	const mod = await import(pathToFileURL(join(pkgDir, 'nitro_xml_wasm.js')).href);
	if (typeof mod.default === 'function') await mod.default();
	else if (typeof mod.initSync === 'function') {
		mod.initSync({ module: readFileSync(join(pkgDir, 'nitro_xml_wasm_bg.wasm')) });
	}
	if (typeof mod.richTextSurfaceEval !== 'function') {
		throw new Error('richTextSurfaceEval missing');
	}
	return mod;
}

try {
	const wasm = await loadWasm();
	mkdirSync(OUT, { recursive: true });
	const results = [];
	let failed = 0;
	for (const c of CASES) {
		const out = JSON.parse(wasm.richTextSurfaceEval(JSON.stringify(c.script)));
		const err = c.expect(out);
		const ok = !err;
		if (!ok) failed++;
		const summary = ok ? `OK ${c.id}` : `FAIL ${c.id}: ${err}`;
		console.log(summary);
		results.push({ id: c.id, ok, summary, out });
	}
	writeFileSync(join(OUT, 'rich-parity.json'), JSON.stringify(results, null, 2));
	const gate =
		failed === 0
			? `rich-text-parity: PASS (${CASES.length}/${CASES.length})`
			: `rich-text-parity: FAIL (${CASES.length - failed}/${CASES.length})`;
	writeFileSync(join(OUT, 'rich-gate-summary.txt'), gate + '\n');
	console.log(gate);
	if (failed) process.exit(1);
} catch (e) {
	console.error(e);
	process.exit(1);
}
