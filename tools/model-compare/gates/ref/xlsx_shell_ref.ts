// SPDX-License-Identifier: MIT
// Reference side of the #29 xlsx shell dump: zoom default, tabs, copy, safe-href links, find.
import { readFileSync } from 'node:fs';

const up = process.env.UPSTREAM_DIR!;
const { hyperlinkPolicy } = await import(`${up}/src/core/opc/safe-href.ts`);
const { loadXlsx } = await import(`${up}/src/core/xlsx/read/load.ts`);
const { cellView } = await import(`${up}/src/core/xlsx/layout/cell-view.ts`);
const { createConditionalFormatEvaluator } = await import(
	`${up}/src/core/xlsx/layout/cf-evaluator.ts`
);
const { createCalcEngine } = await import(`${up}/src/core/xlsx/formula/engine.ts`);
const { parseRange } = await import(`${up}/src/core/xlsx/address.ts`);

function displayed(
	workbook: any,
	sheetIndex: number,
	row: number,
	col: number,
	cf: ReturnType<typeof createConditionalFormatEvaluator> | undefined,
): string {
	return cellView(workbook, sheetIndex, row, col, cf).text;
}

function cellTextTsv(
	workbook: any,
	sheetIndex: number,
	rangeRef: string,
	cf: ReturnType<typeof createConditionalFormatEvaluator> | undefined,
): string {
	const range = parseRange(rangeRef);
	if (!range) return '';
	const lines: string[] = [];
	for (let row = range.start.row; row <= range.end.row; row++) {
		const cols: string[] = [];
		for (let col = range.start.col; col <= range.end.col; col++) {
			cols.push(displayed(workbook, sheetIndex, row, col, cf));
		}
		lines.push(cols.join('\t'));
	}
	return lines.join('\n');
}

function linkAllowed(target?: string, location?: string): boolean {
	if (location) return true;
	return hyperlinkPolicy(target).store;
}

function hyperlinksJson(workbook: any, sheetIndex: number) {
	const sheet = workbook.sheets[sheetIndex];
	if (!sheet) return [];
	const out: unknown[] = [];
	for (const link of sheet.hyperlinks ?? []) {
		if (!linkAllowed(link.target, link.location)) continue;
		const open =
			!!(link.location && String(link.location).length) || hyperlinkPolicy(link.target).open;
		const entry: Record<string, unknown> = {
			sheet: sheetIndex,
			row: link.range.start.row,
			col: link.range.start.col,
			open,
		};
		if (link.target) entry.target = link.target;
		if (link.location) entry.location = link.location;
		if (link.tooltip) entry.tooltip = link.tooltip;
		out.push(entry);
	}
	return out;
}

function searchSample(
	workbook: any,
	query: string,
	cfFor: (sheetIndex: number) => ReturnType<typeof createConditionalFormatEvaluator> | undefined,
) {
	const q = query.toLowerCase();
	const matches: unknown[] = [];
	for (let sheetIndex = 0; sheetIndex < workbook.sheets.length; sheetIndex++) {
		const sheet = workbook.sheets[sheetIndex];
		if (!sheet || sheet.state !== 'visible') continue;
		const cf = cfFor(sheetIndex);
		const rows: Map<number, Map<number, unknown>> =
			sheet.rows instanceof Map ? sheet.rows : new Map();
		for (const [row, cols] of rows) {
			if (!(cols instanceof Map)) continue;
			for (const col of cols.keys()) {
				const text = displayed(workbook, sheetIndex, row, col, cf);
				if (!text.toLowerCase().includes(q)) continue;
				matches.push({ sheet: sheetIndex, row, col, text });
			}
		}
	}
	matches.sort((a: any, b: any) => a.sheet - b.sheet || a.row - b.row || a.col - b.col);
	return matches.slice(0, 20);
}

async function dump(file: string) {
	const bytes = new Uint8Array(readFileSync(file));
	const workbook = await loadXlsx(bytes);
	const calc = createCalcEngine(workbook);
	const cfCache = new Map<number, ReturnType<typeof createConditionalFormatEvaluator> | undefined>();
	const cfFor = (sheetIndex: number) => {
		if (cfCache.has(sheetIndex)) return cfCache.get(sheetIndex);
		const sheet = workbook.sheets[sheetIndex];
		const cf =
			sheet?.conditionalFormats?.length > 0
				? createConditionalFormatEvaluator(workbook, sheetIndex, (formula, at) =>
						calc.evaluate(formula, at),
					)
				: undefined;
		cfCache.set(sheetIndex, cf);
		return cf;
	};
	const tabs = workbook.sheets
		.map((s: any, index: number) => ({
			index,
			name: s.name,
			sheetId: s.sheetId,
			state: s.state,
		}))
		.filter((s: any) => s.state === 'visible');
	const active = workbook.activeSheet ?? 0;
	return {
		format: 'xlsx',
		zoom: 100.0,
		tabs,
		selection: 'A1',
		copySample: cellTextTsv(workbook, active, 'A1', cfFor(active)),
		hyperlinks: hyperlinksJson(workbook, active),
		findSample: searchSample(workbook, 'a', cfFor),
		diagnostics: {
			fontSubstitutions: [],
			unsupported: [],
		},
	};
}

const file = process.argv[2];
if (!file) {
	console.log(JSON.stringify({ error: 'usage' }));
	process.exit(2);
}
try {
	console.log(JSON.stringify(await dump(file)));
} catch (e) {
	console.log(JSON.stringify({ error: String(e) }));
	process.exit(1);
}
