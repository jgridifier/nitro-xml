// SPDX-License-Identifier: MIT
/** pixelmatch wrapper for PNG buffers. */

import { PNG } from 'pngjs';
import pixelmatch from 'pixelmatch';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import { mismatchRatio, withinThreshold } from './thresholds.mjs';

/**
 * @param {Buffer} aPng
 * @param {Buffer} bPng
 * @param {{ format: string, diffPath?: string }} opts
 */
export function comparePngBuffers(aPng, bPng, opts) {
	const imgA = PNG.sync.read(aPng);
	const imgB = PNG.sync.read(bPng);
	if (imgA.width !== imgB.width || imgA.height !== imgB.height) {
		return {
			ok: false,
			mismatched: Math.max(imgA.width * imgA.height, imgB.width * imgB.height),
			total: Math.max(imgA.width * imgA.height, imgB.width * imgB.height),
			ratio: 1,
			reason: `size mismatch ${imgA.width}x${imgA.height} vs ${imgB.width}x${imgB.height}`,
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
	const ok = withinThreshold(mismatched, total, opts.format);
	if (opts.diffPath && mismatched > 0) {
		mkdirSync(dirname(opts.diffPath), { recursive: true });
		writeFileSync(opts.diffPath, PNG.sync.write(diff));
	}
	return { ok, mismatched, total, ratio, width, height };
}
