// SPDX-License-Identifier: MIT
// Equivalence normalisers for the parse race. Every adapter's extract() builds a "raw" extraction
// in the shapes below; these functions turn it into the canonical form that is compared against
// the reference (upstream ooxml TS). The rules are documented in docs/scoreboard-method.md.
//
// xlsx raw:  { cells: [[sheetName, a1Ref, type, value], ...] }   type in n|s|b|e|d
// docx raw:  { paragraphs: [string, ...] }                        body + table-cell paragraphs, in order
// pptx raw:  { slides: [[string, ...], ...] }                     per slide, paragraphs in shape order

const EXCEL_EPOCH_MS = Date.UTC(1899, 11, 30);

/** Collapses every whitespace run (tabs, line breaks, NBSP) to one space and trims. */
export function normText(s) {
	return String(s ?? '')
		.replace(/\r\n?/g, '\n')
		.replace(/[\s ​]+/g, ' ')
		.trim()
		.normalize('NFC');
}

/** ISO date/time (or a JS Date) to an Excel 1900-system serial; dates normalise to numbers. */
export function dateToSerial(value) {
	const ms = value instanceof Date ? value.getTime() : Date.parse(/Z|[+-]\d\d:?\d\d$/.test(value) ? value : `${value}Z`);
	if (!Number.isFinite(ms)) return undefined;
	return (ms - EXCEL_EPOCH_MS) / 86400000;
}

/** Canonical xlsx cells: drops empty cells, maps d to n, sorts by (sheet, row, col). */
export function canonXlsx(raw) {
	const out = [];
	for (const [sheet, ref, type, value] of raw.cells) {
		if (value === null || value === undefined || value === '') continue;
		let t = type;
		let v = value;
		if (t === 'd') {
			const serial = dateToSerial(v);
			if (serial === undefined) continue;
			t = 'n';
			v = serial;
		}
		if (t === 'n') v = Number(v);
		else if (t === 'b') v = v === true || v === 1 || v === '1' || v === 'TRUE' || v === 'true' ? 'TRUE' : 'FALSE';
		else v = String(v).replace(/\r\n?/g, '\n');
		if (t === 's' && v === '') continue;
		out.push([String(sheet), String(ref).replace(/\$/g, '').toUpperCase(), t, v]);
	}
	const key = ([s, ref]) => {
		const m = /^([A-Z]+)(\d+)$/.exec(ref);
		let col = 0;
		for (const ch of m ? m[1] : '') col = col * 26 + ch.charCodeAt(0) - 64;
		return [s, m ? Number(m[2]) : 0, col];
	};
	out.sort((a, b) => {
		const [sa, ra, ca] = key(a);
		const [sb, rb, cb] = key(b);
		return sa < sb ? -1 : sa > sb ? 1 : ra - rb || ca - cb;
	});
	return out;
}

export function canonDocx(raw) {
	return raw.paragraphs.map(normText).filter((p) => p !== '');
}

export function canonPptx(raw) {
	return raw.slides.map((paras) => paras.map(normText).filter((p) => p !== ''));
}

export const canon = { xlsx: canonXlsx, docx: canonDocx, pptx: canonPptx };

function numEq(a, b) {
	if (a === b) return true;
	const scale = Math.max(Math.abs(a), Math.abs(b));
	return Math.abs(a - b) <= 1e-9 * scale;
}

const show = (x) => (x === undefined ? '(missing)' : JSON.stringify(x).slice(0, 200));

/** For two different strings: where they first differ, with 40 characters of context each side. */
function around(a, b) {
	if (typeof a !== 'string' || typeof b !== 'string') return undefined;
	let i = 0;
	while (i < a.length && i < b.length && a[i] === b[i]) i++;
	const from = Math.max(0, i - 40);
	return { at: i, reference: a.slice(from, i + 40), entrant: b.slice(from, i + 40) };
}

/** Returns null when equivalent, otherwise a description of the first difference. */
export function firstDifference(format, ref, got) {
	if (format === 'xlsx') {
		const n = Math.max(ref.length, got.length);
		for (let i = 0; i < n; i++) {
			const a = ref[i];
			const b = got[i];
			if (!a || !b || a[0] !== b[0] || a[1] !== b[1] || a[2] !== b[2]) {
				return { index: i, reference: show(a), entrant: show(b), counts: [ref.length, got.length] };
			}
			const same = a[2] === 'n' ? numEq(a[3], b[3]) : a[3] === b[3];
			if (!same) return { index: i, reference: show(a), entrant: show(b), counts: [ref.length, got.length] };
		}
		return null;
	}
	if (format === 'docx') {
		const n = Math.max(ref.length, got.length);
		for (let i = 0; i < n; i++) {
			if (ref[i] !== got[i]) {
				return { paragraph: i, reference: show(ref[i]), entrant: show(got[i]), context: around(ref[i], got[i]), counts: [ref.length, got.length] };
			}
		}
		return null;
	}
	const n = Math.max(ref.length, got.length);
	for (let s = 0; s < n; s++) {
		const a = ref[s];
		const b = got[s];
		if (!a || !b) return { slide: s + 1, reference: show(a), entrant: show(b), counts: [ref.length, got.length] };
		const m = Math.max(a.length, b.length);
		for (let i = 0; i < m; i++) {
			if (a[i] !== b[i]) return { slide: s + 1, paragraph: i, reference: show(a[i]), entrant: show(b[i]), context: around(a[i], b[i]) };
		}
	}
	return null;
}
