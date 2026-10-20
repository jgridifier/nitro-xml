// SPDX-License-Identifier: MIT
//! PNG pixel compare for the first-paint race (loads pixelmatch from parity/visual).

import { createRequire } from 'node:module';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '..', '..', '..', '..');
const visualRoot = join(repo, 'parity', 'visual');
const require = createRequire(join(visualRoot, 'package.json'));
const { PNG } = require('pngjs');
const pixelmatch = require('pixelmatch');

/**
 * @param {Buffer} aPng
 * @param {Buffer} bPng
 * @param {{ limit: number, diffPath?: string }} opts limit = max mismatch fraction
 */
export function comparePng(aPng, bPng, opts) {
	const imgA = PNG.sync.read(aPng);
	const imgB = PNG.sync.read(bPng);
	if (imgA.width !== imgB.width || imgA.height !== imgB.height) {
		return {
			ok: false,
			mismatched: imgA.width * imgA.height,
			total: imgA.width * imgA.height,
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
	const ratio = total ? mismatched / total : 0;
	const ok = ratio <= opts.limit;
	if (mismatched > 0 && opts.diffPath) {
		mkdirSync(dirname(opts.diffPath), { recursive: true });
		writeFileSync(opts.diffPath, PNG.sync.write(diff));
	}
	return { ok, mismatched, total, ratio };
}
