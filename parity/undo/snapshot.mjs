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
		s: cell.s ?? cell.style ?? null,
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
							runs: (p.runs || []).map((r) => ({
								text: r.text ?? '',
								bold: !!r.bold,
								italic: !!r.italic,
								underline: !!r.underline,
								fontSize: r.fontSize ?? null,
								color: r.color ?? null,
							})),
						})),
					})),
				),
			};
		}
		return {
			index,
			type: b.type || 'paragraph',
			align: b.align ?? null,
			runs: (b.runs || []).map((r) => ({
				text: r.text ?? '',
				bold: !!r.bold,
				italic: !!r.italic,
				underline: !!r.underline,
				fontSize: r.fontSize ?? null,
				color: r.color ?? null,
			})),
		};
	});
	return { format: 'docx', blocks };
}

/**
 * Pptx: per-slide element geometry + text + textStyle.
 * @param {any[]} slides
 */
export function snapshotPptx(slides) {
	return {
		format: 'pptx',
		slides: (slides || []).map((slide, slideIndex) => ({
			slideIndex,
			elements: (slide.elements || []).map((el) => ({
				id: el.id,
				x: el.x ?? 0,
				y: el.y ?? 0,
				width: el.width ?? 0,
				height: el.height ?? 0,
				rotation: el.rotation ?? 0,
				text: el.text ?? null,
				textStyle: el.textStyle
					? {
							bold: !!el.textStyle.bold,
							italic: !!el.textStyle.italic,
							fontSize: el.textStyle.fontSize ?? null,
							color: el.textStyle.color ?? null,
						}
					: null,
				textSegments: (el.textSegments || []).map((seg) => ({
					text: seg.text ?? '',
					bold: !!seg.bold,
					italic: !!seg.italic,
					fontSize: seg.fontSize ?? null,
					color: seg.color ?? null,
				})),
			})),
		})),
	};
}

export function snapshotsEqual(a, b) {
	return stableStringify(a) === stableStringify(b);
}
