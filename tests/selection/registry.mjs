// SPDX-License-Identifier: MIT
/**
 * Selection and caret parity cases (#43).
 *
 * Actions: click, shift-click, drag, keyboard-extend, select-all, copy.
 * Compares selection ranges, caret/highlight rectangles (±1 CSS px), copied text.
 *
 * ready:false + allowList = out of scope until the referenced issue lands; the
 * gate still passes. ready:true cases must match on both sides.
 */

/** @typedef {'xlsx'|'docx'|'pptx'} SelectionFormat */

/**
 * @typedef {object} SelectionAction
 * @property {'select-ref'|'click'|'shift-click'|'drag'|'keyboard-extend'|'select-all'|'copy'} op
 * @property {string} [ref] A1 ref for select-ref / click target cell
 * @property {string} [from] drag start cell
 * @property {string} [to] drag / shift-click / keyboard-extend end cell
 * @property {'ArrowRight'|'ArrowLeft'|'ArrowUp'|'ArrowDown'} [key]
 * @property {number} [count] keyboard-extend repeat count
 */

/**
 * @typedef {object} SelectionCase
 * @property {string} id
 * @property {SelectionFormat} format
 * @property {boolean} ready
 * @property {string} [todo]
 * @property {string} [allowList] issue ref for deferred/out-of-scope work
 * @property {string} [fixture] repo-relative path
 * @property {SelectionAction[]} [actions]
 * @property {boolean} [shell] run Node shell select+copy (no Playwright)
 * @property {boolean} [live] run Playwright live-viewer actions/geometry
 * @property {string[]} [compare] subset of 'selection'|'copy'|'highlight'|'caret'
 * @property {number} [width]
 * @property {number} [height]
 */

/** @type {SelectionCase[]} */
export const SELECTION_CASES = [
	// --- xlsx: shell select-ref + copy (wired today) ---
	{
		id: 'xlsx-select-copy-a1',
		format: 'xlsx',
		ready: true,
		fixture: 'fixtures/bidi/arabic-hebrew-mixed.xlsx',
		shell: true,
		actions: [{ op: 'select-ref', ref: 'A1' }, { op: 'copy' }],
		compare: ['selection', 'copy'],
	},
	{
		id: 'xlsx-select-copy-row',
		format: 'xlsx',
		ready: true,
		fixture: 'fixtures/bidi/arabic-hebrew-mixed.xlsx',
		shell: true,
		actions: [{ op: 'select-ref', ref: 'A1:E1' }, { op: 'copy' }],
		compare: ['selection', 'copy'],
	},
	{
		id: 'xlsx-select-highlight-b2',
		format: 'xlsx',
		ready: true,
		fixture: 'fixtures/bidi/arabic-hebrew-mixed.xlsx',
		live: true,
		width: 800,
		height: 480,
		actions: [{ op: 'select-ref', ref: 'B1' }],
		// Ready-mode "caret" is the active-cell outline (.xg-outline / .xg-active).
		compare: ['selection', 'highlight', 'caret'],
	},
	{
		id: 'xlsx-select-highlight-range',
		format: 'xlsx',
		ready: true,
		fixture: 'fixtures/bidi/arabic-hebrew-mixed.xlsx',
		live: true,
		width: 800,
		height: 480,
		actions: [{ op: 'select-ref', ref: 'A1:C1' }],
		compare: ['selection', 'highlight', 'caret'],
	},
	// --- xlsx: pointer / keyboard (need hit-testing + key forwarding) ---
	{
		id: 'xlsx-click',
		format: 'xlsx',
		ready: false,
		allowList: '#29',
		todo: 'Pointer hit-test → selectCell (upstream wirePointer); reserved in xlsx.rs.',
		fixture: 'fixtures/bidi/arabic-hebrew-mixed.xlsx',
		live: true,
		actions: [{ op: 'click', ref: 'B1' }, { op: 'copy' }],
		compare: ['selection', 'copy', 'highlight', 'caret'],
	},
	{
		id: 'xlsx-shift-click',
		format: 'xlsx',
		ready: false,
		allowList: '#29',
		todo: 'Shift-click → extendSelection (upstream wirePointer).',
		fixture: 'fixtures/bidi/arabic-hebrew-mixed.xlsx',
		live: true,
		actions: [
			{ op: 'click', ref: 'A1' },
			{ op: 'shift-click', ref: 'C1' },
			{ op: 'copy' },
		],
		compare: ['selection', 'copy', 'highlight'],
	},
	{
		id: 'xlsx-drag',
		format: 'xlsx',
		ready: false,
		allowList: '#29',
		todo: 'Pointer drag → extendSelection with capture (upstream wirePointer).',
		fixture: 'fixtures/bidi/arabic-hebrew-mixed.xlsx',
		live: true,
		actions: [{ op: 'drag', from: 'A1', to: 'E1' }, { op: 'copy' }],
		compare: ['selection', 'copy', 'highlight'],
	},
	{
		id: 'xlsx-keyboard-extend',
		format: 'xlsx',
		ready: false,
		allowList: '#29',
		todo: 'Keyboard events → selection.move(nav, shift) (upstream readyKeyDown); element must push Keyboard.',
		fixture: 'fixtures/bidi/arabic-hebrew-mixed.xlsx',
		live: true,
		actions: [
			{ op: 'select-ref', ref: 'A1' },
			{ op: 'keyboard-extend', key: 'ArrowRight', count: 2 },
			{ op: 'copy' },
		],
		compare: ['selection', 'copy', 'highlight', 'caret'],
	},
	{
		id: 'xlsx-select-all',
		format: 'xlsx',
		ready: false,
		allowList: '#29',
		todo: 'Ctrl+A / corner click → region or GridSelection.all().',
		fixture: 'fixtures/bidi/arabic-hebrew-mixed.xlsx',
		live: true,
		actions: [{ op: 'select-ref', ref: 'A1' }, { op: 'select-all' }, { op: 'copy' }],
		compare: ['selection', 'copy'],
	},
	// --- pptx ---
	{
		id: 'pptx-text-span',
		format: 'pptx',
		ready: false,
		allowList: '#27',
		todo: 'Slide text selection + shell copy empty until pptx clipboard/inline editor (#27).',
		fixture: 'fixtures/bidi/arabic-hebrew-slides.pptx',
		live: true,
		actions: [{ op: 'click', ref: '0' }, { op: 'copy' }],
		compare: ['selection', 'copy', 'caret'],
	},
	// --- docx (after #158 / <nitro-docx>) ---
	{
		id: 'docx-text-span',
		format: 'docx',
		ready: false,
		allowList: '#158',
		todo: 'Cover once <nitro-docx> lands on main (#158 / #23); ProseMirror coordsAtPos caret.',
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
