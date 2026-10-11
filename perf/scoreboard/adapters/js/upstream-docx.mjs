// SPDX-License-Identifier: MIT
// Upstream ooxml TS (reference), docx: the entry the upstream Word UI uses
// (src/ui/src/docx/component.ts -> ooxml-core/docx/load loadDocument).
import { loadDocument } from 'ooxml-core/docx/load';

export const parse = (bytes) => loadDocument(bytes);

const HIDDEN = new Set(['delete', 'moveFrom']);
const paraText = (p) =>
	p.runs
		.filter((r) => !(r.revision && HIDDEN.has(r.revision.kind)))
		.map((r) => r.text ?? '')
		.join('');

export function extract(loaded) {
	const paragraphs = [];
	for (const block of loaded.model.blocks) {
		if (block.type === 'paragraph') paragraphs.push(paraText(block));
		else if (block.type === 'table') {
			for (const row of block.rows) for (const cell of row) for (const p of cell.paragraphs) paragraphs.push(paraText(p));
		}
	}
	return { paragraphs };
}
