// SPDX-License-Identifier: MIT
// Reference side of the #20 + #21 gate: upstream readPackage dump JSON.
import { readFileSync } from 'node:fs';

const up = process.env.UPSTREAM_DIR!;
const { readPackage } = await import(`${up}/src/core/docx/parse.ts`);
const { defaultStyleCatalogs } = await import(`${up}/src/core/docx/default-styles.ts`);
const { resolveParagraphFormatting } = await import(`${up}/src/core/docx/paragraph-styles.ts`);
const {
	computeListLabels,
	displayListLabel,
	resolveParagraphNumbering,
} = await import(`${up}/src/core/docx/numbering-format.ts`);
const { forEachParagraph } = await import(`${up}/src/core/docx/parse-warnings.ts`);

const { readZipParts } = await import(`${up}/src/core/xlsx/read/package.ts`);

const file = process.argv[2]!;
const bytes = new Uint8Array(readFileSync(file));
const parts = await readZipParts(bytes);
const decoder = new TextDecoder('utf-8');
const text = (part: string): string | undefined => {
	const b = parts.get(part);
	if (!b) return undefined;
	let t = decoder.decode(b);
	if (t.charCodeAt(0) === 0xfeff) t = t.slice(1);
	return t;
};

const formatOf = (mainCt: string | undefined): string => {
	switch (mainCt) {
		case 'application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml':
			return 'docx';
		case 'application/vnd.ms-word.document.macroEnabled.main+xml':
			return 'docm';
		case 'application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml':
			return 'dotx';
		case 'application/vnd.ms-word.template.macroEnabledTemplate.main+xml':
			return 'dotm';
		default:
			return 'docx';
	}
};

const ct = text('[Content_Types].xml');
const mainMatch = ct?.match(
	/Override[^>]+PartName="\/word\/document\.xml"[^>]+ContentType="([^"]+)"/,
);
const format = formatOf(mainMatch?.[1]);

const { model } = await readPackage(bytes);

const defaults = defaultStyleCatalogs();
const paragraphStyles = model.paragraphStyles ?? defaults.paragraphStyles;
const characterStyles = model.characterStyles ?? defaults.characterStyles;
const numberingCatalog = model.numberingCatalog ?? { abstractNums: {}, nums: {}, warnings: [] };
const theme = model.theme ?? { colors: {}, colorMapping: {}, fonts: { major: {}, minor: {} } };
const settings = {
	evenAndOddHeaders: Boolean(model.evenAndOddHeaders),
	autoHyphenation: Boolean(model.autoHyphenation),
};

/** #28: deep chartSpace parity is a separate gate; host fields only here. */
function stripChartDeep(blocks: typeof model.blocks) {
	forEachParagraph(blocks, (paragraph) => {
		for (const run of paragraph.runs) {
			const chart = run.image?.chart;
			if (!chart) continue;
			delete chart.chartSpace;
			delete chart.formatting;
			delete chart.colorPalette;
			delete chart.workbookPartName;
			delete chart.title;
		}
	});
}

stripChartDeep(model.blocks);
if (model.sections) {
	for (const section of model.sections) {
		for (const slots of [section.headers, section.footers]) {
			if (!slots) continue;
			for (const slot of Object.values(slots)) {
				if (slot?.blocks) stripChartDeep(slot.blocks);
			}
		}
	}
}
for (const notes of [model.footnotes, model.endnotes]) {
	if (!notes) continue;
	for (const note of notes) stripChartDeep(note.blocks);
}

const imagePartNames = new Set<string>();
forEachParagraph(
	[
		...model.blocks,
		...(model.sections ?? []).flatMap((s) =>
			[s.headers, s.footers].flatMap((slots) =>
				slots ? Object.values(slots).flatMap((c) => c?.blocks ?? []) : [],
			),
		),
		...(model.footnotes ?? []).flatMap((n) => n.blocks),
		...(model.endnotes ?? []).flatMap((n) => n.blocks),
	],
	(paragraph) => {
		for (const run of paragraph.runs) {
			for (const part of [run.image?.partName, run.image?.svgPartName]) {
				if (part) imagePartNames.add(part);
			}
		}
	},
);

function flattenParagraphs(blocks: typeof model.blocks) {
	const result: (typeof model.blocks)[number] & { type: 'paragraph' }[] = [];
	for (const block of blocks) {
		if (block.type === 'paragraph') result.push(block);
		else for (const row of block.rows) for (const cell of row) result.push(...cell.paragraphs);
	}
	return result;
}

const paragraphs = flattenParagraphs(model.blocks);
const labels = computeListLabels(model);

const resolvedParagraphs = paragraphs.map((p) => ({
	id: p.id,
	resolved: resolveParagraphFormatting(p, paragraphStyles),
	numbering: resolveParagraphNumbering(p, paragraphStyles, numberingCatalog) ?? null,
}));

const listLabels = paragraphs.flatMap((p) => {
	const label = labels.get(p.id);
	if (!label) return [];
	return [
		{
			paragraphId: p.id,
			numId: label.numId,
			level: label.level,
			text: displayListLabel(label.text),
			suffix: label.suffix,
		},
	];
});

const out: Record<string, unknown> = {
	format,
	paragraphStyles,
	characterStyles,
	numberingCatalog,
	theme,
	settings,
	page: model.page,
	blocks: model.blocks,
	resolvedParagraphs,
	listLabels,
	mediaPartNames: [...imagePartNames].sort(),
	warnings: model.warnings,
};
if (model.sections) out.sections = model.sections;
if (model.footnotes) out.footnotes = model.footnotes;
if (model.endnotes) out.endnotes = model.endnotes;
if (model.comments) out.comments = model.comments;
if (model.trackChanges !== undefined) out.trackChanges = model.trackChanges;
if (model.trackFormatting !== undefined) out.trackFormatting = model.trackFormatting;
if (model.trackMoves !== undefined) out.trackMoves = model.trackMoves;
if (model.footnoteNumFmt) out.footnoteNumFmt = model.footnoteNumFmt;
if (model.endnoteNumFmt) out.endnoteNumFmt = model.endnoteNumFmt;
if (model.evenAndOddHeaders) out.evenAndOddHeaders = true;
if (model.autoHyphenation) out.autoHyphenation = true;

console.log(JSON.stringify(out));
