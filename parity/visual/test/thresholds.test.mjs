// SPDX-License-Identifier: MIT
import { test, expect } from 'bun:test';
import { withinThreshold, mismatchRatio, THRESHOLDS } from '../thresholds.mjs';

test('xlsx 0.1% threshold rejects just-over and accepts just-under', () => {
	// Catches: off-by-factor-of-100 when converting PRODUCT_SPEC percent limits to fractions.
	const total = 1_000_000;
	expect(THRESHOLDS.xlsx).toBe(0.001);
	expect(withinThreshold(1000, total, 'xlsx')).toBe(true); // exactly 0.1%
	expect(withinThreshold(1001, total, 'xlsx')).toBe(false);
	expect(mismatchRatio(1000, total)).toBeCloseTo(0.001);
});

test('docx and pptx share the 0.5% page/slide bound', () => {
	// Catches: docx/pptx accidentally wired to the tighter xlsx viewport limit.
	expect(THRESHOLDS.docx).toBe(0.005);
	expect(THRESHOLDS.pptx).toBe(0.005);
	expect(withinThreshold(5, 1000, 'docx')).toBe(true);
	expect(withinThreshold(6, 1000, 'pptx')).toBe(false);
});
