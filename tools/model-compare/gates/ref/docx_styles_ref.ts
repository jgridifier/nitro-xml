// SPDX-License-Identifier: MIT
// Reference side of the #20 gate: upstream styles/numbering/theme/settings dump JSON.
import { readFileSync } from 'node:fs';

const up = process.env.UPSTREAM_DIR!;
const { readPackage } = await import(`${up}/src/core/docx/parse.ts`);
const { parseParagraphStyleCatalog, resolveParagraphFormatting } = await import(
	`${up}/src/core/docx/paragraph-styles.ts`,
);
const { parseRunStyleCatalog } = await import(`${up}/src/core/docx/character-styles.ts`);
const { defaultStyleCatalogs } = await import(`${up}/src/core/docx/default-styles.ts`);
const { parseNumberingCatalog } = await import(`${up}/src/core/docx/numbering-parse.ts`);
const {
	computeListLabels,
	displayListLabel,
	resolveParagraphNumbering,
} = await import(`${up}/src/core/docx/numbering-format.ts`);
const { parseTheme, parseColorSchemeMapping } = await import(`${up}/src/core/docx/theme.ts`);
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
if (!model.theme && text('word/settings.xml')) {
	theme.colorMapping = parseColorSchemeMapping(text('word/settings.xml')!);
}
const settings = {
	evenAndOddHeaders: Boolean(model.evenAndOddHeaders),
	autoHyphenation: Boolean(model.autoHyphenation),
};

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

console.log(
	JSON.stringify({
		format,
		paragraphStyles,
		characterStyles,
		numberingCatalog,
		theme,
		settings,
		resolvedParagraphs,
		listLabels,
	}),
);
