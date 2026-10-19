// SPDX-License-Identifier: MIT
// nitro-xml-pptx through the wasm-bindgen shim in adapters/rust/nitro-pptx-shim.
import initWasm, { parse as parseWasm, extract as extractWasm } from '../../dist/rust/nitro-pptx/shim.js';

let wasm;
export async function init() {
	wasm = await initWasm({ module_or_path: new URL('../../dist/rust/nitro-pptx/shim_bg.wasm', import.meta.url) });
}
export const wasmMemoryBytes = () => wasm.memory.buffer.byteLength;
export const parse = (bytes) => parseWasm(bytes);
export const dispose = (model) => model.free();

function walk(elements, out) {
	for (const e of elements ?? []) {
		if (typeof e.text === 'string' && e.type !== 'picture') out.push(...e.text.split('\n'));
		if (e.type === 'table' && e.tableData) {
			for (const row of e.tableData.rows ?? []) {
				for (const cell of row.cells ?? []) {
					if (typeof cell.text === 'string') out.push(...cell.text.split('\n'));
				}
			}
		}
		if (e.children) walk(e.children, out);
	}
}

export function extract(model) {
	const data = JSON.parse(extractWasm(model));
	return {
		slides: data.slides.map((slide) => {
			const out = [];
			walk(slide.elements, out);
			return out;
		}),
	};
}
