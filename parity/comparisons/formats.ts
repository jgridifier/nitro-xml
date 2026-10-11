// SPDX-License-Identifier: MIT
/**
 * Format comparisons still waiting on their Rust dump.
 * xlsx worksheet slice is ready — see comparisons/xlsx.ts (#16).
 * docx styles/numbering/theme/settings is ready — see comparisons/docx.ts (#20).
 * Flip `ready` here when pptx (and later xlsx CellItem / docx LayoutResult) land.
 */

import type { Comparison, Manifest, ParityFormat } from '../types.ts';
import { selectCorpus } from '../corpus.ts';
import { dumpPptxViewIr } from '../oracle/pptx.ts';
import { dumpFormatViewIrRust } from '../rust/dump.ts';

function formatComparison(
	id: Extract<ParityFormat, 'pptx'>,
	label: string,
	todo: string,
	oracle: (path: string) => Promise<unknown>,
	ready = false,
): Comparison {
	return {
		id,
		label,
		ready,
		todo,
		oracle,
		async rust(fixturePath: string) {
			return dumpFormatViewIrRust(id, fixturePath, todo);
		},
		selectFixtures(manifest: Manifest) {
			return selectCorpus(manifest, {
				format: id,
				feature: 'parity-corpus',
				skipOnDemand: true,
				tiers: ['S', 'M'],
			});
		},
	};
}

export const pptxComparison = formatComparison(
	'pptx',
	'pptx view IR (PptxElement trees)',
	'#24',
	dumpPptxViewIr,
	false,
);
