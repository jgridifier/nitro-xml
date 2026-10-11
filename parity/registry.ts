// SPDX-License-Identifier: MIT
/**
 * Comparison registry. Format parsers register (or flip `ready`) here when their
 * Rust view-IR dump lands — see comparisons/formats.ts.
 */

import type { Comparison, ParityFormat } from './types.ts';
import { xmlComparison } from './comparisons/xml.ts';
import { opcComparison } from './comparisons/opc.ts';
import { xlsxComparison, docxComparison, pptxComparison } from './comparisons/formats.ts';

const ALL: Comparison[] = [
	xmlComparison,
	opcComparison,
	xlsxComparison,
	docxComparison,
	pptxComparison,
];

export function listComparisons(): Comparison[] {
	return ALL.slice();
}

export function getComparison(id: string): Comparison {
	const c = ALL.find((x) => x.id === id);
	if (!c) {
		throw new Error(
			`unknown comparison '${id}'. Registered: ${ALL.map((x) => x.id).join(', ')}`,
		);
	}
	return c;
}

export function readyComparisons(): Comparison[] {
	return ALL.filter((c) => c.ready);
}

export type { ParityFormat };
