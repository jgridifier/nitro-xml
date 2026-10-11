// SPDX-License-Identifier: MIT
/**
 * Visual-gate case registry (#45).
 *
 * Add a row and a prepare/host path when a format's first paint is ready.
 * docx / pptx stay `ready: false` until their painters ship.
 */

/** @typedef {'xlsx-view-ir' | 'chart-svg' | 'xlsx-viewer'} VisualKind */

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
 * @property {boolean} [rust] when true, prepare + compare a Rust/WASM side
 * @property {boolean} [informational] when true, mismatches are reported but do not fail the gate
 */

/** @type {VisualCase[]} */
export const VISUAL_CASES = [
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
		// Live `<nitro-xlsx>` (command-buffer grid) vs upstream `<xlsx-editor>` (Lit chrome +
		// DOM grid) will not meet PRODUCT_SPEC ≤0.1% until painters converge. Record ratios;
		// do not fail CI.
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
		id: 'docx',
		format: 'docx',
		ready: false,
		todo: 'Register when docx Print Layout paint is hosted (after renderer port).',
	},
	{
		id: 'pptx',
		format: 'pptx',
		ready: false,
		todo: 'Register when pptx slide paint is hosted (after renderer port).',
	},
];

export function readyCases() {
	return VISUAL_CASES.filter((c) => c.ready);
}
