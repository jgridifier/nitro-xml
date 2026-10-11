// SPDX-License-Identifier: MIT
/**
 * Format comparisons still waiting on their Rust dump.
 * xlsx workbook slice is ready — see comparisons/xlsx.ts (#15 / #131).
 * Flip `ready` here when docx/pptx (and later xlsx CellItem view IR) land.
 */

import type { Comparison, Manifest, ParityFormat } from '../types.ts';
import { selectCorpus } from '../corpus.ts';
import { dumpDocxViewIr } from '../oracle/docx.ts';
import { dumpPptxViewIr } from '../oracle/pptx.ts';
import { dumpFormatViewIrRust } from '../rust/dump.ts';

function formatComparison(
	id: Extract<ParityFormat, 'docx' | 'pptx'>,
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

export const docxComparison = formatComparison(
	'docx',
	'docx view IR (LayoutResult + fake measurer)',
	'#20',
	dumpDocxViewIr,
	false,
);

export const pptxComparison = formatComparison(
	'pptx',
	'pptx view IR (PptxElement trees)',
	'#24',
	dumpPptxViewIr,
	false,
);
