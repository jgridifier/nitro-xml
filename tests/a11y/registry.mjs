// SPDX-License-Identifier: MIT
/**
 * Accessibility gate cases (#42).
 *
 * xlsx + pptx: differential axe / AX-tree / keyboard vs upstream TS hosts.
 * docx: registered, not ready until #158 (Print Layout + nitro-docx) lands.
 */

/** @typedef {'xlsx'|'docx'|'pptx'} A11yFormat */

/**
 * @typedef {object} A11yCase
 * @property {string} id
 * @property {A11yFormat} format
 * @property {boolean} ready
 * @property {string} [todo]
 * @property {string} [fixture] repo-relative path
 * @property {number} [width]
 * @property {number} [height]
 * @property {number} [slide] pptx slide index
 */

/** @type {A11yCase[]} */
export const A11Y_CASES = [
	{
		id: 'xlsx-poi-45544',
		format: 'xlsx',
		ready: true,
		fixture: 'fixtures/real/xlsx/poi-45544.xlsx',
		width: 800,
		height: 480,
	},
	{
		id: 'pptx-poi-51187',
		format: 'pptx',
		ready: true,
		fixture: 'fixtures/real/pptx/poi-51187.pptx',
		width: 960,
		height: 540,
		slide: 0,
	},
	{
		id: 'docx-print-layout',
		format: 'docx',
		ready: false,
		todo: 'Register when #158 (docx Print Layout + nitro-docx) lands.',
	},
];

export function readyCases() {
	return A11Y_CASES.filter((c) => c.ready);
}
