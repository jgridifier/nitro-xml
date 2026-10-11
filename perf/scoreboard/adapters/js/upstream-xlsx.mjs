// SPDX-License-Identifier: MIT
// Upstream ooxml TS (reference), xlsx: the same entry the upstream Excel UI uses
// (src/ui/src/xlsx/editor-files.ts -> ooxml-core/xlsx/load loadWorkbook).
import { loadWorkbook } from 'ooxml-core/xlsx/load';

export const parse = (bytes) => loadWorkbook(bytes);

export function extract(workbook) {
	const cells = [];
	for (const sheet of workbook.sheets) {
		for (const [row, cols] of sheet.rows) {
			for (const [col, cell] of cols) {
				const v = cell.value;
				if (v === null || v === undefined) continue;
				const ref = colLabel(col) + (row + 1);
				if (typeof v === 'number') cells.push([sheet.name, ref, 'n', v]);
				else if (typeof v === 'boolean') cells.push([sheet.name, ref, 'b', v]);
				else if (typeof v === 'string') cells.push([sheet.name, ref, 's', v]);
				else if (v && typeof v === 'object' && 'error' in v) cells.push([sheet.name, ref, 'e', v.error]);
			}
		}
	}
	return { cells };
}

function colLabel(col) {
	let s = '';
	for (let n = col + 1; n > 0; n = Math.floor((n - 1) / 26)) s = String.fromCharCode(65 + ((n - 1) % 26)) + s;
	return s;
}
