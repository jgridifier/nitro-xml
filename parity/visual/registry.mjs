// SPDX-License-Identifier: MIT
/**
 * Visual-gate case registry (#45).
 *
 * Add a row and a prepare/host path when a format's first paint is ready.
 * pptx cases are generated from the fixture corpus (see pptx-corpus.mjs, #27).
 * docx Print Layout IR is hosted via `docx-layout-ir` (#23 / #158).
 */

import { pptxVisualCorpus } from './pptx-corpus.mjs';

/** @typedef {'xlsx-view-ir' | 'chart-svg' | 'xlsx-viewer' | 'pptx-viewer' | 'docx-layout-ir' | 'docx-viewer'} VisualKind */

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
		informational: true,
		fixture: 'fixtures/bidi/arabic-hebrew-paragraphs.docx',
		width: 800,
		height: 600,
		rust: true,
		threshold: 0.001,
	},
];

/** @type {VisualCase[]} */
export const VISUAL_CASES = [...STATIC_CASES, ...pptxVisualCorpus()];

export function readyCases() {
	return VISUAL_CASES.filter((c) => c.ready);
}
