// SPDX-License-Identifier: MIT
// docx-preview 0.4.1: parseAsync (the parse half of renderAsync), default options
// (https://github.com/VolodymyrBaydalka/docxjs#api). Parsing does not depend on the render options.
import { parseAsync } from 'docx-preview';

export const parse = (bytes) => parseAsync(bytes, {});

// Content that is not body text in the reference: deletions, field instructions, drawings
// (text boxes), maths and comments.
const SKIP = new Set(['deleted', 'deletedText', 'instruction', 'drawing', 'vmlPicture', 'vmlElement', 'comment', 'commentReference', 'image']);

function text(node) {
	if (!node || SKIP.has(node.type) || node.type?.startsWith('mml')) return '';
	if (node.type === 'text' || node.type === 'symbol') return node.text ?? node.char ?? '';
	if (node.type === 'break') return node.break === 'lastRenderedPageBreak' ? '' : ' ';
	if (node.type === 'tab') return ' ';
	if (node.type === 'noBreakHyphen') return '‑';
	return (node.children ?? []).map(text).join('');
}

function blocks(children, out) {
	for (const node of children ?? []) {
		if (node.type === 'paragraph') out.push(text(node));
		else if (node.type === 'table') {
			for (const row of node.children ?? []) {
				for (const cell of row.children ?? []) {
					for (const p of cell.children ?? []) if (p.type === 'paragraph') out.push(text(p));
				}
			}
		} else if (node.children && node.type !== 'table') blocks(node.children, out);
	}
}

export function extract(doc) {
	const out = [];
	blocks(doc.documentPart.body.children, out);
	return { paragraphs: out };
}
