// SPDX-License-Identifier: MIT
// Catches: a dropped pptx text box hiding under the 0.5% slide budget (Jared, Oct 16 2026).
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { PNG } from 'pngjs';
import { comparePngBuffers } from '../compare.mjs';
import { PPTX_TILE_GUARD, THRESHOLDS } from '../thresholds.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));

test('blanking the subtitle box of a hard deck stays under 0.5% overall but trips the DPR2 tile guard', () => {
	// Real upstream render of hard deck real-pptx-poi-bug62513, slide 0, DPR2 (1920x1080).
	const buf = readFileSync(join(HERE, 'fixtures/bug62513-ts-dpr2.png'));
	const opts = { format: 'pptx', threshold: THRESHOLDS.pptx, dpr: 2, tileGuard: PPTX_TILE_GUARD };

	// Control: the untouched render passes.
	const same = comparePngBuffers(buf, buf, opts);
	expect(same.ok).toBe(true);

	// Simulate Rust dropping the "November 24, 2011" subtitle shape: paint its box white.
	const img = PNG.sync.read(buf);
	for (let y = 610; y < 700; y++) {
		for (let x = 430; x < 1030; x++) {
			const i = (y * img.width + x) * 4;
			img.data[i] = img.data[i + 1] = img.data[i + 2] = img.data[i + 3] = 255;
		}
	}
	const dropped = comparePngBuffers(buf, PNG.sync.write(img), opts);
	// The whole text box costs less than the slide budget, so the overall check alone passes...
	expect(dropped.ratio).toBeLessThan(THRESHOLDS.pptx);
	expect(dropped.ratio).toBeGreaterThan(0.001); // ...and would have failed the old 0.1% bound.
	// ...and only the local guard catches it.
	expect(dropped.worstTile).toBeGreaterThan(PPTX_TILE_GUARD.maxRatioByDpr[2]);
	// At DPR1 the same drop is ~14% of a tile, under the 20% DPR1 limit: DPR2 carries the guard.
	expect(dropped.worstTile).toBeLessThan(PPTX_TILE_GUARD.maxRatioByDpr[1]);
	expect(dropped.ok).toBe(false);
});
