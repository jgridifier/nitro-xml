// SPDX-License-Identifier: MIT
// nitro-xml-xlsx through the wasm-bindgen shim in adapters/rust/nitro-xlsx-shim.
import initWasm, { parse as parseWasm, extract as extractWasm } from '../../dist/rust/nitro-xlsx/shim.js';

let wasm;
export async function init() {
	wasm = await initWasm({ module_or_path: new URL('../../dist/rust/nitro-xlsx/shim_bg.wasm', import.meta.url) });
}
export const wasmMemoryBytes = () => wasm.memory.buffer.byteLength;
export const parse = (bytes) => parseWasm(bytes);
export const extract = (model) => JSON.parse(extractWasm(model));
export const dispose = (model) => model.free();
