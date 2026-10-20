// SPDX-License-Identifier: MIT
/** Size-tier fixtures for the first-paint race (#41). */

export const TIERS = [
	{ format: 'xlsx', tier: 'S', rel: 'fixtures/generated/xlsx/cells_1k.xlsx' },
	{ format: 'xlsx', tier: 'M', rel: 'fixtures/generated/xlsx/cells_100k.xlsx' },
	{ format: 'xlsx', tier: 'L', rel: 'fixtures/generated/xlsx/cells_1m.xlsx' },
	{ format: 'xlsx', tier: 'XL', rel: 'fixtures/generated/xlsx/xl_a.xlsx' },
	// TODO(#23): docx tiers when Stage 4 docx viewer gate lands.
	{ format: 'pptx', tier: 'S', rel: 'fixtures/generated/pptx/slides_10.pptx' },
	{ format: 'pptx', tier: 'M', rel: 'fixtures/generated/pptx/slides_50.pptx' },
	{ format: 'pptx', tier: 'L', rel: 'fixtures/generated/pptx/slides_200.pptx' },
	{ format: 'pptx', tier: 'XL', rel: 'fixtures/generated/pptx/xl_a.pptx' },
];

/** xlsx ≤0.1% / pptx ≤0.5% — PRODUCT_SPEC §5 (fraction). */
export const PIXEL_LIMIT = Object.freeze({
	xlsx: 0.001,
	pptx: 0.005,
	docx: 0.005,
});
