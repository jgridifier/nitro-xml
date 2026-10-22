// SPDX-License-Identifier: MIT
// @betteroffice/docx 0.4.3 — Rust/WASM parseDocx (parse + opc wasm only; layout/edit unused).
// Fastest documented parse path: parseDocx(bytes, { preloadFonts: false }).
// Instantiation of wasm is outside the timed span (preload in init).
import { parseDocx } from '@betteroffice/docx/docx';

let memories = null;

export async function init() {
	// Warm opc+parse wasm outside the timed span (layout/edit stay unused).
	await parseDocx(buildMinimalDocx(), {
		preloadFonts: false,
		parseHeadersFooters: false,
		parseNotes: false,
	});
	try {
		const mod = await import('@betteroffice/docx');
		memories = mod.wasmModuleMemories ?? null;
	} catch {
		memories = null;
	}
}

export const wasmMemoryBytes = () => {
	if (!memories) return null;
	try {
		const rows = memories();
		return rows.reduce((n, r) => n + (r.bufferBytes || 0), 0);
	} catch {
		return null;
	}
};

export const parse = (bytes) =>
	parseDocx(bytes, {
		preloadFonts: false,
		parseHeadersFooters: false,
		parseNotes: false,
	});

// Deletions / move-from are not body text in the upstream reference.
const SKIP_RUN = new Set(['deletion', 'moveFrom', 'moveFromRangeStart', 'moveFromRangeEnd']);

function kids(node) {
	return node?.content ?? node?.children ?? [];
}

function runText(node) {
	if (!node || typeof node !== 'object') return '';
	if (SKIP_RUN.has(node.type)) return '';
	if (node.type === 'text') return node.text ?? '';
	if (node.type === 'tab' || node.type === 'ptab') return ' ';
	if (node.type === 'break') return node.break === 'textWrapping' || !node.break ? ' ' : '';
	if (node.type === 'noBreakHyphen') return '‑';
	if (node.type === 'softHyphen') return '';
	if (node.type === 'symbol') return node.char ?? node.text ?? '';
	let s = '';
	for (const c of kids(node)) s += runText(c);
	return s;
}

function paraText(p) {
	let s = '';
	for (const c of kids(p)) {
		if (c.type === 'deletion' || c.type === 'moveFrom') continue;
		if (c.type === 'run') s += runText(c);
		else if (
			c.type === 'hyperlink' ||
			c.type === 'smartTag' ||
			c.type === 'simpleField' ||
			c.type === 'insertion' ||
			c.type === 'moveTo'
		) {
			for (const r of kids(c)) s += runText(r);
		} else if (c.type === 'fldChar' || c.type === 'instrText') {
			// field instructions are not body text in the reference
		} else {
			// Unknown inline: still walk children (covers field results, bookmarks, etc.)
			s += runText(c);
		}
	}
	return s;
}

function blocks(list, out) {
	for (const b of list ?? []) {
		if (b.type === 'paragraph') out.push(paraText(b));
		else if (b.type === 'table') {
			for (const row of b.rows ?? []) {
				for (const cell of row.cells ?? []) {
					blocks(kids(cell), out);
				}
			}
		} else if (b.type === 'sdt' || b.type === 'structuredDocumentTag' || b.type === 'blockSdt') {
			blocks(kids(b), out);
		}
	}
}

export function extract(doc) {
	const out = [];
	blocks(doc.package.document.content, out);
	return { paragraphs: out };
}

/** Minimal valid DOCX so init can warm opc+parse wasm without createDocx/edit. */
function buildMinimalDocx() {
	const enc = new TextEncoder();
	const files = [
		[
			'[Content_Types].xml',
			enc.encode(
				'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>' +
					'<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">' +
					'<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>' +
					'<Default Extension="xml" ContentType="application/xml"/>' +
					'<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>' +
					'</Types>',
			),
		],
		[
			'_rels/.rels',
			enc.encode(
				'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>' +
					'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">' +
					'<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>' +
					'</Relationships>',
			),
		],
		[
			'word/document.xml',
			enc.encode(
				'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>' +
					'<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">' +
					'<w:body><w:p><w:r><w:t>x</w:t></w:r></w:p><w:sectPr/></w:body></w:document>',
			),
		],
		[
			'word/_rels/document.xml.rels',
			enc.encode(
				'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>' +
					'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>',
			),
		],
	];
	return zipStore(files);
}

function crc32(buf) {
	let c = ~0;
	for (let i = 0; i < buf.length; i++) {
		c ^= buf[i];
		for (let k = 0; k < 8; k++) c = c & 1 ? (c >>> 1) ^ 0xedb88320 : c >>> 1;
	}
	return ~c >>> 0;
}

function u16(n) {
	const b = new Uint8Array(2);
	new DataView(b.buffer).setUint16(0, n, true);
	return b;
}
function u32(n) {
	const b = new Uint8Array(4);
	new DataView(b.buffer).setUint32(0, n, true);
	return b;
}

function zipStore(files) {
	const parts = [];
	const centrals = [];
	let offset = 0;
	for (const [name, data] of files) {
		const nameBytes = new TextEncoder().encode(name);
		const crc = crc32(data);
		const local = new Uint8Array([
			...u32(0x04034b50),
			...u16(20),
			...u16(0),
			...u16(0), // store
			...u16(0),
			...u16(0),
			...u32(crc),
			...u32(data.length),
			...u32(data.length),
			...u16(nameBytes.length),
			...u16(0),
			...nameBytes,
			...data,
		]);
		parts.push(local);
		const central = new Uint8Array([
			...u32(0x02014b50),
			...u16(20),
			...u16(20),
			...u16(0),
			...u16(0),
			...u16(0),
			...u16(0),
			...u32(crc),
			...u32(data.length),
			...u32(data.length),
			...u16(nameBytes.length),
			...u16(0),
			...u16(0),
			...u16(0),
			...u16(0),
			...u32(0),
			...u32(offset),
			...nameBytes,
		]);
		centrals.push(central);
		offset += local.length;
	}
	const centralSize = centrals.reduce((n, c) => n + c.length, 0);
	const end = new Uint8Array([
		...u32(0x06054b50),
		...u16(0),
		...u16(0),
		...u16(files.length),
		...u16(files.length),
		...u32(centralSize),
		...u32(offset),
		...u16(0),
	]);
	const total = offset + centralSize + end.length;
	const out = new Uint8Array(total);
	let o = 0;
	for (const p of parts) {
		out.set(p, o);
		o += p.length;
	}
	for (const c of centrals) {
		out.set(c, o);
		o += c.length;
	}
	out.set(end, o);
	return out;
}
