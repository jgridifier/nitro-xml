// SPDX-License-Identifier: MIT
// nitro-xml-docx through the wasm-bindgen shim in adapters/rust/nitro-docx-shim.
import initWasm, { parse as parseWasm, extract as extractWasm } from '../../dist/rust/nitro-docx/shim.js';

let wasm;
export async function init() {
	wasm = await initWasm({ module_or_path: new URL('../../dist/rust/nitro-docx/shim_bg.wasm', import.meta.url) });
}
export const wasmMemoryBytes = () => wasm.memory.buffer.byteLength;
export const parse = (bytes) => parseWasm(bytes);
export const dispose = (model) => model.free();

const HIDDEN = new Set(['delete', 'moveFrom']);
const paraText = (p) =>
	p.runs
		.filter((r) => !(r.revision && HIDDEN.has(r.revision.kind)))
		.map((r) => r.text ?? '')
		.join('');

export function extract(model) {
	const { blocks } = JSON.parse(extractWasm(model));
	const paragraphs = [];
	for (const block of blocks) {
		if (block.type === 'paragraph') paragraphs.push(paraText(block));
		else if (block.type === 'table') {
			for (const row of block.rows) for (const cell of row) for (const p of cell.paragraphs) paragraphs.push(paraText(p));
		}
	}
	return { paragraphs };
}
