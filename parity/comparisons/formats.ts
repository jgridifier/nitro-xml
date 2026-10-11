// SPDX-License-Identifier: MIT
/**
 * Format view-IR comparisons. Registered now so #15 / #20 / #24 / #25 can flip
 * `ready` when their Rust dump exists; `--ci` skips until then.
 */

import type { Comparison, Manifest, ParityFormat } from '../types.ts';
import { selectCorpus } from '../corpus.ts';
import { dumpXlsxViewIr } from '../oracle/xlsx.ts';
import { dumpDocxViewIr } from '../oracle/docx.ts';
import { dumpPptxViewIr } from '../oracle/pptx.ts';
import { dumpFormatViewIrRust } from '../rust/dump.ts';

function formatComparison(
	id: Extract<ParityFormat, 'xlsx' | 'docx' | 'pptx'>,
	label: string,
	todo: string,
	oracle: (path: string) => Promise<unknown>,
	/** Flip to true in the PR that lands the Rust view-IR dump. */
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

export const xlsxComparison = formatComparison(
	'xlsx',
	'xlsx view IR (CellItem / CellView)',
	'#15',
	dumpXlsxViewIr,
	false,
);

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
