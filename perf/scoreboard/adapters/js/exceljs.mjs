// SPDX-License-Identifier: MIT
// ExcelJS 4.4.0. In a browser the documented reader is workbook.xlsx.load(buffer)
// (https://github.com/exceljs/exceljs#reading-xlsx); the streaming WorkbookReader needs Node streams.
import ExcelJS from 'exceljs';

export async function parse(bytes) {
	const wb = new ExcelJS.Workbook();
	await wb.xlsx.load(bytes);
	return wb;
}

// ExcelJS turns date-formatted numbers into JS Dates; map them back to the stored serial in the
// workbook's own date system (1900 or 1904) so the value compares like for like.
let epochMs = Date.UTC(1899, 11, 30);
const serial = (d) => (d.getTime() - epochMs) / 86400000;

function typed(v) {
	if (v === null || v === undefined) return null;
	if (typeof v === 'number') return ['n', v];
	if (typeof v === 'boolean') return ['b', v];
	if (typeof v === 'string') return ['s', v];
	if (v instanceof Date) return ['n', serial(v)];
	if (v.error) return ['e', v.error];
	if (v.richText) return ['s', v.richText.map((r) => r.text).join('')];
	if ('formula' in v || 'sharedFormula' in v) return v.result === undefined ? null : typed(v.result);
	if ('text' in v) return typed(v.text);
	return null;
}

export function extract(wb) {
	epochMs = wb.properties?.date1904 ? Date.UTC(1904, 0, 1) : Date.UTC(1899, 11, 30);
	const cells = [];
	wb.eachSheet((ws) => {
		ws.eachRow({ includeEmpty: false }, (row) => {
			row.eachCell({ includeEmpty: false }, (cell) => {
				if (cell.type === ExcelJS.ValueType.Merge) return;
				// Formula cells: the documented `cell.result` (cell.value drops a cached result of 0).
				const t = typed(cell.type === ExcelJS.ValueType.Formula ? cell.result : cell.value);
				if (t) cells.push([ws.name, cell.address, t[0], t[1]]);
			});
		});
	});
	return { cells };
}
