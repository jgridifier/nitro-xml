// SPDX-License-Identifier: MIT
/**
 * TS oracle for editor recalculation (#77).
 *
 *   bun tools/model-compare/gates/ref/xlsx_editor_recalc.mjs <fixture.xlsx> <script.json> <out.json>
 *
 * Checkpoints are formula cells only, after open, each top-level op, undo-all, and redo-all.
 * Values are the upstream edit session with automatic calculation on.
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../../../..');

function upstreamRoot() {
	return process.env.UPSTREAM_DIR || join(ROOT, 'upstream');
}

function formulaCells(workbook) {
	const cells = [];
	workbook.sheets.forEach((sheet, sheetIndex) => {
		const rows = sheet.rows;
		if (!rows || typeof rows.forEach !== 'function') return;
		rows.forEach((rowMap, row) => {
			if (!rowMap || typeof rowMap.forEach !== 'function') return;
			rowMap.forEach((cell, col) => {
				if (!cell || cell.formula === undefined) return;
				cells.push({
					sheet: sheetIndex,
					row,
					col,
					f: cell.formula,
					v: cell.value === undefined ? null : cell.value,
				});
			});
		});
	});
	cells.sort((a, b) => a.sheet - b.sheet || a.row - b.row || a.col - b.col);
	return cells;
}

async function applyOne(session, op, parseRange) {
	switch (op.op) {
		case 'setCellInput':
			session.setCellInput(op.sheet, op.row, op.col, op.text);
			return;
		case 'setCellValue':
			session.setCellValue(op.sheet, op.row, op.col, op.value);
			return;
		case 'applyStyle': {
			const range = parseRange(op.range);
			if (!range) throw new Error(`bad range ${op.range}`);
			session.applyStyle(op.sheet, [range], op.patch);
			return;
		}
		case 'applyCellStyle': {
			const range = parseRange(op.range);
			if (!range) throw new Error(`bad range ${op.range}`);
			session.applyCellStyle(op.sheet, [range], op.name);
			return;
		}
		case 'addSheet':
			session.addSheet(op.name, op.at);
			return;
		case 'renameSheet':
			session.renameSheet(op.index, op.name);
			return;
		case 'deleteSheet':
			session.deleteSheet(op.index);
			return;
		case 'insertRows':
			session.insertRows(op.sheet, op.at, op.count);
			return;
		case 'deleteRows':
			session.deleteRows(op.sheet, op.at, op.count);
			return;
		case 'insertColumns':
			session.insertColumns(op.sheet, op.at, op.count);
			return;
		case 'deleteColumns':
			session.deleteColumns(op.sheet, op.at, op.count);
			return;
		case 'batch':
			session.batch(op.label || 'batch', () => {
				for (const inner of op.ops) applyOne(session, inner, parseRange);
			});
			return;
		default:
			throw new Error(`unsupported xlsx op: ${op.op}`);
	}
}

async function main() {
	const [, , fixturePath, scriptPath, outPath] = process.argv;
	if (!fixturePath || !scriptPath || !outPath) {
		console.error(
			'Usage: bun xlsx_editor_recalc.mjs <fixture.xlsx> <script.json> <out.json>',
		);
		process.exit(2);
	}
	const up = upstreamRoot();
	const { loadXlsx } = await import(`${up}/src/core/xlsx/read/load.ts`);
	const { createEditSession } = await import(`${up}/src/core/xlsx/edit/session.ts`);
	const { parseRange } = await import(`${up}/src/core/xlsx/address.ts`);

	const bytes = new Uint8Array(readFileSync(fixturePath));
	const script = JSON.parse(readFileSync(scriptPath, 'utf8'));
	const workbook = await loadXlsx(bytes);
	const session = createEditSession(workbook, { recalc: true });
	const checkpoints = [];
	const push = (phase) => {
		checkpoints.push({ phase, formulas: formulaCells(session.workbook) });
	};
	push('start');
	for (const op of script.ops) {
		await applyOne(session, op, parseRange);
		push('do');
	}
	while (session.canUndo()) {
		session.undo();
		push('undo');
	}
	while (session.canRedo()) {
		session.redo();
		push('redo');
	}
	writeFileSync(outPath, JSON.stringify({ checkpoints }) + '\n');
}

main().catch((err) => {
	console.error(err);
	process.exit(1);
});
