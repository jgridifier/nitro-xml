// SPDX-License-Identifier: MIT
// SheetJS CE 0.20.3 (official CDN build). Fastest documented setup that still yields every cell's
// value and type: dense mode plus skipping formatted text, formulae and HTML
// (https://docs.sheetjs.com/docs/api/parse-options, https://docs.sheetjs.com/docs/csf/sheet#dense-mode).
import * as XLSX from 'xlsx';

export const OPTIONS = { type: 'array', dense: true, cellText: false, cellFormula: false, cellHTML: false, cellStyles: false };

export const parse = (bytes) => XLSX.read(bytes, OPTIONS);

const ERRORS = { 0: '#NULL!', 7: '#DIV/0!', 15: '#VALUE!', 23: '#REF!', 29: '#NAME?', 36: '#NUM!', 42: '#N/A', 43: '#GETTING_DATA' };

export function extract(wb) {
	const cells = [];
	for (const name of wb.SheetNames) {
		const ws = wb.Sheets[name];
		const rows = ws && ws['!data'];
		if (!rows) continue;
		for (let r = 0; r < rows.length; r++) {
			const row = rows[r];
			if (!row) continue;
			for (let c = 0; c < row.length; c++) {
				const cell = row[c];
				if (!cell || cell.t === 'z' || cell.v === undefined) continue;
				const ref = XLSX.utils.encode_cell({ r, c });
				if (cell.t === 'e') cells.push([name, ref, 'e', ERRORS[cell.v] ?? String(cell.v)]);
				else cells.push([name, ref, cell.t, cell.v instanceof Date ? cell.v.toISOString() : cell.v]);
			}
		}
	}
	return { cells };
}
