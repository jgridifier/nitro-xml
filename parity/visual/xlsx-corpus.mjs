// SPDX-License-Identifier: MIT
/**
 * Build xlsx viewer visual cases from fixtures/real/xlsx (#150 follow-up).
 *
 * Hard rows are those measured ≤0.1% at DPR1, DPR2 and print on branch tip;
 * the rest stay informational. `poi-45544` stays the static hard case in
 * `registry.mjs` (stable id `xlsx-viewer-poi-45544`); this corpus skips it.
 */

import { existsSync, readdirSync } from 'node:fs';
import { join, dirname, relative, extname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const XLSX_EXT = /\.(xlsx|xlsm|xltx|xltm)$/i;

/** Already gated as static `xlsx-viewer-poi-45544` (#150). */
const STATIC_HARD = 'fixtures/real/xlsx/poi-45544.xlsx';

/**
 * Fixture paths (repo-relative) that pass ≤0.1% at DPR1 + DPR2 + print.
 * Visual runner launches Chromium with `--disable-lcd-text` (same as docx).
 */
export const HARD_FIXTURES = new Set([
	'fixtures/real/xlsx/poi-45540_classic_Footer.xlsx',
	'fixtures/real/xlsx/poi-45540_classic_Header.xlsx',
	'fixtures/real/xlsx/poi-46535.xlsx',
	'fixtures/real/xlsx/poi-47813.xlsx',
	'fixtures/real/xlsx/poi-49156.xlsx',
	'fixtures/real/xlsx/poi-50096.xlsx',
	'fixtures/real/xlsx/poi-56644.xlsx',
	'fixtures/real/xlsx/poi-57181.xlsm',
	'fixtures/real/xlsx/poi-62629_target.xlsm',
	'fixtures/real/xlsx/poi-ConditionalFormattingSamples.xlsx',
	'fixtures/real/xlsx/poi-NewStyleConditionalFormattings.xlsx',
	'fixtures/real/xlsx/poi-DataTableCities.xlsx',
	'fixtures/real/xlsx/poi-DateFormatTests.xlsx',
	'fixtures/real/xlsx/poi-57893-many-merges.xlsx',
	'fixtures/real/xlsx/poi-61652.xlsx',
	'fixtures/real/xlsx/poi-ExcelPivotTableSample.xlsx',
	'fixtures/real/xlsx/poi-ExcelWithAttachments.xlsm',
	'fixtures/real/xlsx/poi-FormulaEvalTestData_Copy.xlsx',
	'fixtures/real/xlsx/poi-StructuredRefs-lots-with-lookups.xlsx',
	'fixtures/real/xlsx/poi-WithChartSheet.xlsx',
	'fixtures/real/xlsx/poi-no_drawing_patriarch.xlsx',
	'fixtures/real/xlsx/poi-WithEmbeded.xlsx',
	'fixtures/real/xlsx/poi-bug64512_embed.xlsx',
	'fixtures/real/xlsx/poi-test.xltx',
	'fixtures/real/xlsx/python-pptx-shp-embedded-xlsx.xlsx',
	'fixtures/real/xlsx/usgov-census-NST-EST2024-POP.xlsx',
	'fixtures/real/xlsx/usgov-eia-MER-T01.01.xlsx',
	'fixtures/real/xlsx/usgov-fhwa-2022-vm1.xlsx',
]);

function listXlsxFiles() {
	const out = [];
	const walk = (dir) => {
		if (!existsSync(dir)) return;
		for (const name of readdirSync(dir, { withFileTypes: true })) {
			const p = join(dir, name.name);
			if (name.isDirectory()) walk(p);
			else if (XLSX_EXT.test(name.name)) out.push(p);
		}
	};
	walk(join(ROOT, 'fixtures/real/xlsx'));
	return out.filter((p) => XLSX_EXT.test(p)).sort();
}

/**
 * @returns {import('./registry.mjs').VisualCase[]}
 */
export function xlsxVisualCorpus() {
	const paths = listXlsxFiles();
	const maxRaw = process.env.VISUAL_XLSX_MAX;
	const max = maxRaw != null && maxRaw !== '' ? Number(maxRaw) : Infinity;
	const limited = Number.isFinite(max) && max >= 0 ? paths.slice(0, max) : paths;

	return limited
		.map((abs) => {
			const fixture = relative(ROOT, abs).replace(/\\/g, '/');
			if (fixture === STATIC_HARD) return null;
			const ext = extname(fixture).replace(/^\./, '').toLowerCase();
			const base = basename(fixture, extname(fixture));
			const slug = `real-xlsx-${base}-${ext}`
				.replace(/[^a-zA-Z0-9]+/g, '-')
				.replace(/^-+|-+$/g, '')
				.toLowerCase();
			const hard = HARD_FIXTURES.has(fixture);
			return {
				id: `xlsx-viewer-${slug || 'fixture'}`,
				format: 'xlsx',
				kind: 'xlsx-viewer',
				ready: true,
				fixture,
				width: 800,
				height: 480,
				rust: true,
				threshold: 0.001,
				informational: !hard,
			};
		})
		.filter(Boolean);
}
