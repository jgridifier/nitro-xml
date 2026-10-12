// SPDX-License-Identifier: MIT
/**
 * Undo/redo-aware apply of edits/*.json ops against the pinned upstream TS editor (#51).
 * Captures canonical model snapshots after every do / undo / redo step.
 */

import { join } from 'node:path';
import { applyPptxOps, loadPptxMutators } from '../edit/apply.mjs';
import { ROOT } from '../edit/scripts.mjs';
import {
	snapshotXlsx,
	snapshotDocx,
	snapshotPptx,
	stableStringify,
	snapshotsEqual,
} from './snapshot.mjs';

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

/** Deep-clone model state for docx undo stacks. */
function cloneModel(v) {
	return structuredClone(v);
}

/**
 * Expand top-level ops: batch stays one undo unit for xlsx; for docx/pptx
 * batch (if ever used) expands so each inner op is a checkpoint.
 */
function topLevelOps(ops) {
	return ops;
}

/**
 * @typedef {{ phase: string, step: number, label: string, snapshot: object }} Checkpoint
 */

/**
 * Run one script with do → undo-all → redo-all checkpoints.
 * @param {Uint8Array} bytes
 * @param {{ format: string, ops: object[], id?: string }} script
 * @returns {Promise<{ checkpoints: Checkpoint[], start: object, afterDos: object, afterUndoAll: object, afterRedoAll: object, undoSave: Uint8Array, redoSave: Uint8Array, noopSave: Uint8Array }>}
 */
export async function runUndoScript(bytes, script) {
	const format = script.format;
	if (format === 'xlsx') return runXlsx(bytes, script);
	if (format === 'docx') return runDocx(bytes, script);
	if (format === 'pptx') return runPptx(bytes, script);
	throw new Error(`unsupported format ${format}`);
}

async function runXlsx(bytes, script) {
	const up = upstreamRoot();
	const { loadXlsx } = await import(`${up}/src/core/xlsx/read/load.ts`);
	const { saveXlsx } = await import(`${up}/src/core/xlsx/write/save.ts`);
	const { createEditSession } = await import(`${up}/src/core/xlsx/edit/session.ts`);
	const { parseRange } = await import(`${up}/src/core/xlsx/address.ts`);

	const noopWb = await loadXlsx(bytes);
	const noopSave = await saveXlsx(noopWb);

	const workbook = await loadXlsx(bytes);
	const session = createEditSession(workbook, { recalc: false });
	/** @type {Checkpoint[]} */
	const checkpoints = [];
	let step = 0;
	const push = (phase, label) => {
		checkpoints.push({
			phase,
			step: step++,
			label,
			snapshot: snapshotXlsx(session.workbook),
		});
	};

	const start = snapshotXlsx(session.workbook);
	push('start', 'open');

	const applyOne = (op) => {
		switch (op.op) {
			case 'setCellInput':
				session.setCellInput(op.sheet, op.row, op.col, op.text);
				return `setCellInput ${op.sheet}:${op.row}:${op.col}`;
			case 'setCellValue':
				session.setCellValue(op.sheet, op.row, op.col, op.value);
				return `setCellValue ${op.sheet}:${op.row}:${op.col}`;
			case 'applyStyle': {
				const range = parseRange(op.range);
				if (!range) throw new Error(`bad range ${op.range}`);
				session.applyStyle(op.sheet, [range], op.patch);
				return `applyStyle ${op.range}`;
			}
			case 'applyCellStyle': {
				const range = parseRange(op.range);
				if (!range) throw new Error(`bad range ${op.range}`);
				session.applyCellStyle(op.sheet, [range], op.name);
				return `applyCellStyle ${op.name}`;
			}
			case 'addSheet':
				session.addSheet(op.name, op.at);
				return `addSheet ${op.name || ''}`;
			case 'renameSheet':
				session.renameSheet(op.index, op.name);
				return `renameSheet ${op.index}`;
			case 'deleteSheet':
				session.deleteSheet(op.index);
				return `deleteSheet ${op.index}`;
			case 'insertRows':
				session.insertRows(op.sheet, op.at, op.count);
				return `insertRows ${op.sheet}:${op.at}:${op.count}`;
			case 'deleteRows':
				session.deleteRows(op.sheet, op.at, op.count);
				return `deleteRows ${op.sheet}:${op.at}:${op.count}`;
			case 'insertColumns':
				session.insertColumns(op.sheet, op.at, op.count);
				return `insertColumns ${op.sheet}:${op.at}:${op.count}`;
			case 'deleteColumns':
				session.deleteColumns(op.sheet, op.at, op.count);
				return `deleteColumns ${op.sheet}:${op.at}:${op.count}`;
			case 'batch':
				session.batch(op.label || 'batch', () => {
					for (const inner of op.ops) applyOne(inner);
				});
				return `batch ${op.label || ''}`;
			default:
				throw new Error(`unsupported xlsx op: ${op.op}`);
		}
	};

	for (const op of topLevelOps(script.ops)) {
		// For batch, applyOne already groups undo; still one checkpoint after.
		if (op.op === 'batch') {
			const label = applyOne(op);
			push('do', label);
		} else {
			const label = applyOne(op);
			push('do', label);
		}
	}
	const afterDos = snapshotXlsx(session.workbook);

	while (session.canUndo()) {
		const label = session.undoLabel() || 'undo';
		session.undo();
		push('undo', label);
	}
	const afterUndoAll = snapshotXlsx(session.workbook);
	const undoSave = await saveXlsx(session.workbook);

	while (session.canRedo()) {
		const label = session.redoLabel() || 'redo';
		session.redo();
		push('redo', label);
	}
	const afterRedoAll = snapshotXlsx(session.workbook);
	const redoSave = await saveXlsx(session.workbook);

	return {
		checkpoints,
		start,
		afterDos,
		afterUndoAll,
		afterRedoAll,
		undoSave,
		redoSave,
		noopSave,
	};
}

function applyDocxOp(model, op) {
	switch (op.op) {
		case 'setParagraphText': {
			const block = model.blocks[op.blockIndex];
			if (!block || block.type !== 'paragraph') {
				throw new Error(`block ${op.blockIndex} is not a paragraph`);
			}
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
			if (!run) throw new Error(`run ${ri} missing`);
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
			if (!table || table.type !== 'table') throw new Error(`not a table`);
			const cell = table.rows[op.row]?.[op.col];
			if (!cell) throw new Error(`cell missing`);
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
			if (!table || table.type !== 'table') throw new Error(`not a table`);
			const cols = table.rows[0]?.length || 1;
			const row = Array.from({ length: cols }, (_, c) => emptyDocxCell(`r${op.at}-c${c}`));
			table.rows.splice(op.at, 0, row);
			return `insertTableRow ${op.at}`;
		}
		case 'deleteTableRow': {
			const table = model.blocks[op.blockIndex];
			if (!table || table.type !== 'table') throw new Error(`not a table`);
			table.rows.splice(op.at, 1);
			return `deleteTableRow ${op.at}`;
		}
		case 'insertTableColumn': {
			const table = model.blocks[op.blockIndex];
			if (!table || table.type !== 'table') throw new Error(`not a table`);
			for (const [ri, row] of table.rows.entries()) {
				row.splice(op.at, 0, emptyDocxCell(`r${ri}-c${op.at}`));
			}
			return `insertTableColumn ${op.at}`;
		}
		case 'deleteTableColumn': {
			const table = model.blocks[op.blockIndex];
			if (!table || table.type !== 'table') throw new Error(`not a table`);
			for (const row of table.rows) row.splice(op.at, 1);
			return `deleteTableColumn ${op.at}`;
		}
		default:
			throw new Error(`unsupported docx op: ${op.op}`);
	}
}

function restoreDocxModel(model, snapModel) {
	const keys = new Set([...Object.keys(model), ...Object.keys(snapModel)]);
	for (const k of keys) {
		if (k in snapModel) model[k] = cloneModel(snapModel[k]);
		else delete model[k];
	}
}

async function runDocx(bytes, script) {
	const up = upstreamRoot();
	const { loadDocx } = await import(`${up}/src/core/docx/parse.ts`);

	const noopLoaded = await loadDocx(bytes);
	const noopSave = await noopLoaded.save();

	const loaded = await loadDocx(bytes);
	const model = loaded.model;
	/** @type {{ model: any, label: string }[]} */
	const past = [];
	/** @type {{ model: any, label: string }[]} */
	const future = [];

	/** @type {Checkpoint[]} */
	const checkpoints = [];
	let step = 0;
	const push = (phase, label) => {
		checkpoints.push({
			phase,
			step: step++,
			label,
			snapshot: snapshotDocx(model),
		});
	};

	const start = snapshotDocx(model);
	push('start', 'open');

	for (const op of script.ops) {
		past.push({ model: cloneModel(model), label: op.op });
		future.length = 0;
		const label = applyDocxOp(model, op);
		push('do', label);
	}
	const afterDos = snapshotDocx(model);

	while (past.length) {
		const entry = past.pop();
		future.push({ model: cloneModel(model), label: entry.label });
		restoreDocxModel(model, entry.model);
		push('undo', entry.label);
	}
	const afterUndoAll = snapshotDocx(model);
	const undoSave = await loaded.save(model);

	while (future.length) {
		const entry = future.pop();
		past.push({ model: cloneModel(model), label: entry.label });
		restoreDocxModel(model, entry.model);
		push('redo', entry.label);
	}
	const afterRedoAll = snapshotDocx(model);
	const redoSave = await loaded.save(model);

	return {
		checkpoints,
		start,
		afterDos,
		afterUndoAll,
		afterRedoAll,
		undoSave,
		redoSave,
		noopSave,
	};
}

async function runPptx(bytes, script) {
	const up = upstreamRoot();
	const { PptxHandler } = await import(`${up}/src/core/pptx/core/PptxHandler.ts`);
	const { cloneSlides } = await import(`${up}/src/core/pptx/editor/render/editor-mutations.ts`);
	const { EditorHistory } = await import(`${up}/src/core/pptx/editor/render/editor-history.ts`);
	const mut = await loadPptxMutators();

	const handler = new PptxHandler();
	const noopData = await handler.load(toArrayBuffer(bytes));
	const noopSave = await handler.save(noopData.slides, {
		headerFooter: noopData.headerFooter,
		tableStyles: noopData.tableStyleMap,
	});

	const data = await handler.load(toArrayBuffer(bytes));
	let slides = data.slides;
	const history = new EditorHistory({ maxDepth: 200 });
	const saveOpts = {
		headerFooter: data.headerFooter,
		tableStyles: data.tableStyleMap,
	};

	/** @type {Checkpoint[]} */
	const checkpoints = [];
	let step = 0;
	const push = (phase, label) => {
		checkpoints.push({
			phase,
			step: step++,
			label,
			snapshot: snapshotPptx(slides),
		});
	};

	const start = snapshotPptx(slides);
	push('start', 'open');

	for (const op of script.ops) {
		history.record(cloneSlides(slides), op.op);
		slides = applyPptxOps(slides, [op], mut);
		const label =
			op.op === 'setShapeText' || op.op === 'patchShapeGeometry' || op.op === 'setShapeTextStyle'
				? `${op.op} ${op.slideIndex}`
				: `${op.op} ${op.slideIndex ?? ''}`;
		push('do', label);
	}
	const afterDos = snapshotPptx(slides);

	while (history.canUndo) {
		const r = history.undo(cloneSlides(slides));
		if (!r) break;
		slides = r.snapshot;
		push('undo', r.label);
	}
	const afterUndoAll = snapshotPptx(slides);
	const undoSave = await handler.save(slides, saveOpts);

	while (history.canRedo) {
		const r = history.redo(cloneSlides(slides));
		if (!r) break;
		slides = r.snapshot;
		push('redo', r.label);
	}
	const afterRedoAll = snapshotPptx(slides);
	const redoSave = await handler.save(slides, saveOpts);

	return {
		checkpoints,
		start,
		afterDos,
		afterUndoAll,
		afterRedoAll,
		undoSave,
		redoSave,
		noopSave,
	};
}

export { snapshotsEqual, stableStringify };
