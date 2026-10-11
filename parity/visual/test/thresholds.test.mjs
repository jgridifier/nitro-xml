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

test('docx keeps the 0.5% page bound; pptx hard cases use the 0.1% slide bound', () => {
	// Catches: pptx harden (#27) accidentally left on the old 0.5% slide limit,
	// or docx tightened to the xlsx/pptx 0.1% viewport bound.
	expect(THRESHOLDS.docx).toBe(0.005);
	expect(THRESHOLDS.pptx).toBe(0.001);
	expect(withinThreshold(5, 1000, 'docx')).toBe(true);
	expect(withinThreshold(1, 1000, 'pptx')).toBe(true); // exactly 0.1%
	expect(withinThreshold(2, 1000, 'pptx')).toBe(false);
});
