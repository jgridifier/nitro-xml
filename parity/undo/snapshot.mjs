// SPDX-License-Identifier: MIT
/**
 * Canonical model snapshots for gate (c) (#51).
 * Stable JSON projections — not full in-memory objects — so TS and WASM can match.
 */

/** Stable stringify: sorted object keys, undefined dropped. */
export function stableStringify(value) {
	return JSON.stringify(value, replacer);
}

function replacer(_key, v) {
	if (v === undefined) return undefined;
	if (v && typeof v === 'object' && !Array.isArray(v) && !(v instanceof Uint8Array)) {
		const out = {};
		for (const k of Object.keys(v).sort()) out[k] = v[k];
		return out;
	}
	if (typeof v === 'bigint') return v.toString();
	return v;
}

/**
 * Xlsx: sheet names + sparse cell map (value + style id + formula).
 * @param {any} workbook
 */
function projectCell(cell) {
	if (!cell) return null;
	return {
		v: cell.v ?? cell.value ?? null,
		t: cell.t ?? cell.type ?? null,
		f: cell.f ?? cell.formula ?? null,
		// Upstream Cell uses styleId; keep s/style aliases for hand dumps.
		s: cell.styleId ?? cell.s ?? cell.style ?? null,
	};
}

/** Upstream sheet.rows: Map<row, Map<col, cell>>. */
export function snapshotXlsx(workbook) {
	const sheets = (workbook.sheets || []).map((sh, index) => {
		const cells = [];
		const rows = sh.rows;
		if (rows && typeof rows.forEach === 'function') {
			rows.forEach((rowMap, r) => {
				if (rowMap && typeof rowMap.forEach === 'function') {
					rowMap.forEach((cell, c) => {
						const p = projectCell(cell);
						if (!p) return;
						cells.push({ ref: `${r}:${c}`, ...p });
					});
				}
			});
		}
		cells.sort((a, b) => (a.ref < b.ref ? -1 : a.ref > b.ref ? 1 : 0));
		return {
			index,
			name: sh.name ?? `Sheet${index + 1}`,
			state: sh.state ?? null,
			cells,
		};
	});
	return { format: 'xlsx', sheets };
}

/**
 * Docx: blocks projection (paragraphs + tables).
 * @param {any} model
 */
function projectRevision(rev) {
	if (!rev) return null;
	return {
		id: rev.id,
		kind: rev.kind,
		author: rev.author,
		...(rev.move ? { move: rev.move } : {}),
	};
}

function projectRun(r) {
	return {
		text: r.text ?? '',
		bold: !!r.bold,
		italic: !!r.italic,
		underline: !!r.underline,
		fontSize: r.fontSize ?? null,
		color: r.color ?? null,
		commentIds: r.commentIds ?? null,
		revision: projectRevision(r.revision),
		formatRevision: projectRevision(r.formatRevision),
	};
}

function projectComments(comments) {
	return (comments || []).map((c) => ({
		id: c.id ?? null,
		author: c.author ?? null,
		text: c.text ?? null,
		resolved: !!c.resolved,
		parentId: c.parentId ?? null,
	}));
}

export function snapshotDocx(model) {
	const blocks = (model.blocks || []).map((b, index) => {
		if (b.type === 'table') {
			return {
				index,
				type: 'table',
				rows: (b.rows || []).map((row) =>
					(row || []).map((cell) => ({
						paragraphs: (cell.paragraphs || []).map((p) => ({
							align: p.align ?? null,
							markRevision: projectRevision(p.markRevision),
							formatRevision: projectRevision(p.formatRevision),
							runs: (p.runs || []).map(projectRun),
						})),
					})),
				),
			};
		}
		return {
			index,
			type: b.type || 'paragraph',
			align: b.align ?? null,
			markRevision: projectRevision(b.markRevision),
			formatRevision: projectRevision(b.formatRevision),
			runs: (b.runs || []).map(projectRun),
		};
	});
	const snap = { format: 'docx', blocks };
	if (model.comments) snap.comments = projectComments(model.comments);
	return snap;
}

/**
 * Pptx: per-slide element geometry + text + textStyle, plus notes and the
 * thumbnail rail (slide order + id; preview bytes stay null).
 * @param {any[]} slides
 */
function projectBullet(info) {
	if (!info) return null;
	/** @type {Record<string, unknown>} */
	const out = {};
	if (info.char) out.char = info.char;
	if (info.autoNumType) out.autoNumType = info.autoNumType;
	if (info.autoNumType) {
		if (info.autoNumStartAt != null) out.autoNumStartAt = info.autoNumStartAt;
		if (info.paragraphIndex != null) out.paragraphIndex = info.paragraphIndex;
	}
	if (info.none) out.none = true;
	return Object.keys(out).length ? out : null;
}

function projectSegment(seg) {
	const style = seg.style || {};
	/** @type {Record<string, unknown>} */
	const out = {
		text: seg.text ?? '',
		bold: !!(seg.bold ?? style.bold),
		italic: !!(seg.italic ?? style.italic),
		fontSize: seg.fontSize ?? style.fontSize ?? null,
		color: seg.color ?? style.color ?? null,
	};
	if (seg.underline ?? style.underline) out.underline = true;
	const align = seg.align ?? seg.paragraphProperties?.align;
	if (align) out.align = align;
	const bullet = projectBullet(seg.bulletInfo);
	if (bullet) out.bulletInfo = bullet;
	return out;
}

function projectTextStyle(ts) {
	if (!ts) return null;
	/** @type {Record<string, unknown>} */
	const out = {
		bold: !!ts.bold,
		italic: !!ts.italic,
		fontSize: ts.fontSize ?? null,
		color: ts.color ?? null,
	};
	if (ts.autoFit !== undefined) out.autoFit = !!ts.autoFit;
	if (ts.autoFitMode) out.autoFitMode = ts.autoFitMode;
	return out;
}

function projectTableCell(cell) {
	/** @type {Record<string, unknown>} */
	const out = { text: cell.text ?? '' };
	if ((cell.gridSpan ?? 1) > 1) out.gridSpan = cell.gridSpan;
	if ((cell.rowSpan ?? 1) > 1) out.rowSpan = cell.rowSpan;
	if (cell.hMerge) out.hMerge = true;
	if (cell.vMerge) out.vMerge = true;
	return out;
}

function projectTable(table) {
	if (!table) return null;
	return {
		columnWidths: table.columnWidths || [],
		rows: (table.rows || []).map((row) => {
			/** @type {Record<string, unknown>} */
			const out = {
				cells: (row.cells || []).map(projectTableCell),
			};
			if (row.height) out.height = row.height;
			return out;
		}),
	};
}

function projectPptxElement(el) {
	const style = el.shapeStyle || {};
	const hasChildren = el.type === 'group' || Array.isArray(el.children);
	/** @type {Record<string, unknown>} */
	const out = {
		id: el.id,
		x: el.x ?? 0,
		y: el.y ?? 0,
		width: el.width ?? 0,
		height: el.height ?? 0,
		rotation: el.rotation ?? 0,
		text: el.text ?? null,
		textStyle: projectTextStyle(el.textStyle),
		textSegments: (el.textSegments || []).map(projectSegment),
		fillColor: el.fillColor ?? style.fillColor ?? null,
		fillMode: el.fillMode ?? style.fillMode ?? null,
		strokeColor: el.strokeColor ?? style.strokeColor ?? null,
		strokeWidth: el.strokeWidth ?? style.strokeWidth ?? null,
		children: hasChildren ? (el.children || []).map(projectPptxElement) : null,
	};
	const table = projectTable(el.tableData);
	if (table) out.tableData = table;
	return out;
}

export function snapshotPptx(slides) {
	const projected = (slides || []).map((slide, slideIndex) => ({
		slideIndex,
		id: slide.id ?? null,
		notes: slide.notes ?? null,
		elements: (slide.elements || []).map(projectPptxElement),
	}));
	return {
		format: 'pptx',
		slides: projected,
		thumbnails: projected.map((slide) => ({
			slideIndex: slide.slideIndex,
			id: slide.id,
			preview: null,
		})),
	};
}

export function snapshotsEqual(a, b) {
	return stableStringify(a) === stableStringify(b);
}
