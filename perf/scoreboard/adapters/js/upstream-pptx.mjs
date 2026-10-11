// SPDX-License-Identifier: MIT
// Upstream ooxml TS (reference), pptx: PptxHandler.load, as the upstream PowerPoint viewer's
// load-content service does (viewers/pptx/packages/angular/src/viewer/load-content.service.ts).
import { PptxHandler } from 'ooxml-core/pptx';

export function parse(bytes) {
	const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
	return new PptxHandler().load(buffer);
}

// Paragraphs of a segment list: the slide-load parser separates <a:p> with a bare '\n' segment
// (no isLineBreak). The parser also emits a synthetic bullet-marker segment per bulleted paragraph
// (bulletInfo set and not `none`; PptxHandlerRuntimeShapeParagraphContentParsing.ts); markers are
// not document text. Text runs of a paragraph with no bullet carry bulletInfo { none: true }.
function segmentParagraphs(segments) {
	const out = [];
	let cur = '';
	for (const s of segments) {
		if (s.isParagraphBreak || (s.text === '\n' && !s.isLineBreak)) {
			out.push(cur);
			cur = '';
		} else if (!s.bulletInfo || s.bulletInfo.none) cur += s.text ?? '';
	}
	out.push(cur);
	return out;
}

function walk(elements, out) {
	for (const e of elements) {
		if (e.textSegments) out.push(...segmentParagraphs(e.textSegments));
		else if (typeof e.text === 'string' && e.type !== 'picture') out.push(...e.text.split('\n'));
		if (e.type === 'table' && e.tableData) {
			for (const row of e.tableData.rows ?? []) {
				for (const cell of row.cells ?? []) {
					if (cell.textSegments) out.push(...segmentParagraphs(cell.textSegments));
					else if (typeof cell.text === 'string') out.push(...cell.text.split('\n'));
				}
			}
		}
		if (e.children) walk(e.children, out);
	}
}

export function extract(data) {
	return {
		slides: data.slides.map((slide) => {
			const out = [];
			walk(slide.elements, out);
			return out;
		}),
	};
}
