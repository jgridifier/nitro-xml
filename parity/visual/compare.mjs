// SPDX-License-Identifier: MIT
/** pixelmatch wrapper for PNG buffers. */

import { PNG } from 'pngjs';
import pixelmatch from 'pixelmatch';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import { mismatchRatio, withinThreshold, THRESHOLDS } from './thresholds.mjs';

/**
 * @param {Buffer} aPng
 * @param {Buffer} bPng
 * @param {{ format: string, diffPath?: string }} opts
 */
function writeDiffPng(diffPath, png) {
	if (!diffPath) return;
	mkdirSync(dirname(diffPath), { recursive: true });
	writeFileSync(diffPath, PNG.sync.write(png));
}

/**
 * Write a diff PNG for every mismatched comparison (including informational cases).
 * Size mismatches get a red canvas at the larger extent so CI always has a PNG.
 */
export function comparePngBuffers(aPng, bPng, opts) {
	const imgA = PNG.sync.read(aPng);
	const imgB = PNG.sync.read(bPng);
	if (imgA.width !== imgB.width || imgA.height !== imgB.height) {
		const width = Math.max(imgA.width, imgB.width);
		const height = Math.max(imgA.height, imgB.height);
		const diff = new PNG({ width, height });
		for (let i = 0; i < diff.data.length; i += 4) {
			diff.data[i] = 255;
			diff.data[i + 1] = 0;
			diff.data[i + 2] = 0;
			diff.data[i + 3] = 255;
		}
		writeDiffPng(opts.diffPath, diff);
		return {
			ok: false,
			mismatched: width * height,
			total: width * height,
			ratio: 1,
			reason: `size mismatch ${imgA.width}x${imgA.height} vs ${imgB.width}x${imgB.height}`,
			width,
			height,
		};
	}
	const { width, height } = imgA;
	const diff = new PNG({ width, height });
	const mismatched = pixelmatch(imgA.data, imgB.data, diff.data, width, height, {
		threshold: 0.1,
		includeAA: false,
	});
	const total = width * height;
	const ratio = mismatchRatio(mismatched, total);
	const limit = opts.threshold ?? THRESHOLDS[opts.format];
	const ok =
		limit === undefined
			? withinThreshold(mismatched, total, opts.format)
			: mismatched / total <= limit;
	if (mismatched > 0) {
		writeDiffPng(opts.diffPath, diff);
	}
	return { ok, mismatched, total, ratio, width, height };
}
