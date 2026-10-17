// SPDX-License-Identifier: MIT
/** PRODUCT_SPEC.md §5 visual thresholds (fraction of pixels that may differ). */

export const THRESHOLDS = Object.freeze({
	xlsx: 0.001, // ≤0.1% per viewport
	docx: 0.005, // ≤0.5% per page
	/**
	 * pptx hard cases: ≤0.5% per slide overall (Jared, Oct 16 2026), paired with
	 * PPTX_TILE_GUARD so a dropped shape cannot hide under the larger budget.
	 */
	pptx: 0.005, // ≤0.5% per slide + tile guard
	/** Chart S1 is hosted as a fixed viewport (xlsx-hosted charts use the xlsx bound). */
	chart: 0.001,
});

/**
 * Local guard for pptx: slide a window (`tile` CSS px, scaled by DPR, step
 * `stride`) over the pixelmatch diff and fail when any window's mismatched
 * fraction exceeds the limit for that DPR. Calibrated on the Oct 16 corpus
 * (CI run 38991507468): the worst already-hard decks peak at 18.75% of a
 * window at DPR1 (upstream-51edb06a949d hyperlink run) and 10.9% at DPR2
 * (upstream-0c4571a8c30e), so these are the tightest limits that keep every
 * 0.1%-era hard deck green. DPR2 is tighter because glyph AA noise spreads
 * thinner there while a dropped shape covers the same fraction at both DPRs.
 */
export const PPTX_TILE_GUARD = Object.freeze({
	tile: 32,
	stride: 16,
	maxRatio: 0.2,
	maxRatioByDpr: Object.freeze({ 1: 0.2, 2: 0.12 }),
});

/** Tile-guard limit for a DPR (falls back to `maxRatio`). */
export function tileLimit(guard, dpr = 1) {
	return guard.maxRatioByDpr?.[dpr] ?? guard.maxRatio;
}

/**
 * Worst window of a mismatch mask (1 = differing pixel).
 * @param {Uint8Array} mask width*height
 * @returns {{ ratio: number, x: number, y: number }} x/y in CSS px (top-left)
 */
export function worstTile(mask, width, height, dpr = 1, guard = PPTX_TILE_GUARD) {
	const t = Math.max(1, Math.round(guard.tile * dpr));
	const s = Math.max(1, Math.round(guard.stride * dpr));
	const W = width + 1;
	// Summed-area table so each window costs O(1).
	const sat = new Uint32Array(W * (height + 1));
	for (let y = 0; y < height; y++) {
		let row = 0;
		for (let x = 0; x < width; x++) {
			row += mask[y * width + x];
			sat[(y + 1) * W + x + 1] = sat[y * W + x + 1] + row;
		}
	}
	const tw = Math.min(t, width);
	const th = Math.min(t, height);
	const area = tw * th;
	let best = { ratio: 0, x: 0, y: 0 };
	if (area === 0) return best;
	const ys = [];
	for (let y = 0; y + th <= height; y += s) ys.push(y);
	if (ys[ys.length - 1] !== height - th) ys.push(height - th);
	const xs = [];
	for (let x = 0; x + tw <= width; x += s) xs.push(x);
	if (xs[xs.length - 1] !== width - tw) xs.push(width - tw);
	for (const y of ys) {
		for (const x of xs) {
			const n =
				sat[(y + th) * W + x + tw] - sat[y * W + x + tw] - sat[(y + th) * W + x] + sat[y * W + x];
			const ratio = n / area;
			if (ratio > best.ratio) best = { ratio, x: Math.round(x / dpr), y: Math.round(y / dpr) };
		}
	}
	return best;
}

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
