// SPDX-License-Identifier: MIT
/**
 * TS oracle: apply an xlsx edits/*.json script and dump touched cell model state (#66).
 *
 * Usage:
 *   bun parity/edit/xlsx_model_dump.mjs <fixture.xlsx> <script.json> <out.json>
 *
 * Snapshot shape (matches Rust XlsxHost::model_snapshot):
 *   { cells: [{ sheet, r, c, v, f?, s?, style }], stylesCount,
 *     sheets: [{ name, sheetId, state }], activeSheet }
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');

function upstreamRoot() {
	return process.env.UPSTREAM_DIR || join(ROOT, 'upstream');
}

/** @param {object[]} ops @param {Set<string>} out @param {(r:string)=>object|null} parseRange */
function collectTouched(ops, out, parseRange) {
	for (const op of ops) {
		switch (op.op) {
			case 'setCellInput':
			case 'setCellValue':
				out.add(`${op.sheet},${op.row},${op.col}`);
				break;
			case 'applyStyle':
			case 'applyCellStyle': {
				const range = parseRange(op.range);
				if (!range) throw new Error(`bad range ${op.range}`);
				const r = {
					start: {
						row: Math.min(range.start.row, range.end.row),
						col: Math.min(range.start.col, range.end.col),
					},
					end: {
						row: Math.max(range.start.row, range.end.row),
						col: Math.max(range.start.col, range.end.col),
					},
				};
				for (let row = r.start.row; row <= r.end.row; row++) {
					for (let col = r.start.col; col <= r.end.col; col++) {
						out.add(`${op.sheet},${row},${col}`);
					}
				}
				break;
			}
			case 'batch':
				collectTouched(op.ops || [], out, parseRange);
				break;
			default:
				break;
		}
	}
}

function applyXlsxOps(session, ops, parseRange) {
	for (const op of ops) {
		switch (op.op) {
			case 'setCellInput':
				session.setCellInput(op.sheet, op.row, op.col, op.text);
				break;
			case 'setCellValue':
				session.setCellValue(op.sheet, op.row, op.col, op.value);
				break;
			case 'applyStyle': {
				const range = parseRange(op.range);
				if (!range) throw new Error(`bad range ${op.range}`);
				session.applyStyle(op.sheet, [range], op.patch);
				break;
			}
			case 'applyCellStyle': {
				const range = parseRange(op.range);
				if (!range) throw new Error(`bad range ${op.range}`);
				session.applyCellStyle(op.sheet, [range], op.name);
				break;
			}
			case 'addSheet':
				session.addSheet(op.name, op.at);
				break;
			case 'renameSheet':
				session.renameSheet(op.index, op.name);
				break;
			case 'deleteSheet':
				session.deleteSheet(op.index);
				break;
			case 'batch':
				session.batch(op.label || 'batch', () => applyXlsxOps(session, op.ops, parseRange));
				break;
			default:
				throw new Error(`unsupported xlsx op: ${op.op}`);
		}
	}
}

async function main() {
	const [, , fixturePath, scriptPath, outPath] = process.argv;
	if (!fixturePath || !scriptPath || !outPath) {
		console.error(
			'Usage: bun xlsx_model_dump.mjs <fixture.xlsx> <script.json> <out.json>',
		);
		process.exit(2);
	}

	const up = upstreamRoot();
	const { loadXlsx } = await import(`${up}/src/core/xlsx/read/load.ts`);
	const { createEditSession } = await import(`${up}/src/core/xlsx/edit/session.ts`);
	const { parseRange } = await import(`${up}/src/core/xlsx/address.ts`);
	const { getCell } = await import(`${up}/src/core/xlsx/cells.ts`);
	const { styleAt } = await import(`${up}/src/core/xlsx/styles.ts`);

	const bytes = new Uint8Array(readFileSync(fixturePath));
	const script = JSON.parse(readFileSync(scriptPath, 'utf8'));
	const workbook = await loadXlsx(bytes);
	const session = createEditSession(workbook, { recalc: false });

	const touched = new Set();
	collectTouched(script.ops, touched, parseRange);
	applyXlsxOps(session, script.ops, parseRange);

	const cells = [];
	const keys = [...touched].sort((a, b) => {
		const [as, ar, ac] = a.split(',').map(Number);
		const [bs, br, bc] = b.split(',').map(Number);
		return as - bs || ar - br || ac - bc;
	});
	for (const key of keys) {
		const [sheet, r, c] = key.split(',').map(Number);
		const ws = session.workbook.sheets[sheet];
		const cell = ws ? getCell(ws, r, c) : undefined;
		if (!cell) {
			cells.push({ sheet, r, c, v: null });
			continue;
		}
		/** @type {Record<string, unknown>} */
		const entry = {
			sheet,
			r,
			c,
			v: cell.value === undefined ? null : cell.value,
			style: styleAt(session.workbook, cell.styleId),
		};
		if (cell.formula !== undefined) entry.f = cell.formula;
		if (cell.styleId !== undefined) entry.s = cell.styleId;
		cells.push(entry);
	}

	const snap = {
		cells,
		stylesCount: session.workbook.styles.length,
		sheets: session.workbook.sheets.map((s) => ({
			name: s.name,
			sheetId: s.sheetId,
			state: s.state,
		})),
		activeSheet: session.workbook.activeSheet,
	};
	writeFileSync(outPath, JSON.stringify(snap) + '\n');
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
