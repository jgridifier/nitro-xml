// SPDX-License-Identifier: MIT
/**
 * Apply an edits/*.json op list to the pinned upstream TS editor and save.
 * Test-only oracle path — never shipped.
 */

import { join } from 'node:path';
import { ROOT } from './scripts.mjs';

function upstreamRoot() {
	return process.env.UPSTREAM_DIR || join(ROOT, 'upstream');
}

function toArrayBuffer(u8) {
	return u8.buffer.slice(u8.byteOffset, u8.byteOffset + u8.byteLength);
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
 * @param {object[]} ops
 */
export function applyDocxOps(model, ops) {
	for (const op of ops) {
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
				break;
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
				break;
			}
			case 'setParagraphAlign': {
				const block = model.blocks[op.blockIndex];
				if (!block || block.type !== 'paragraph') {
					throw new Error(`block ${op.blockIndex} is not a paragraph`);
				}
				block.align = op.align;
				break;
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
				break;
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
				break;
			}
			case 'deleteTableRow': {
				const table = model.blocks[op.blockIndex];
				if (!table || table.type !== 'table') {
					throw new Error(`block ${op.blockIndex} is not a table`);
				}
				if (table.rows.length <= 1) throw new Error('cannot delete last table row');
				table.rows.splice(op.at, 1);
				break;
			}
			case 'insertTableColumn': {
				const table = model.blocks[op.blockIndex];
				if (!table || table.type !== 'table') {
					throw new Error(`block ${op.blockIndex} is not a table`);
				}
				for (const [ri, row] of table.rows.entries()) {
					row.splice(op.at, 0, emptyDocxCell(`r${ri}-c${op.at}`));
				}
				break;
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
				break;
			}
			default:
				throw new Error(`unsupported docx op: ${op.op}`);
		}
	}
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
	const { cloneElement } = await import(`${up}/src/core/pptx/core/utils/clone-utils.ts`);
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
	};
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
		return await handler.save(slides, {
			headerFooter: data.headerFooter,
			tableStyles: data.tableStyleMap,
		});
	}

	throw new Error(`unsupported format ${format}`);
}
