// SPDX-License-Identifier: MIT
/**
 * TS oracle: docx view IR = LayoutResult from layoutDocumentModel + createFakeMeasurer.
 * Both engines must use the deterministic fake measurer so fonts never cause a diff.
 */

import { readFileSync } from 'node:fs';
import { upstreamDir } from '../paths.ts';

export async function dumpDocxViewIr(fixturePath: string): Promise<unknown> {
	const up = upstreamDir();
	const { loadDocx } = await import(`${up}/src/core/docx/parse.ts`);
	const { layoutDocumentModel } = await import(`${up}/src/core/docx/layout/layout.ts`);
	const { createFakeMeasurer } = await import(`${up}/src/core/docx/layout/measure.ts`);

	const bytes = readFileSync(fixturePath);
	const loaded = await loadDocx(bytes);
	const result = layoutDocumentModel(loaded.model, createFakeMeasurer());
	return {
		format: 'docx',
		pages: result.pages,
		approximations: result.approximations ?? [],
	};
}
