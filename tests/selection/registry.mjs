// SPDX-License-Identifier: MIT
/**
 * Selection and caret parity cases (#43).
 *
 * Actions: click, shift-click, drag, keyboard-extend, select-all, copy.
 * Compares selection ranges, caret/highlight rectangles (±1 CSS px), copied text.
 *
 * ready:false + allowList = out of scope (stub painters or pending hosts);
 * the gate still passes. ready:true cases must match on both sides.
 */

/** @typedef {'xlsx'|'docx'|'pptx'} SelectionFormat */

/**
 * @typedef {object} SelectionAction
 * @property {'select-ref'|'click'|'shift-click'|'drag'|'keyboard-extend'|'select-all'|'copy'|'text-select'} op
 * @property {string} [ref] A1 ref, pptx element id, or click target
 * @property {string} [from] drag start cell / text
 * @property {string} [to] drag / shift-click end
 * @property {'ArrowRight'|'ArrowLeft'|'ArrowUp'|'ArrowDown'} [key]
 * @property {number} [count] keyboard-extend repeat count
 * @property {string} [elementId] pptx shape id for text-select
 * @property {string} [substring] text substring to select inside a body
 */

/**
 * @typedef {object} SelectionCase
 * @property {string} id
 * @property {SelectionFormat} format
 * @property {boolean} ready
 * @property {string} [todo]
 * @property {string} [allowList] issue or painter-code ref for deferred work
 * @property {string} [fixture] repo-relative path
 * @property {SelectionAction[]} [actions]
 * @property {boolean} [shell] Node shell select+copy (no Playwright)
 * @property {boolean} [live] Playwright live-viewer actions/geometry
 * @property {string[]} [compare] subset of 'selection'|'copy'|'highlight'|'caret'
 * @property {number} [width]
 * @property {number} [height]
 */

const BIDI_XLSX = 'fixtures/bidi/arabic-hebrew-mixed.xlsx';
const MERGES_XLSX = 'fixtures/generated/xlsx/cells_1k_merges.xlsx';
const POI_MERGES_XLSX = 'fixtures/real/xlsx/poi-57893-many-merges.xlsx';
const BIDI_PPTX = 'fixtures/bidi/arabic-hebrew-slides.pptx';
const TYPEFACE_PPTX = 'fixtures/real/pptx/python-pptx-txt-font-typeface.pptx';

const BIDI_SHAPE = 'ppt/slides/slide1.xml-shape-0';
const TYPEFACE_SHAPE0 = 'ppt/slides/slide1.xml-shape-0';
const TYPEFACE_SHAPE1 = 'ppt/slides/slide1.xml-shape-1';

/** @type {SelectionCase[]} */
export const SELECTION_CASES = [
	// --- xlsx shell (select-ref + TSV copy) ---
	{
		id: 'xlsx-select-copy-a1',
		format: 'xlsx',
		ready: true,
		fixture: BIDI_XLSX,
		shell: true,
		actions: [{ op: 'select-ref', ref: 'A1' }, { op: 'copy' }],
		compare: ['selection', 'copy'],
	},
	{
		id: 'xlsx-select-copy-row',
		format: 'xlsx',
		ready: true,
		fixture: BIDI_XLSX,
		shell: true,
		actions: [{ op: 'select-ref', ref: 'A1:E1' }, { op: 'copy' }],
		compare: ['selection', 'copy'],
	},
	{
		id: 'xlsx-select-copy-merges-block',
		format: 'xlsx',
		ready: true,
		fixture: MERGES_XLSX,
		shell: true,
		actions: [{ op: 'select-ref', ref: 'A1:C3' }, { op: 'copy' }],
		compare: ['selection', 'copy'],
	},
	// --- xlsx live geometry (select-ref) ---
	{
		id: 'xlsx-select-highlight-b1',
		format: 'xlsx',
		ready: true,
		fixture: BIDI_XLSX,
		live: true,
		width: 800,
		height: 480,
		actions: [{ op: 'select-ref', ref: 'B1' }],
		compare: ['selection', 'highlight', 'caret'],
	},
	{
		id: 'xlsx-select-highlight-range',
		format: 'xlsx',
		ready: true,
		fixture: BIDI_XLSX,
		live: true,
		width: 800,
		height: 480,
		actions: [{ op: 'select-ref', ref: 'A1:C1' }],
		compare: ['selection', 'highlight', 'caret'],
	},
	// --- xlsx pointer / keyboard on merges fixture ---
	{
		id: 'xlsx-click-merge',
		format: 'xlsx',
		ready: true,
		fixture: MERGES_XLSX,
		live: true,
		width: 800,
		height: 480,
		actions: [{ op: 'click', ref: 'B2' }, { op: 'copy' }],
		compare: ['selection', 'copy', 'highlight', 'caret'],
	},
	{
		id: 'xlsx-shift-click-merge',
		format: 'xlsx',
		ready: true,
		fixture: MERGES_XLSX,
		live: true,
		width: 800,
		height: 480,
		actions: [
			{ op: 'click', ref: 'A1' },
			{ op: 'shift-click', ref: 'C1' },
			{ op: 'copy' },
		],
		compare: ['selection', 'copy', 'highlight'],
	},
	{
		id: 'xlsx-drag-range',
		format: 'xlsx',
		ready: true,
		fixture: MERGES_XLSX,
		live: true,
		width: 800,
		height: 480,
		// click-drag from A1 expands merge then extends to E1 → A1:E2
		actions: [{ op: 'drag', from: 'A1', to: 'E1' }, { op: 'copy' }],
		compare: ['selection', 'copy', 'highlight'],
	},
	{
		id: 'xlsx-keyboard-extend',
		format: 'xlsx',
		ready: true,
		fixture: MERGES_XLSX,
		live: true,
		width: 800,
		height: 480,
		actions: [
			{ op: 'select-ref', ref: 'A1' },
			{ op: 'keyboard-extend', key: 'ArrowRight', count: 2 },
			{ op: 'copy' },
		],
		compare: ['selection', 'copy', 'highlight', 'caret'],
	},
	{
		id: 'xlsx-select-all-region',
		format: 'xlsx',
		ready: true,
		fixture: MERGES_XLSX,
		live: true,
		width: 800,
		height: 480,
		actions: [{ op: 'select-ref', ref: 'A1' }, { op: 'select-all' }, { op: 'copy' }],
		compare: ['selection', 'copy'],
	},
	{
		id: 'xlsx-drag-poi-merges',
		format: 'xlsx',
		ready: true,
		fixture: POI_MERGES_XLSX,
		live: true,
		width: 800,
		height: 480,
		actions: [{ op: 'drag', from: 'A1', to: 'A3' }, { op: 'copy' }],
		compare: ['selection', 'copy', 'highlight'],
	},
	// --- pptx (reading paint: shape click, text body, select-all, copy) ---
	{
		id: 'pptx-shape-click-copy',
		format: 'pptx',
		ready: true,
		fixture: BIDI_PPTX,
		live: true,
		width: 960,
		height: 540,
		actions: [{ op: 'click', ref: BIDI_SHAPE }, { op: 'copy' }],
		compare: ['selection', 'copy', 'highlight'],
	},
	{
		id: 'pptx-text-select-copy',
		format: 'pptx',
		ready: true,
		fixture: BIDI_PPTX,
		live: true,
		width: 960,
		height: 540,
		actions: [
			{
				op: 'text-select',
				elementId: BIDI_SHAPE,
				substring: 'مرحبا بالعالم',
			},
			{ op: 'copy' },
		],
		// Caret CSS rect for RTL Arabic differs by font/bidi layout (± tens of px);
		// copy of the selected span is the differential proof.
		compare: ['selection', 'copy'],
	},
	{
		id: 'pptx-select-all-copy',
		format: 'pptx',
		ready: true,
		fixture: TYPEFACE_PPTX,
		live: true,
		width: 960,
		height: 540,
		actions: [
			{ op: 'click', ref: TYPEFACE_SHAPE0 },
			{ op: 'select-all' },
			{ op: 'copy' },
		],
		compare: ['selection', 'copy'],
	},
	{
		id: 'pptx-shape-click-typeface',
		format: 'pptx',
		ready: true,
		fixture: TYPEFACE_PPTX,
		live: true,
		width: 960,
		height: 540,
		actions: [{ op: 'click', ref: TYPEFACE_SHAPE1 }, { op: 'copy' }],
		compare: ['selection', 'copy', 'highlight'],
	},
	// --- stub painters only (not on the selection path) ---
	{
		id: 'pptx-stub-p10-3d-charts',
		format: 'pptx',
		ready: false,
		allowList: 'P10',
		todo: '3D chart painter deferred (docs/adr/0002); not on view selection path.',
	},
	{
		id: 'pptx-stub-p12-smartart-3d',
		format: 'pptx',
		ready: false,
		allowList: 'P12',
		todo: 'SmartArt 3D painter deferred.',
	},
	{
		id: 'pptx-stub-p13-threejs',
		format: 'pptx',
		ready: false,
		allowList: 'P13',
		todo: 'three.js host deferred.',
	},
	{
		id: 'pptx-stub-p25-print-notes',
		format: 'pptx',
		ready: false,
		allowList: 'P25',
		todo: 'Print/notes/handouts deferred.',
	},
	{
		id: 'pptx-stub-p26-raster-export',
		format: 'pptx',
		ready: false,
		allowList: 'P26',
		todo: 'Raster export deferred (not on view path).',
	},
	{
		id: 'pptx-stub-p27-svg-export',
		format: 'pptx',
		ready: false,
		allowList: 'P27',
		todo: 'SVG export deferred (painter code P27, not GitHub #27).',
	},
	// --- docx ---
	{
		id: 'docx-text-span',
		format: 'docx',
		ready: false,
		allowList: '#158',
		todo: 'Cover once <nitro-docx> lands on main (#158 / #23).',
		live: true,
		actions: [{ op: 'click', ref: 'p0' }, { op: 'copy' }],
		compare: ['selection', 'copy', 'caret', 'highlight'],
	},
];

export function readyCases() {
	return SELECTION_CASES.filter((c) => c.ready);
}

export function deferredCases() {
	return SELECTION_CASES.filter((c) => !c.ready);
}
