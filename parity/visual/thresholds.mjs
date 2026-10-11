// SPDX-License-Identifier: MIT
/** PRODUCT_SPEC.md §5 visual thresholds (fraction of pixels that may differ). */

export const THRESHOLDS = Object.freeze({
	xlsx: 0.001, // ≤0.1% per viewport
	docx: 0.005, // ≤0.5% per page
	/** pptx hard cases target the xlsx-style 0.1% slide bound (#27 harden / #150 method). */
	pptx: 0.001, // ≤0.1% per slide
	/** Chart S1 is hosted as a fixed viewport (xlsx-hosted charts use the xlsx bound). */
	chart: 0.001,
});

/** True when mismatched/total is within the format's allowed fraction. */
export function withinThreshold(mismatched, total, format) {
	if (total <= 0) return mismatched === 0;
	const limit = THRESHOLDS[format];
	if (limit === undefined) {
		throw new Error(`unknown visual format: ${format}`);
	}
	return mismatched / total <= limit;
}

export function mismatchRatio(mismatched, total) {
	if (total <= 0) return mismatched === 0 ? 0 : 1;
	return mismatched / total;
}
