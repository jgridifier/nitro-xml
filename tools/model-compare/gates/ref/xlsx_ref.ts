// SPDX-License-Identifier: MIT
// Reference side of the #15 gate: Stage-3 workbook slice (no worksheet cell parse).
import { readFileSync } from 'node:fs';

const up = process.env.UPSTREAM_DIR!;
const { RELATIONSHIP_TYPES } = await import(`${up}/src/core/opc/index.ts`);
const { detectDigitalSignatures } = await import(`${up}/src/core/opc/signature/index.ts`);
const { parseRange } = await import(`${up}/src/core/xlsx/address.ts`);
const { createWorksheet } = await import(`${up}/src/core/xlsx/workbook.ts`);
const { CONTENT_TYPES, SourceIndex, readZipParts } = await import(
	`${up}/src/core/xlsx/read/package.ts`
);
const { parseSharedStrings } = await import(`${up}/src/core/xlsx/read/shared-strings.ts`);
const { parseStyles } = await import(`${up}/src/core/xlsx/read/styles.ts`);
const { parseTheme } = await import(`${up}/src/core/xlsx/read/theme.ts`);
const { parseWorkbookPart } = await import(`${up}/src/core/xlsx/read/workbook-part.ts`);

const MACRO_TYPES = new Set<string>([CONTENT_TYPES.workbookMacro, CONTENT_TYPES.templateMacro]);
const PRINT_AREA = '_xlnm.Print_Area';

function liftPrintAreas(workbook: {
	definedNames: Array<{
		name: string;
		formula: string;
		localSheet?: number;
		hidden?: boolean;
		comment?: string;
	}>;
	sheets: Array<{ pageSetup?: { printArea?: unknown } }>;
}): void {
	workbook.definedNames = workbook.definedNames.filter((name) => {
		if (name.name !== PRINT_AREA || name.localSheet === undefined) return true;
		const sheet = workbook.sheets[name.localSheet];
		const match = /^(?:'(?:[^']|'')+'|[^!,]+)!([$A-Z0-9:]+)$/i.exec(name.formula.trim());
		const range = match ? parseRange(match[1] ?? '') : undefined;
		if (!sheet || !range) return true;
		sheet.pageSetup = { ...sheet.pageSetup, printArea: range };
		return false;
	});
}

function sheetJson(sheet: {
	name: string;
	sheetId: number;
	state: string;
	partName?: string;
	pageSetup?: { printArea?: { start: { row: number; col: number }; end: { row: number; col: number } } };
}) {
	const out: Record<string, unknown> = {
		name: sheet.name,
		sheetId: sheet.sheetId,
		state: sheet.state,
	};
	if (sheet.partName !== undefined) out.partName = sheet.partName;
	if (sheet.pageSetup?.printArea) {
		out.pageSetup = { printArea: sheet.pageSetup.printArea };
	}
	return out;
}

function definedNameJson(name: {
	name: string;
	formula: string;
	localSheet?: number;
	hidden?: boolean;
	comment?: string;
}) {
	const out: Record<string, unknown> = { name: name.name, formula: name.formula };
	if (name.localSheet !== undefined) out.localSheet = name.localSheet;
	if (name.hidden) out.hidden = true;
	if (name.comment) out.comment = name.comment;
	return out;
}

async function dump(file: string): Promise<Record<string, unknown>> {
	const bytes = new Uint8Array(readFileSync(file));
	const parts = await readZipParts(bytes);
	const source = new SourceIndex(parts);
	const workbookPart = source.workbookPart();
	if (!workbookPart) throw new Error('XLSX package has no workbook part (xl/workbook.xml)');
	const book = parseWorkbookPart(source.text(workbookPart) ?? '');
	const warnings: string[] = [];
	const warn = (message: string) => {
		if (!warnings.includes(message)) warnings.push(message);
	};
	const partOf = (type: string) => source.targetOfType(workbookPart, type);
	const stylesPart = partOf(RELATIONSHIP_TYPES.styles);
	const styles = parseStyles(stylesPart ? source.text(stylesPart) : undefined, warnings);
	const themePart = partOf(RELATIONSHIP_TYPES.theme);
	const stringsPart = partOf(RELATIONSHIP_TYPES.sharedStrings);
	const sharedStrings = parseSharedStrings(
		stringsPart ? source.text(stringsPart) : undefined,
		styles.palette,
	);
	const rels = source.rels(workbookPart);
	const sheets = book.sheets.map((entry) => {
		const rel = rels.get(entry.relId);
		const target = rel ? source.target(workbookPart, rel) : undefined;
		let sheet = createWorksheet(entry.name, entry.sheetId);
		if (rel?.type === RELATIONSHIP_TYPES.worksheet && target) {
			sheet.partName = target;
		} else {
			if (target) sheet.partName = target;
			const kind = rel?.type.split('/').pop() ?? 'sheet';
			warn(`Sheet "${entry.name}" is a ${kind}; it is kept on save but cannot be shown or edited.`);
		}
		sheet.state = entry.state;
		return sheet;
	});
	if (source.has('xl/vbaProject.bin') || partOf(RELATIONSHIP_TYPES.vbaProject))
		warn('The workbook contains VBA macros; they are kept on save but never run.');
	if (partOf(RELATIONSHIP_TYPES.pivotCacheDefinition))
		warn('Pivot tables are kept on save but are not refreshed or shown as pivot tables.');
	if (partOf(RELATIONSHIP_TYPES.externalLink))
		warn('External workbook links are kept but not updated.');
	const signatures = detectDigitalSignatures([...parts.keys()]);
	if (signatures.hasSignatures)
		warn(
			'The workbook is digitally signed. The signature is not verified, and saving removes it because any change invalidates it.',
		);
	const contentType = source.contentType(workbookPart) ?? CONTENT_TYPES.workbook;
	if (!sheets.length) throw new Error('XLSX workbook has no sheets');
	const workbook = {
		sheets,
		styles: styles.styles,
		namedStyles: styles.namedStyles,
		definedNames: book.definedNames,
		theme: parseTheme(themePart ? source.text(themePart) : undefined),
		activeSheet: Math.min(Math.max(0, book.activeTab), Math.max(0, sheets.length - 1)),
		date1904: book.date1904,
		format: MACRO_TYPES.has(contentType) ? 'xlsm' : 'xlsx',
		warnings,
		xfMap: styles.xfMap,
		palette: styles.palette,
		dxfs: styles.dxfs,
		sharedStrings,
		fullCalcOnLoad: book.fullCalcOnLoad ? true : undefined,
		structureLocked: book.structureLocked ? true : undefined,
		calcMode: book.calcMode,
		iterate: book.iterate,
		workbookPasswordHash: book.workbookPasswordHash,
		workbookModernHash: book.workbookModernHash,
	};
	liftPrintAreas(workbook);
	const out: Record<string, unknown> = {
		format: workbook.format,
		date1904: workbook.date1904,
		activeSheet: workbook.activeSheet,
		sheets: workbook.sheets.map(sheetJson),
		definedNames: workbook.definedNames.map(definedNameJson),
		theme: workbook.theme,
		styles: workbook.styles,
		namedStyles: workbook.namedStyles,
		xfMap: workbook.xfMap,
		dxfs: workbook.dxfs,
		sharedStrings: workbook.sharedStrings,
		warnings: workbook.warnings,
	};
	if (workbook.palette) out.palette = workbook.palette;
	if (workbook.fullCalcOnLoad) out.fullCalcOnLoad = true;
	if (workbook.structureLocked) out.structureLocked = true;
	if (workbook.calcMode) out.calcMode = workbook.calcMode;
	if (workbook.iterate) out.iterate = workbook.iterate;
	if (workbook.workbookPasswordHash) out.workbookPasswordHash = workbook.workbookPasswordHash;
	if (workbook.workbookModernHash) out.workbookModernHash = workbook.workbookModernHash;
	return out;
}

const file = process.argv[2];
if (!file) {
	console.log(JSON.stringify({ error: 'usage' }));
	process.exit(2);
}
try {
	const out = await dump(file);
	console.log(JSON.stringify(out));
} catch (e) {
	console.log(JSON.stringify({ error: e instanceof Error ? e.message : String(e) }));
	process.exit(2);
}
