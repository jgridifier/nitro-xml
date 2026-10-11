// SPDX-License-Identifier: MIT
/**
 * Build informational pptx visual cases from the parity fixture corpus (#27).
 *
 * Same redistributable S/M pool as `parity/corpus.ts` selectCorpus (skip
 * on-demand L/XL). Each case compares slide 0 of `<nitro-pptx>` vs upstream
 * `renderSlideStage` at DPR 1/2. Mismatches are reported, not hard-failed.
 *
 * Caps: `VISUAL_PPTX_MAX` (default unlimited). Print-to-PDF is skipped for
 * these cases until painters converge (DPR numbers drive the fidelity work).
 */

import { existsSync, readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');

/**
 * @returns {import('./registry.mjs').VisualCase[]}
 */
export function pptxVisualCorpus() {
	const manifest = JSON.parse(readFileSync(join(ROOT, 'fixtures/manifest.json'), 'utf8'));
	const tiers = new Set(['S', 'M']);
	const rows = manifest.fixtures.filter(
		(f) =>
			f.format === 'pptx' &&
			f.redistributable &&
			!f.on_demand &&
			tiers.has(f.tier) &&
			f.path &&
			existsSync(join(ROOT, f.path)),
	);
	rows.sort((a, b) => String(a.path).localeCompare(String(b.path)));

	const maxRaw = process.env.VISUAL_PPTX_MAX;
	const max = maxRaw != null && maxRaw !== '' ? Number(maxRaw) : Infinity;
	const limited = Number.isFinite(max) && max >= 0 ? rows.slice(0, max) : rows;

	return limited.map((f) => ({
		id: `pptx-${f.id}`,
		format: 'pptx',
		kind: 'pptx-viewer',
		ready: true,
		fixture: f.path,
		slide: 0,
		// Standard 16:9 stage; painters fit the authored slide into this box.
		width: 960,
		height: 540,
		rust: true,
		informational: true,
		// Print raster is expensive at corpus scale; DPR 1/2 is the fidelity signal.
		skipPrint: true,
	}));
}
