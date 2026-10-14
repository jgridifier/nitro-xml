// SPDX-License-Identifier: MIT
/**
 * Visual-gate case registry (#45).
 *
 * Add a row and a prepare/host path when a format's first paint is ready.
 * pptx cases are generated from the fixture corpus (see pptx-corpus.mjs, #27).
 * docx Print Layout IR is hosted via `docx-layout-ir` (#23 / #158).
 */

import { docxVisualCorpus } from './docx-corpus.mjs';
import { pptxVisualCorpus } from './pptx-corpus.mjs';
import { xlsxVisualCorpus } from './xlsx-corpus.mjs';

/** @typedef {'xlsx-view-ir' | 'chart-svg' | 'xlsx-viewer' | 'xlsx-edit-chrome' | 'pptx-viewer' | 'docx-layout-ir' | 'docx-viewer' | 'docx-edit-chrome'} VisualKind */

/**
 * @typedef {object} VisualCase
 * @property {string} id
 * @property {'xlsx'|'docx'|'pptx'|'chart'} format
 * @property {VisualKind} [kind]
 * @property {boolean} ready
 * @property {string} [todo]
 * @property {string} [fixture] repo-relative path
 * @property {string} [part] OOXML part path inside the package (charts)
 * @property {number} [width]
 * @property {number} [height]
 * @property {number} [slide] pptx slide index (default 0)
 * @property {boolean} [rust] when true, prepare + compare a Rust/WASM side
 * @property {boolean} [informational] when true, mismatches are reported but do not fail the gate
 * @property {boolean} [skipPrint] when true, skip print-to-PDF for this case
 * @property {number} [threshold] per-case mismatch fraction (overrides format default)
 */

/** @type {VisualCase[]} */
const STATIC_CASES = [
	{
		id: 'xlsx-poi-45544',
		format: 'xlsx',
		kind: 'xlsx-view-ir',
		ready: true,
		fixture: 'fixtures/real/xlsx/poi-45544.xlsx',
		width: 800,
		height: 480,
		rust: true,
	},
	{
		id: 'xlsx-viewer-poi-45544',
		format: 'xlsx',
		kind: 'xlsx-viewer',
		ready: true,
		fixture: 'fixtures/real/xlsx/poi-45544.xlsx',
		width: 800,
		height: 480,
		rust: true,
		// Hard case: live `<nitro-xlsx>` vs upstream `<xlsx-editor>` ≤0.1% at DPR1/2/print (#150).
	},
	{
		// #29 AC: layout checked at 375 px (mobile viewport) + DPR 1/2.
		id: 'xlsx-viewer-poi-45544-375',
		format: 'xlsx',
		kind: 'xlsx-viewer',
		ready: true,
		fixture: 'fixtures/real/xlsx/poi-45544.xlsx',
		width: 375,
		height: 667,
		rust: true,
		skipPrint: true,
	},
	{
		id: 'xlsx-edit-chrome-poi-45544',
		format: 'xlsx',
		kind: 'xlsx-edit-chrome',
		ready: true,
		fixture: 'fixtures/real/xlsx/poi-45544.xlsx',
		width: 960,
		height: 120,
		rust: true,
		threshold: 0.001,
		skipPrint: true,
		// Informational until ribbon/formula-bar chrome ≤0.1% vs upstream (#150 follow-up).
		informational: true,
	},
	{
		id: 'docx-edit-chrome-bidi',
		format: 'docx',
		kind: 'docx-edit-chrome',
		ready: true,
		fixture: 'fixtures/bidi/arabic-hebrew-paragraphs.docx',
		width: 960,
		height: 120,
		rust: true,
		threshold: 0.001,
		skipPrint: true,
		// Informational: editor ribbon vs upstream docx chrome (#83 / #150 follow-up).
		informational: true,
	},
	{
		id: 'chart-poi-dataValidation-chart1',
		format: 'chart',
		kind: 'chart-svg',
		ready: true,
		fixture: 'fixtures/real/xlsx/poi-dataValidationTableRange.xlsx',
		part: 'xl/charts/chart1.xml',
		width: 480,
		height: 320,
		rust: true,
	},
	{
		id: 'docx-bidi-paragraphs',
		format: 'docx',
		kind: 'docx-layout-ir',
		ready: true,
		fixture: 'fixtures/bidi/arabic-hebrew-paragraphs.docx',
		width: 800,
		height: 600,
		rust: true,
	},
	{
		id: 'docx-viewer-bidi-paragraphs',
		format: 'docx',
		kind: 'docx-viewer',
		ready: true,
		fixture: 'fixtures/bidi/arabic-hebrew-paragraphs.docx',
		width: 800,
		height: 600,
		rust: true,
		// Hard case: live `<nitro-docx>` vs upstream Print Layout ≤0.1% at DPR1/DPR2/print (#23).
		threshold: 0.001,
	},
	{
		// #29 AC: layout checked at 375 px (mobile viewport) + DPR 1/2.
		id: 'docx-viewer-bidi-paragraphs-375',
		format: 'docx',
		kind: 'docx-viewer',
		ready: true,
		fixture: 'fixtures/bidi/arabic-hebrew-paragraphs.docx',
		width: 375,
		height: 667,
		rust: true,
		threshold: 0.001,
		skipPrint: true,
	},
	{
		// #29 AC: layout checked at 375 px (mobile viewport) + DPR 1/2.
		id: 'pptx-viewer-poi-51187-375',
		format: 'pptx',
		kind: 'pptx-viewer',
		ready: true,
		fixture: 'fixtures/real/pptx/poi-51187.pptx',
		width: 375,
		height: 667,
		slide: 0,
		rust: true,
		skipPrint: true,
	},
];

/** @type {VisualCase[]} */
export const VISUAL_CASES = [
	...STATIC_CASES,
	...xlsxVisualCorpus(),
	...docxVisualCorpus(),
	...pptxVisualCorpus(),
];

export function readyCases() {
	let cases = VISUAL_CASES.filter((c) => c.ready);
	const only = (process.env.VISUAL_ONLY || '')
		.split(',')
		.map((s) => s.trim())
		.filter(Boolean);
	if (only.length) cases = cases.filter((c) => only.includes(c.id));
	return cases;
}
