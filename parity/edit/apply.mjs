// SPDX-License-Identifier: MIT
/**
 * Apply an edits/*.json op list to the pinned upstream TS editor and save.
 * Test-only oracle path — never shipped.
 */

import { join } from 'node:path';
import { ROOT } from './scripts.mjs';
import {
	acceptRevision,
	rejectRevision,
	acceptAllRevisions,
	rejectAllRevisions,
} from '../../upstream/src/core/docx/revision-commands.ts';
import {
	deleteComment,
	resolveComment,
	replyToComment,
} from '../../upstream/src/core/docx/ui/comment-commands.ts';

let commentSerial = 0;
function nextCommentId(provided) {
	return provided || `dve-comment-${Date.now().toString(36)}-${++commentSerial}`;
}

function attachCommentId(run, id) {
	const ids = [...(run.commentIds || [])];
	if (!ids.includes(id)) ids.push(id);
	ids.sort();
	run.commentIds = ids;
}

function forEachRun(model, fn) {
	for (const block of model.blocks || []) {
		if (block.type === 'paragraph') {
			for (const run of block.runs || []) fn(run);
		} else if (block.type === 'table') {
			for (const row of block.rows || []) {
				for (const cell of row || []) {
					for (const para of cell.paragraphs || []) {
						for (const run of para.runs || []) fn(run);
					}
				}
			}
		}
	}
}

function removeCommentAnchor(run, id) {
	const remaining = (run.commentIds || []).filter((existing) => existing !== id);
	if (remaining.length) run.commentIds = remaining;
	else delete run.commentIds;
}

function upstreamRoot() {
	return process.env.UPSTREAM_DIR || join(ROOT, 'upstream');
}

function toArrayBuffer(u8) {
	return u8.buffer.slice(u8.byteOffset, u8.byteOffset + u8.byteLength);
}

/**
 * `rebuildTableXmlFromData` mints a new `a16:colId` with `Math.random`.
 * Seed each save so two applies of `insertSlideTableColumn` stay byte-stable.
 * @template T
 * @param {() => Promise<T>} fn
 * @returns {Promise<T>}
 */
export async function withStableRandom(fn) {
	const prev = Math.random;
	let s = 0x85a11ce;
	Math.random = () => {
		s = (Math.imul(1664525, s) + 1013904223) >>> 0;
		return s / 4294967296;
	};
	try {
		return await fn();
	} finally {
		Math.random = prev;
	}
}
function emptyDocxCell(id) {
	return {
		paragraphs: [{ type: 'paragraph', id, runs: [{ text: '' }] }],
	};
}

/**
 * @param {object} session
 * @param {object[]} ops
 */
function applyXlsxOps(session, ops, parseRange) {
	for (const op of ops) {
		switch (op.op) {
			case 'setCellInput':
				session.setCellInput(op.sheet, op.row, op.col, op.text);
				break;
			case 'setCellValue':
				session.setCellValue(op.sheet, op.row, op.col, op.value);
				break;
			case 'applyStyle': {
				const range = parseRange(op.range);
				if (!range) throw new Error(`bad range ${op.range}`);
				session.applyStyle(op.sheet, [range], op.patch);
				break;
			}
			case 'applyCellStyle': {
				const range = parseRange(op.range);
				if (!range) throw new Error(`bad range ${op.range}`);
				session.applyCellStyle(op.sheet, [range], op.name);
				break;
			}
			case 'addSheet':
				session.addSheet(op.name, op.at);
				break;
			case 'renameSheet':
				session.renameSheet(op.index, op.name);
				break;
			case 'deleteSheet':
				session.deleteSheet(op.index);
				break;
			case 'insertRows':
				session.insertRows(op.sheet, op.at, op.count);
				break;
			case 'deleteRows':
				session.deleteRows(op.sheet, op.at, op.count);
				break;
			case 'insertColumns':
				session.insertColumns(op.sheet, op.at, op.count);
				break;
			case 'deleteColumns':
				session.deleteColumns(op.sheet, op.at, op.count);
				break;
			case 'batch':
				session.batch(op.label || 'batch', () => applyXlsxOps(session, op.ops, parseRange));
				break;
			default:
				throw new Error(`unsupported xlsx op: ${op.op}`);
		}
	}
}

/**
 * @param {object} model DocumentModel
 * @param {object} op
 * @returns {string} undo label
 */
export function applyDocxOp(model, op) {
	switch (op.op) {
			case 'setParagraphText': {
				const block = model.blocks[op.blockIndex];
				if (!block || block.type !== 'paragraph') {
					throw new Error(`block ${op.blockIndex} is not a paragraph`);
				}
				/** @type {Record<string, unknown>} */
				const run = { text: op.text };
				if (op.bold !== undefined) run.bold = op.bold;
				if (op.italic !== undefined) run.italic = op.italic;
				if (op.fontSize !== undefined) run.fontSize = op.fontSize;
				if (op.color !== undefined) run.color = op.color;
				block.runs = [run];
				return `setParagraphText ${op.blockIndex}`;
			}
			case 'setRunFormatting': {
				const block = model.blocks[op.blockIndex];
				if (!block || block.type !== 'paragraph') {
					throw new Error(`block ${op.blockIndex} is not a paragraph`);
				}
				const ri = op.runIndex ?? 0;
				const run = block.runs[ri];
				if (!run) throw new Error(`run ${ri} missing on block ${op.blockIndex}`);
				if (op.bold !== undefined) run.bold = op.bold;
				if (op.italic !== undefined) run.italic = op.italic;
				if (op.fontSize !== undefined) run.fontSize = op.fontSize;
				if (op.color !== undefined) run.color = op.color;
				if (op.underline !== undefined) run.underline = op.underline;
				return `setRunFormatting ${op.blockIndex}`;
			}
			case 'setParagraphAlign': {
				const block = model.blocks[op.blockIndex];
				if (!block || block.type !== 'paragraph') {
					throw new Error(`block ${op.blockIndex} is not a paragraph`);
				}
				block.align = op.align;
				return `setParagraphAlign ${op.blockIndex}`;
			}
			case 'setTableCellText': {
				const table = model.blocks[op.blockIndex];
				if (!table || table.type !== 'table') {
					throw new Error(`block ${op.blockIndex} is not a table`);
				}
				const cell = table.rows[op.row]?.[op.col];
				if (!cell) throw new Error(`cell ${op.row},${op.col} missing`);
				const para = cell.paragraphs[0] || {
					type: 'paragraph',
					id: `p-${op.row}-${op.col}`,
					runs: [],
				};
				para.runs = [{ text: op.text }];
				cell.paragraphs[0] = para;
				return `setTableCellText ${op.row},${op.col}`;
			}
			case 'insertTableRow': {
				const table = model.blocks[op.blockIndex];
				if (!table || table.type !== 'table') {
					throw new Error(`block ${op.blockIndex} is not a table`);
				}
				const cols = table.rows[0]?.length || 1;
				const row = Array.from({ length: cols }, (_, c) =>
					emptyDocxCell(`r${op.at}-c${c}`),
				);
				table.rows.splice(op.at, 0, row);
				return `insertTableRow ${op.at}`;
			}
			case 'deleteTableRow': {
				const table = model.blocks[op.blockIndex];
				if (!table || table.type !== 'table') {
					throw new Error(`block ${op.blockIndex} is not a table`);
				}
				if (table.rows.length <= 1) throw new Error('cannot delete last table row');
				table.rows.splice(op.at, 1);
				return `deleteTableRow ${op.at}`;
			}
			case 'insertTableColumn': {
				const table = model.blocks[op.blockIndex];
				if (!table || table.type !== 'table') {
					throw new Error(`block ${op.blockIndex} is not a table`);
				}
				for (const [ri, row] of table.rows.entries()) {
					row.splice(op.at, 0, emptyDocxCell(`r${ri}-c${op.at}`));
				}
				return `insertTableColumn ${op.at}`;
			}
			case 'deleteTableColumn': {
				const table = model.blocks[op.blockIndex];
				if (!table || table.type !== 'table') {
					throw new Error(`block ${op.blockIndex} is not a table`);
				}
				if ((table.rows[0]?.length || 0) <= 1) {
					throw new Error('cannot delete last table column');
				}
				for (const row of table.rows) row.splice(op.at, 1);
				return `deleteTableColumn ${op.at}`;
			}
			case 'acceptRevision': {
				Object.assign(model, acceptRevision(model, op.id));
				return `acceptRevision ${op.id}`;
			}
			case 'rejectRevision': {
				Object.assign(model, rejectRevision(model, op.id));
				return `rejectRevision ${op.id}`;
			}
			case 'acceptAllRevisions': {
				Object.assign(model, acceptAllRevisions(model));
				return 'acceptAllRevisions';
			}
			case 'rejectAllRevisions': {
				Object.assign(model, rejectAllRevisions(model));
				return 'rejectAllRevisions';
			}
			case 'addComment': {
				const block = model.blocks[op.blockIndex];
				if (!block || block.type !== 'paragraph') {
					throw new Error(`block ${op.blockIndex} is not a paragraph`);
				}
				const id = nextCommentId(op.id);
				const comment = { id, author: op.author, text: op.text, resolved: false };
				model.comments = [...(model.comments || []), comment];
				const ri = op.runIndex;
				if (ri !== undefined) {
					const run = block.runs?.[ri];
					if (!run) throw new Error(`run ${ri} missing`);
					attachCommentId(run, id);
				} else {
					for (const run of block.runs || []) attachCommentId(run, id);
				}
				return `addComment ${id}`;
			}
			case 'replyToComment': {
				if (!(model.comments || []).some((c) => c.id === op.parentId)) {
					throw new Error(`comment ${op.parentId} missing`);
				}
				model.comments = replyToComment(
					model.comments || [],
					op.parentId,
					op.author,
					op.text,
					op.id ? () => op.id : undefined,
				);
				return `replyToComment ${op.parentId}`;
			}
			case 'resolveComment': {
				if (!(model.comments || []).some((c) => c.id === op.id)) {
					throw new Error(`comment ${op.id} missing`);
				}
				model.comments = resolveComment(model.comments || [], op.id, op.resolved ?? true);
				return `resolveComment ${op.id}`;
			}
			case 'deleteComment': {
				if (!(model.comments || []).some((c) => c.id === op.id)) {
					throw new Error(`comment ${op.id} missing`);
				}
				model.comments = deleteComment(model.comments || [], op.id);
				forEachRun(model, (run) => removeCommentAnchor(run, op.id));
				return `deleteComment ${op.id}`;
			}
			default:
				throw new Error(`unsupported docx op: ${op.op}`);
		}
}

/**
 * @param {object} model DocumentModel
 * @param {object[]} ops
 */
export function applyDocxOps(model, ops) {
	for (const op of ops) applyDocxOp(model, op);
}

function renumberPptxSlides(slides) {
	return slides.map((slide, i) =>
		slide.slideNumber === i + 1 ? slide : { ...slide, slideNumber: i + 1 },
	);
}

/** Deterministic stand-in for `makeSlideId` so Rust and a second save agree. */
function reassignPptxElementIds(el, prefix) {
	const next = { ...el, id: `${prefix}${el.id}` };
	if (Array.isArray(el.children) && el.children.length) {
		next.children = el.children.map((child) => reassignPptxElementIds(child, prefix));
	}
	return next;
}

function isParagraphSeparator(seg) {
	return Boolean(seg.isParagraphBreak) || (seg.text === '\n' && !seg.isLineBreak);
}

function contentRunIndexes(segments) {
	/** @type {number[][]} */
	const paras = [[]];
	segments.forEach((seg, i) => {
		if (isParagraphSeparator(seg)) paras.push([]);
		else paras[paras.length - 1].push(i);
	});
	return paras;
}

function resolvePptxElement(slides, op) {
	const slide = slides[op.slideIndex];
	if (!slide) throw new Error(`slide ${op.slideIndex} missing`);
	if (op.elementId) {
		const el = slide.elements.find((e) => e.id === op.elementId);
		if (!el) throw new Error(`element ${op.elementId} missing`);
		return el;
	}
	const idx = op.elementIndex ?? 0;
	const el = slide.elements[idx];
	if (!el) throw new Error(`element index ${idx} missing on slide ${op.slideIndex}`);
	return el;
}

/** ElementFactory default solid fill (`createShapeElement`). */
const DEFAULT_SHAPE_FILL = '#4472C4';
const DUPLICATE_OFFSET_PX = 20;

/**
 * @param {object[]} slides
 * @param {object[]} ops
 * @param {*} mut
 */
export function applyPptxOps(slides, ops, mut) {
	const {
		updateElement,
		patchElementGeometry,
		remapTextToSegments,
		removeElement,
		appendElementOnSlide,
		reorderElementOnSlide,
		bringToFront,
		sendToBack,
		bringForward,
		sendBackward,
		groupElements,
		ungroupElements,
		cloneElement,
		cloneSlide,
	} = mut;
	let cur = slides;
	for (const op of ops) {
		switch (op.op) {
			case 'setShapeText': {
				const el = resolvePptxElement(cur, op);
				cur = updateElement(cur, op.slideIndex, el.id, {
					text: op.text,
					textSegments: remapTextToSegments(op.text, el.textSegments, el.textStyle),
				});
				break;
			}
			case 'patchShapeGeometry': {
				const el = resolvePptxElement(cur, op);
				const box = {
					x: op.x ?? el.x ?? 0,
					y: op.y ?? el.y ?? 0,
					width: op.width ?? el.width ?? 0,
					height: op.height ?? el.height ?? 0,
					rotation: op.rotation ?? el.rotation ?? 0,
				};
				cur = patchElementGeometry(cur, op.slideIndex, el.id, box);
				break;
			}
			case 'setShapeTextStyle': {
				const el = resolvePptxElement(cur, op);
				/** @type {Record<string, unknown>} */
				const textStyle = { ...(el.textStyle || {}) };
				if (op.bold !== undefined) textStyle.bold = op.bold;
				if (op.italic !== undefined) textStyle.italic = op.italic;
				if (op.fontSize !== undefined) textStyle.fontSize = op.fontSize;
				if (op.color !== undefined) textStyle.color = op.color;
				cur = updateElement(cur, op.slideIndex, el.id, { textStyle });
				break;
			}
			case 'addShape': {
				const slide = cur[op.slideIndex];
				if (!slide) throw new Error(`slide ${op.slideIndex} missing`);
				const id =
					op.id ?? `added-shape-${op.slideIndex}-${(slide.elements || []).length}`;
				const element = {
					type: 'shape',
					id,
					x: op.x,
					y: op.y,
					width: op.width,
					height: op.height,
					rotation: 0,
					shapeType: op.shapeType,
					shapeStyle: { fillMode: 'solid', fillColor: DEFAULT_SHAPE_FILL },
				};
				cur = appendElementOnSlide(cur, op.slideIndex, element);
				break;
			}
			case 'deleteShape': {
				const el = resolvePptxElement(cur, op);
				cur = removeElement(cur, op.slideIndex, el.id);
				break;
			}
			case 'duplicateShape': {
				const el = resolvePptxElement(cur, op);
				const copy = cloneElement(el);
				copy.id = op.newId ?? `dup-${el.id}`;
				copy.x = (el.x ?? 0) + DUPLICATE_OFFSET_PX;
				copy.y = (el.y ?? 0) + DUPLICATE_OFFSET_PX;
				cur = appendElementOnSlide(cur, op.slideIndex, copy);
				break;
			}
			case 'bringForward':
			case 'sendBackward':
			case 'bringToFront':
			case 'sendToBack': {
				const el = resolvePptxElement(cur, op);
				const transform =
					op.op === 'bringToFront'
						? (elements) => bringToFront(elements, el.id)
						: op.op === 'sendToBack'
							? (elements) => sendToBack(elements, el.id)
							: op.op === 'bringForward'
								? (elements) => bringForward(elements, el.id)
								: (elements) => sendBackward(elements, el.id);
				cur = reorderElementOnSlide(cur, op.slideIndex, transform);
				break;
			}
			case 'groupShapes': {
				const slide = cur[op.slideIndex];
				if (!slide) throw new Error(`slide ${op.slideIndex} missing`);
				const grouped = groupElements(slide.elements || [], op.elementIds || [], op.groupId);
				cur = cur.map((s, i) =>
					i === op.slideIndex ? { ...s, elements: grouped.elements } : s,
				);
				break;
			}
			case 'ungroupShapes': {
				const slide = cur[op.slideIndex];
				if (!slide) throw new Error(`slide ${op.slideIndex} missing`);
				const groupId = op.groupId ?? op.elementId;
				if (!groupId) throw new Error('ungroupShapes missing groupId');
				const found = (slide.elements || []).some((e) => e.id === groupId);
				if (!found) throw new Error(`element ${groupId} missing`);
				const ungrouped = ungroupElements(slide.elements || [], groupId, op.childIds || []);
				cur = cur.map((s, i) =>
					i === op.slideIndex ? { ...s, elements: ungrouped.elements } : s,
				);
				break;
			}
			case 'setShapeFill': {
				const el = resolvePptxElement(cur, op);
				const shapeStyle = { ...(el.shapeStyle || {}) };
				if (op.fillColor !== undefined) shapeStyle.fillColor = op.fillColor;
				if (op.fillMode !== undefined) shapeStyle.fillMode = op.fillMode;
				cur = updateElement(cur, op.slideIndex, el.id, { shapeStyle });
				break;
			}
			case 'setShapeLine': {
				const el = resolvePptxElement(cur, op);
				const shapeStyle = { ...(el.shapeStyle || {}) };
				if (op.strokeColor !== undefined) shapeStyle.strokeColor = op.strokeColor;
				if (op.strokeWidth !== undefined) shapeStyle.strokeWidth = op.strokeWidth;
				cur = updateElement(cur, op.slideIndex, el.id, { shapeStyle });
				break;
			}
			case 'addSlide': {
				// Blank slide shape matches `createBlankSlide`; the id is deterministic.
				const len = cur.length;
				const at = op.at == null ? len : Math.max(0, Math.min(op.at, len));
				const slide = {
					id: `added-slide-${len}`,
					rId: '',
					slideNumber: at + 1,
					elements: [],
				};
				const next = cur.slice();
				next.splice(at, 0, slide);
				cur = renumberPptxSlides(next);
				break;
			}
			case 'deleteSlide': {
				const index = op.index;
				if (cur.length <= 1) throw new Error('cannot delete the last slide');
				if (index == null || index < 0 || index >= cur.length) {
					throw new Error(`slide index ${index} out of range`);
				}
				cur = renumberPptxSlides(cur.filter((_, i) => i !== index));
				break;
			}
			case 'duplicateSlide': {
				const index = op.index;
				if (index == null || index < 0 || index >= cur.length) {
					throw new Error(`slide index ${index} out of range`);
				}
				const source = cur[index];
				const copy = cloneSlide(source);
				const len = cur.length;
				const sourceId = source.id;
				copy.sourceSlideId = String(sourceId).startsWith('ppt/slides/')
					? sourceId
					: (source.sourceSlideId ?? sourceId);
				copy.id = `dup-slide-${len}`;
				copy.rId = '';
				copy.elements = (copy.elements || []).map((el) =>
					reassignPptxElementIds(el, `dup-${len}-`),
				);
				const next = cur.slice();
				next.splice(index + 1, 0, copy);
				cur = renumberPptxSlides(next);
				break;
			}
			case 'reorderSlide': {
				const from = op.from;
				const to = op.to;
				if (
					from == null ||
					to == null ||
					from < 0 ||
					to < 0 ||
					from >= cur.length ||
					to >= cur.length
				) {
					throw new Error('slide index out of range');
				}
				if (from === to) break;
				const next = cur.slice();
				const [moved] = next.splice(from, 1);
				next.splice(to, 0, moved);
				cur = renumberPptxSlides(next);
				break;
			}
			case 'setNotesText': {
				const slide = cur[op.slideIndex];
				if (!slide) throw new Error(`slide ${op.slideIndex} missing`);
				const next = cur.slice();
				// Drop stale segments so the plain string wins on save.
				next[op.slideIndex] = { ...slide, notes: op.text, notesSegments: undefined };
				cur = next;
				break;
			}
case 'setShapeRunStyle': {
				const el = resolvePptxElement(cur, op);
				if (el.type !== 'text' && el.type !== 'shape' && el.type !== 'connector') {
					throw new Error('setShapeRunStyle: element has no text');
				}
				const segments = el.textSegments?.length
					? el.textSegments.map((s) => ({ ...s, style: { ...(s.style || {}) } }))
					: [{ text: el.text ?? '', style: { ...(el.textStyle || {}) } }];
				const paras = contentRunIndexes(segments);
				const para = paras[op.paragraphIndex];
				if (!para) throw new Error(`setShapeRunStyle paragraph ${op.paragraphIndex} missing`);
				const segIdx = para[op.runIndex];
				if (segIdx == null) throw new Error(`setShapeRunStyle run ${op.runIndex} missing`);
				/** @type {Record<string, unknown>} */
				const updates = {};
				if (op.bold !== undefined) updates.bold = op.bold;
				if (op.italic !== undefined) updates.italic = op.italic;
				if (op.underline !== undefined) updates.underline = op.underline;
				if (op.fontSize !== undefined) updates.fontSize = op.fontSize;
				if (op.color !== undefined) updates.color = op.color;
				const end = (segments[segIdx].text ?? '').length;
				const selection = {
					startSegIdx: segIdx,
					startOffset: 0,
					endSegIdx: segIdx,
					endOffset: end,
				};
				const { patch } = mut.applyTextStyleUpdate(el, updates, selection, segments);
				cur = updateElement(cur, op.slideIndex, el.id, patch);
				break;
			}
case 'setShapeParagraphAlign': {
				const el = resolvePptxElement(cur, op);
				if (el.type !== 'text' && el.type !== 'shape' && el.type !== 'connector') {
					throw new Error('setShapeParagraphAlign: element has no text');
				}
				const segments = (el.textSegments?.length ? el.textSegments : [{ text: el.text ?? '', style: {} }]).map(
					(s) => ({
						...s,
						style: { ...(s.style || {}) },
						...(s.paragraphProperties
							? { paragraphProperties: { ...s.paragraphProperties } }
							: {}),
					}),
				);
				const paras = contentRunIndexes(segments);
				const para = paras[op.paragraphIndex];
				if (!para || para[0] == null) {
					throw new Error(`setShapeParagraphAlign paragraph ${op.paragraphIndex} missing`);
				}
				const seg = segments[para[0]];
				seg.paragraphProperties = { ...(seg.paragraphProperties || {}), align: op.align };
				cur = updateElement(cur, op.slideIndex, el.id, { textSegments: segments });
				break;
			}
case 'setShapeBullets': {
				const el = resolvePptxElement(cur, op);
				const range =
					op.startParagraph != null && op.endParagraph != null
						? { startParagraph: op.startParagraph, endParagraph: op.endParagraph }
						: undefined;
				const patch = mut.setElementBullets(el, op.kind, range);
				if (patch.textSegments || patch.textStyle) {
					cur = updateElement(cur, op.slideIndex, el.id, patch);
				}
				break;
			}
case 'setShapeAutofit': {
				const el = resolvePptxElement(cur, op);
				cur = updateElement(cur, op.slideIndex, el.id, mut.autoFitModePatch(el, op.mode));
				break;
			}
case 'setSlideCellText': {
				const el = resolvePptxElement(cur, op);
				if (el.type !== 'table' || !el.tableData) {
					throw new Error('setSlideCellText rejected: not a table');
				}
				const next = mut.setCellText(el, op.row, op.col, op.text);
				cur = updateElement(cur, op.slideIndex, el.id, { tableData: next.tableData });
				break;
			}
case 'insertSlideTableRow': {
				const el = resolvePptxElement(cur, op);
				if (!el.tableData) throw new Error('element is not a table');
				const tableData = mut.insertTableRow(el.tableData, op.row, op.position);
				cur = updateElement(cur, op.slideIndex, el.id, { tableData });
				break;
			}
case 'deleteSlideTableRow': {
				const el = resolvePptxElement(cur, op);
				if (!el.tableData) throw new Error('element is not a table');
				const tableData = mut.deleteTableRow(el.tableData, op.row);
				cur = updateElement(cur, op.slideIndex, el.id, { tableData });
				break;
			}
case 'insertSlideTableColumn': {
				const el = resolvePptxElement(cur, op);
				if (!el.tableData) throw new Error('element is not a table');
				const tableData = mut.insertTableColumn(el.tableData, op.col, op.position);
				cur = updateElement(cur, op.slideIndex, el.id, { tableData });
				break;
			}
case 'deleteSlideTableColumn': {
				const el = resolvePptxElement(cur, op);
				if (!el.tableData) throw new Error('element is not a table');
				const tableData = mut.deleteTableColumn(el.tableData, op.col);
				cur = updateElement(cur, op.slideIndex, el.id, { tableData });
				break;
			}
			default:
				throw new Error(`unsupported pptx op: ${op.op}`);
		}
	}
	return cur;
}

/** Dynamic imports shared by apply, the model dump, and undo session. */
export async function loadPptxMutators() {
	const up = upstreamRoot();
	const { updateElement, patchElementGeometry, removeElement, appendElementOnSlide, reorderElementOnSlide } =
		await import(`${up}/src/core/pptx/editor/render/editor-mutations.ts`);
	const { remapTextToSegments } = await import(`${up}/src/core/pptx/editor/render/remap-text.ts`);
	const { bringToFront, sendToBack, bringForward, sendBackward } = await import(
		`${up}/src/core/pptx/editor/render/element-operations.ts`
	);
	const { groupElements, ungroupElements } = await import(
		`${up}/src/core/pptx/core/utils/group-ops.ts`
	);
	const { cloneElement, cloneSlide } = await import(`${up}/src/core/pptx/core/utils/clone-utils.ts`);
	const { setElementBullets } = await import(`${up}/src/core/pptx/editor/render/bullet-toggle.ts`);
	const { autoFitModePatch } = await import(`${up}/src/ui/src/pptx/render/text-advanced.ts`);
	const { applyTextStyleUpdate } = await import(`${up}/src/ui/src/pptx/render/text-style-update.ts`);
	const { setCellText } = await import(`${up}/src/core/pptx/editor/render/table-cell-edit.ts`);
	const { insertTableRow, deleteTableRow, insertTableColumn, deleteTableColumn } = await import(
		`${up}/src/core/pptx/editor/render/table-layout.ts`
	);
	return {
		updateElement,
		patchElementGeometry,
		remapTextToSegments,
		removeElement,
		appendElementOnSlide,
		reorderElementOnSlide,
		bringToFront,
		sendToBack,
		bringForward,
		sendBackward,
		groupElements,
		ungroupElements,
		cloneElement,
		cloneSlide,
		setElementBullets,
		autoFitModePatch,
		applyTextStyleUpdate,
		setCellText,
		insertTableRow,
		deleteTableRow,
		insertTableColumn,
		deleteTableColumn,
	};
}

/**
 * Open docx bytes, apply ops, return the mutated model (no save).
 * Used for `modelOnly` scripts when upstream save cannot round-trip (#82 / #80).
 * @param {Uint8Array} bytes
 * @param {{ ops: object[] }} script
 */
export async function applyDocxModel(bytes, script) {
	const up = upstreamRoot();
	const { loadDocx } = await import(`${up}/src/core/docx/parse.ts`);
	const loaded = await loadDocx(bytes);
	applyDocxOps(loaded.model, script.ops);
	return loaded.model;
}

/**
 * Open bytes, apply script ops, save.
 * @param {Uint8Array} bytes
 * @param {{ format: string, ops: object[] }} script
 * @returns {Promise<Uint8Array>}
 */
export async function applyAndSave(bytes, script) {
	const up = upstreamRoot();
	const format = script.format;

	if (format === 'xlsx') {
		const { loadXlsx } = await import(`${up}/src/core/xlsx/read/load.ts`);
		const { saveXlsx } = await import(`${up}/src/core/xlsx/write/save.ts`);
		const { createEditSession } = await import(`${up}/src/core/xlsx/edit/session.ts`);
		const { parseRange } = await import(`${up}/src/core/xlsx/address.ts`);
		const workbook = await loadXlsx(bytes);
		const session = createEditSession(workbook, { recalc: false });
		applyXlsxOps(session, script.ops, parseRange);
		return await saveXlsx(session.workbook);
	}

	if (format === 'docx') {
		const { loadDocx } = await import(`${up}/src/core/docx/parse.ts`);
		const loaded = await loadDocx(bytes);
		applyDocxOps(loaded.model, script.ops);
		return await loaded.save(loaded.model);
	}

	if (format === 'pptx') {
		const { PptxHandler } = await import(`${up}/src/core/pptx/core/PptxHandler.ts`);
		const mut = await loadPptxMutators();
		const handler = new PptxHandler();
		const data = await handler.load(toArrayBuffer(bytes));
		const slides = applyPptxOps(data.slides, script.ops, mut);
		return await withStableRandom(() =>
			handler.save(slides, {
				headerFooter: data.headerFooter,
				tableStyles: data.tableStyleMap,
			}),
		);
	}

	throw new Error(`unsupported format ${format}`);
}
