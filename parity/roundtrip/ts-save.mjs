// SPDX-License-Identifier: MIT
/**
 * TS oracle no-op save: open → save untouched for each package family (#49).
 * Uses pinned upstream (e66aaec) via path imports — test-only, never shipped.
 */

import { join } from 'node:path';
import { familyOf, packageKind, ROOT } from './corpus.mjs';

function upstreamRoot() {
	return process.env.UPSTREAM_DIR || join(ROOT, 'upstream');
}

function toArrayBuffer(u8) {
	return u8.buffer.slice(u8.byteOffset, u8.byteOffset + u8.byteLength);
}

/**
 * @param {Uint8Array} bytes
 * @param {import('./corpus.mjs').FixtureRow} row
 * @returns {Promise<Uint8Array>}
 */
export async function tsNoopSave(bytes, row) {
	const kind = packageKind(row);
	const fam = familyOf(kind);
	const up = upstreamRoot();

	if (fam === 'xlsx') {
		const { loadXlsx } = await import(`${up}/src/core/xlsx/read/load.ts`);
		const { saveXlsx } = await import(`${up}/src/core/xlsx/write/save.ts`);
		const wb = await loadXlsx(bytes);
		// Preserve macro-enabled format so vbaProject.bin is carried (#49).
		if (kind === 'xlsm' || kind === 'xltm') wb.format = 'xlsm';
		return await saveXlsx(wb);
	}

	if (fam === 'docx') {
		const { loadDocx } = await import(`${up}/src/core/docx/parse.ts`);
		const loaded = await loadDocx(bytes);
		// Unchanged model → saveDocx returns original bytes (upstream contract).
		return await loaded.save();
	}

	if (fam === 'pptx') {
		const { PptxHandler } = await import(`${up}/src/core/pptx/core/PptxHandler.ts`);
		const handler = new PptxHandler();
		const data = await handler.load(toArrayBuffer(bytes));
		return await handler.save(data.slides, {
			headerFooter: data.headerFooter,
			tableStyles: data.tableStyleMap,
		});
	}

	throw new Error(`unsupported package kind ${kind} for ${row.id}`);
}
