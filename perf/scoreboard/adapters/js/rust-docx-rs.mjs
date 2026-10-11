// SPDX-License-Identifier: MIT
// docx-rs (Rust) through the wasm-bindgen shim in adapters/rust/docx-rs-shim. Instantiation happens
// in init(), outside the timed span; parse() includes copying the bytes into wasm memory.
// extract() walks docx-rs's own JSON model (Docx::json).
import initWasm, { parse as parseWasm, extract as extractWasm } from '../../dist/rust/docx-rs/shim.js';

let wasm;
export async function init() {
	wasm = await initWasm({ module_or_path: new URL('../../dist/rust/docx-rs/shim_bg.wasm', import.meta.url) });
}
export const wasmMemoryBytes = () => wasm.memory.buffer.byteLength;
export const parse = (bytes) => parseWasm(bytes);
export const dispose = (model) => model.free();

// Not body text in the reference: deletions, moved-from text, field instructions, drawings
// (text boxes), comments.
const SKIP = new Set(['delete', 'deleteText', 'moveFrom', 'instrText', 'drawing', 'shape', 'comment', 'commentRangeStart', 'commentRangeEnd', 'footnoteReference']);

function text(node) {
	if (node === null || typeof node !== 'object') return '';
	if (Array.isArray(node)) return node.map(text).join('');
	if (node.type && SKIP.has(node.type)) return '';
	if (node.type === 'text' || node.type === 'sym') return node.data?.text ?? node.data?.char ?? '';
	if (node.type === 'tab' || node.type === 'ptab' || node.type === 'break') return ' ';
	let s = '';
	for (const [k, v] of Object.entries(node)) if (k === 'data' || k === 'children') s += text(v);
	return s;
}

function blocks(children, out) {
	for (const node of children ?? []) {
		if (node.type === 'paragraph') out.push(text(node.data));
		else if (node.type === 'table') {
			for (const row of node.data.rows ?? []) {
				for (const cell of row.data?.cells ?? []) {
					for (const c of cell.data?.children ?? []) if (c.type === 'paragraph') out.push(text(c.data));
				}
			}
		} else if (node.type === 'structuredDataTag') blocks(node.data?.children, out);
	}
}

export function extract(model) {
	const doc = JSON.parse(extractWasm(model));
	const out = [];
	blocks(doc.document.children, out);
	return { paragraphs: out };
}
