// SPDX-License-Identifier: MIT
/**
 * Bidi / RTL gate cases (#44).
 *
 * xlsx: visual (view-IR host from #45) + logical-order shell copy.
 * pptx: logical model text (shell copy is empty until pptx clipboard lands).
 * docx: logical fragment text from LayoutResult dumps (#23).
 */

/** @typedef {'xlsx'|'docx'|'pptx'} BidiFormat */

/**
 * @typedef {object} BidiCase
 * @property {string} id
 * @property {BidiFormat} format
 * @property {boolean} ready
 * @property {string} [todo]
 * @property {string} [fixture] repo-relative path
 * @property {string[]} [copyRanges] A1 ranges for shell copy (xlsx)
 * @property {string[]} [expectedCopy] expected logical-order TSV per range
 * @property {boolean} [visual] run visual pixelmatch via #45 host
 * @property {number} [width]
 * @property {number} [height]
 */

/** Logical-order strings baked into fixtures/bidi/generate.py. */
export const LOGICAL = Object.freeze({
	AR: 'مرحبا بالعالم',
	HE: 'שלום עולם',
	MIXED: 'Hello שלום world مرحبا',
	AR_NUM: 'السعر 1234 ريال',
	HE_NUM: 'מחיר 56 ₪',
});

/** @type {BidiCase[]} */
export const BIDI_CASES = [
	{
		id: 'xlsx-arabic-hebrew-mixed',
		format: 'xlsx',
		ready: true,
		fixture: 'fixtures/bidi/arabic-hebrew-mixed.xlsx',
		visual: true,
		width: 800,
		height: 240,
		copyRanges: ['A1', 'B1', 'C1', 'D1', 'E1', 'A1:E1'],
		expectedCopy: [
			LOGICAL.AR,
			LOGICAL.HE,
			LOGICAL.MIXED,
			LOGICAL.AR_NUM,
			LOGICAL.HE_NUM,
			[LOGICAL.AR, LOGICAL.HE, LOGICAL.MIXED, LOGICAL.AR_NUM, LOGICAL.HE_NUM].join('\t'),
		],
	},
	{
		id: 'xlsx-rtl-sheet',
		format: 'xlsx',
		ready: true,
		fixture: 'fixtures/bidi/rtl-sheet.xlsx',
		visual: true,
		width: 800,
		height: 240,
		copyRanges: ['A1', 'B1', 'C1', 'A1:C1'],
		expectedCopy: [
			LOGICAL.AR,
			LOGICAL.HE,
			LOGICAL.MIXED,
			[LOGICAL.AR, LOGICAL.HE, LOGICAL.MIXED].join('\t'),
		],
	},
	{
		id: 'pptx-arabic-hebrew-slides',
		format: 'pptx',
		ready: true,
		fixture: 'fixtures/bidi/arabic-hebrew-slides.pptx',
		// Visual slide paint host lands with the pptx painter; shell copy is
		// still empty (xlsx-only today). Gate logical model text instead.
		visual: false,
		todo: 'Visual + shell copy when pptx paint/clipboard hosts land (#27).',
	},
	{
		id: 'docx-rtl-paragraphs',
		format: 'docx',
		ready: true,
		fixture: 'fixtures/bidi/arabic-hebrew-paragraphs.docx',
		visual: true,
		width: 800,
		height: 600,
	},
];

export function readyCases() {
	return BIDI_CASES.filter((c) => c.ready);
}
