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
	'bidi-arabic-hebrew-slides',
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
	'real-pptx-poi-SimpleMacro',
	'real-pptx-poi-bug59273',
	'real-pptx-poi-bug62513',
	'real-pptx-python-pptx-act-props',
	'real-pptx-python-pptx-shp-movie-props',
	'real-pptx-python-pptx-test_slides',
	'real-pptx-python-pptx-txt-font-typeface',
	'upstream-091abf411e47',
	'upstream-0c4571a8c30e',
	'upstream-0c7e49afbe04',
	'upstream-0d9a8eaf2de4',
	'upstream-165297e1d546',
	'upstream-169be9b132ef',
	'upstream-1abd9b12cb19',
	'upstream-1b3ce5863e04',
	'upstream-21343018c511',
	'upstream-25297a56a758',
	'upstream-25ceccfae63d',
	'upstream-2789ff8d6772',
	'upstream-2b363b2c9b04',
	'upstream-3858c1c9d869',
	'upstream-38aa694fb4a5',
	'upstream-38fb1b1e8f76',
	'upstream-4647c93fad68',
	'upstream-3e787d01c5f7',
	'upstream-49a48af6d18d',
	'upstream-4a678fd44484',
	'upstream-51edb06a949d',
	'upstream-55531661e868',
	'upstream-57de458fae91',
	'upstream-64fe025bb96d',
	'upstream-6fbe45df1cb3',
	'upstream-717fb7735aa7',
	'upstream-73913b502a2e',
	'upstream-7bb23ba0b604',
	'upstream-7cc22f8e5927',
	'upstream-88e14cba7830',
	'upstream-8bbf3d67bd63',
	'upstream-913ecd7c7c8a',
	'upstream-959ab2f5bac7',
	'upstream-9f78c22429d9',
	'upstream-a10c83ff4127',
	'upstream-a344b2f3f55d',
	'upstream-a4b1b68c66c7',
	'upstream-a61f509fcab9',
	'upstream-a734ca765129',
	'upstream-a8fa453c5023',
	'upstream-af1bd691bad1',
	'upstream-b2b71a866c2e',
	'upstream-b6c7dfcf4428',
	'upstream-bd16f101c6d5',
	'upstream-be4aaa835c68',
	'upstream-d173badb4435',
	'upstream-d34691128d6a',
	'upstream-d38cba8bfc42',
	'upstream-d64f9e91a815',
	'upstream-db668adb7e73',
	'upstream-dbc4432fdc19',
	'upstream-e3dab827b836',
	'upstream-e5e3a918a510',
	'upstream-ea86d831d24d',
	'upstream-f374280d2eb7',
	'upstream-f3a23e347a28',
	'upstream-f7015eebee2e',
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
