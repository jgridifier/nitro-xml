// SPDX-License-Identifier: MIT
/**
 * Undo/redo-aware apply of edits/*.json ops against the pinned upstream TS editor (#51).
 * Captures canonical model snapshots after every do / undo / redo step.
 */

import { join } from 'node:path';
import { applyPptxOps, loadPptxMutators } from '../edit/apply.mjs';
import { ROOT } from '../edit/scripts.mjs';
import { applyDocxOp } from '../edit/apply.mjs';
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
	// XLSX_RECALC=1 matches the Rust editor calculator (#77). The undo-parity
	// save gate leaves this unset so cached formula values stay as loaded.
	const session = createEditSession(workbook, {
		recalc: process.env.XLSX_RECALC === '1',
	});
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
	const noopSave = script.modelOnly ? new Uint8Array(0) : await noopLoaded.save();

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
	const undoSave = script.modelOnly ? new Uint8Array(0) : await loaded.save(model);

	while (future.length) {
		const entry = future.pop();
		past.push({ model: cloneModel(model), label: entry.label });
		restoreDocxModel(model, entry.model);
		push('redo', entry.label);
	}
	const afterRedoAll = snapshotDocx(model);
	const redoSave = script.modelOnly ? new Uint8Array(0) : await loaded.save(model);

	return {
		checkpoints,
		start,
		afterDos,
		afterUndoAll,
		afterRedoAll,
		undoSave,
		redoSave,
		noopSave,
		modelOnly: !!script.modelOnly,
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
