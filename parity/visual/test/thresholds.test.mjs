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

test('pptx slide budget is 0.5% while xlsx stays at 0.1%', () => {
	// Catches: the pptx-only relaxation (Jared, Oct 16 2026) leaking into xlsx, or pptx
	// left on 0.1% (which makes the tile guard pointless).
	expect(THRESHOLDS.pptx).toBe(0.005);
	expect(withinThreshold(5, 1000, 'pptx')).toBe(true); // exactly 0.5%
	expect(withinThreshold(6, 1000, 'pptx')).toBe(false);
	expect(withinThreshold(2, 1000, 'xlsx')).toBe(false);
});
