// SPDX-License-Identifier: MIT
/**
 * Build pptx visual cases from the parity fixture corpus (#27).
 *
 * Same redistributable S/M pool as `parity/corpus.ts` selectCorpus (skip
 * on-demand L/XL). Each case compares slide 0 of `<nitro-pptx>` vs upstream
 * `renderSlideStage` at DPR 1/2.
 *
 * Cases that have reached ≤0.1% on both DPRs are hard (fail the gate);
 * the rest stay informational until painters converge. Print-to-PDF is
 * skipped until on-screen painters settle.
 *
 * Caps: `VISUAL_PPTX_MAX` (default unlimited).
 */

import { existsSync, readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');

/**
 * Fixture ids (manifest `id`, without `pptx-` prefix) that are hard at the
 * 0.1% pptx bound. Measured on the Liberation font pin + style-matrix/blip
 * fidelity branch (#27 harden). Add only when both DPR1 and DPR2 are ≤0.1%.
 */
export const PPTX_HARD_FIXTURE_IDS = new Set([
	'gen-pptx-activex',
	'gen-pptx-equation',
	'gen-pptx-model3d',
	'gen-pptx-notes-print',
	'gen-pptx-placeholder',
	'gen-pptx-slides-10',
	'gen-pptx-slides-10-b',
	'gen-pptx-slides-10-c',
	'gen-pptx-slides-50',
	'gen-pptx-slides-50-b',
	'gen-pptx-slides-50-c',
	'gen-pptx-zoom',
	'real-pptx-poi-51187',
	'real-pptx-poi-EmbeddedAudio',
	'real-pptx-poi-bug59273',
	'real-pptx-python-pptx-act-props',
	'upstream-4a678fd44484',
	'upstream-7cc22f8e5927',
	'upstream-9f78c22429d9',
]);

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

	const idFilter = process.env.VISUAL_PPTX_IDS;
	const idSet = idFilter
		? new Set(idFilter.split(',').map((s) => s.trim()).filter(Boolean))
		: null;
	const filtered = idSet ? rows.filter((f) => idSet.has(f.id)) : rows;
	const maxRaw = process.env.VISUAL_PPTX_MAX;
	const max = maxRaw != null && maxRaw !== '' ? Number(maxRaw) : Infinity;
	const limited = Number.isFinite(max) && max >= 0 ? filtered.slice(0, max) : filtered;

	return limited.map((f) => {
		const hard = PPTX_HARD_FIXTURE_IDS.has(f.id);
		return {
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
			informational: !hard,
			// Print raster is expensive at corpus scale; DPR 1/2 is the fidelity signal.
			skipPrint: true,
		};
	});
}
