// SPDX-License-Identifier: MIT
/**
 * TS oracle: xlsx view IR = CellItem[] (plus sheet metadata) for the used viewport.
 * Dump shape matches what the DOM grid painters consume.
 */

import { readFileSync } from 'node:fs';
import { upstreamDir } from '../paths.ts';

/** Deterministic text measure (both engines must use the same rule). */
export function fakeXlsxMeasure(text: string): number {
	let w = 0;
	for (const ch of text) {
		const cp = ch.codePointAt(0) ?? 0;
		w += cp > 255 ? 14 : 7;
	}
	return w;
}

export async function dumpXlsxViewIr(fixturePath: string): Promise<unknown> {
	const up = upstreamDir();
	const { loadXlsx } = await import(`${up}/src/core/xlsx/read/load.ts`);
	const { cellView } = await import(`${up}/src/core/xlsx/layout/cell-view.ts`);
	const { createGridMetrics } = await import(`${up}/src/core/xlsx/layout/metrics.ts`);
	const { buildItems } = await import(`${up}/src/core/xlsx/ui/grid/cell-items.ts`);
	const { usedRange } = await import(`${up}/src/core/xlsx/cells.ts`);

	const bytes = readFileSync(fixturePath);
	const workbook = await loadXlsx(bytes);

	const sheets = workbook.sheets.map((sheet: any, sheetIndex: number) => {
		const metrics = createGridMetrics(sheet);
		const used = usedRange(sheet);
		const lastRow = Math.min(metrics.lastRow, used ? used.end.row + 5 : 40, 200);
		const lastCol = Math.min(metrics.lastCol, used ? used.end.col + 2 : 20, 60);
		const rows = Array.from({ length: lastRow + 1 }, (_, i) => i);
		const cols = Array.from({ length: lastCol + 1 }, (_, i) => i);
		const items = buildItems({
			sheet,
			metrics,
			zoom: metrics.zoom / 100,
			rows,
			cols,
			view: (r: number, c: number) => cellView(workbook, sheetIndex, r, c),
			measure: fakeXlsxMeasure,
			hasFormula: (r: number, c: number) => {
				const row = sheet.rows instanceof Map ? sheet.rows.get(r) : undefined;
				const cell = row instanceof Map ? row.get(c) : undefined;
				return !!(cell && (cell as { formula?: unknown }).formula);
			},
		});
		return {
			name: sheet.name,
			sheetIndex,
			lastRow,
			lastCol,
			items,
		};
	});

	return { format: 'xlsx', sheets };
}
