// SPDX-License-Identifier: MIT
// nitro-xml-pptx through the wasm-bindgen shim in adapters/rust/nitro-pptx-shim.
// Extract is already scoreboard-shaped (`{ slides: [[paragraph, ...], ...] }`).
import initWasm, { parse as parseWasm, extract as extractWasm } from '../../dist/rust/nitro-pptx/shim.js';

let wasm;
export async function init() {
	wasm = await initWasm({ module_or_path: new URL('../../dist/rust/nitro-pptx/shim_bg.wasm', import.meta.url) });
}
export const wasmMemoryBytes = () => wasm.memory.buffer.byteLength;
export const parse = (bytes) => parseWasm(bytes);
export const extract = (model) => JSON.parse(extractWasm(model));
export const dispose = (model) => model.free();
