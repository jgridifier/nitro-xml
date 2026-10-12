// SPDX-License-Identifier: MIT
// Reference side of the formula-function differential (#73 Math & Trig /
// Statistical, #74 Logical / Text / Information / Date & Time,
// #75 Lookup & Reference / Financial / Engineering).
// Reads a JSON array of cases and prints {formula, rows} from recalculateAll
// (cleaned spill), using the seeded engine in test-helpers.ts.
// `date1904: true` selects the 1904 date system before calculation.
import { readFileSync } from 'node:fs';

const up = process.env.UPSTREAM_DIR;
if (!up) {
	console.error('xlsx_fns_ref: UPSTREAM_DIR is not set');
	process.exit(2);
}

const { book, set, engine, locate } = await import(`${up}/src/core/xlsx/formula/test-helpers.ts`);
const { getCell } = await import(`${up}/src/core/xlsx/cells.ts`);

type Case = {
	formula?: string;
	cells?: Record<string, unknown>;
	hidden?: Record<string, number[]>;
	sheet?: string;
	row?: number;
	col?: number;
	sheets?: string[];
	date1904?: boolean;
};

function colLabel(col: number): string {
	let n = col + 1;
	let out = '';
	while (n > 0) {
		const rem = (n - 1) % 26;
		out = String.fromCharCode(65 + rem) + out;
		n = Math.floor((n - 1) / 26);
	}
	return out;
}

function dumpCase(item: Case): { formula: string; rows: unknown[][] } {
	const formula = item.formula ?? '';
	const sheets = item.sheets?.length ? item.sheets : ['Sheet1', 'Sheet2'];
	const wb = book(item.cells ?? {}, sheets);
	if (item.date1904) wb.date1904 = true;
	if (item.hidden) {
		for (const [name, rows] of Object.entries(item.hidden)) {
			const sheet = wb.sheets.findIndex((candidate: { name: string }) => candidate.name === name);
			const ws = wb.sheets[sheet];
			if (!ws) continue;
			for (const row of rows) ws.rowInfo.set(row, { hidden: true });
		}
	}
	const sheetName = item.sheet ?? sheets[0];
	const row = item.row ?? 0;
	const col = item.col ?? 26;
	const key = `${sheetName}!${colLabel(col)}${row + 1}`;
	const text = formula.startsWith('=') ? formula : `=${formula}`;
	set(wb, key, text);
	const eng = engine(wb);
	eng.recalculateAll();
	const at = locate(wb, key);
	const spill = eng.spillRange(at.sheet, at.row, at.col);
	const ws = wb.sheets[at.sheet];
	const read = (r: number, c: number) => getCell(ws, r, c)?.value ?? 0;
	const rows: unknown[][] = [];
	if (spill) {
		for (let r = spill.start.row; r <= spill.end.row; r++) {
			const line: unknown[] = [];
			for (let c = spill.start.col; c <= spill.end.col; c++) line.push(read(r, c));
			rows.push(line);
		}
	} else {
		rows.push([read(at.row, at.col)]);
	}
	return { formula, rows };
}

const path = process.argv[2];
if (!path) {
	console.error('usage: xlsx_fns_ref.ts <cases.json>');
	process.exit(2);
}
const cases = JSON.parse(readFileSync(path, 'utf8')) as Case[];
process.stdout.write(`${JSON.stringify(cases.map(dumpCase))}\n`);
