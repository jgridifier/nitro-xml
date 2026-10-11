// SPDX-License-Identifier: MIT
// umya (Rust) through the wasm-bindgen shim in adapters/rust/umya-shim. Instantiation happens in
// init(), outside the timed span; parse() includes copying the bytes into wasm memory.
import initWasm, { parse as parseWasm, extract as extractWasm } from '../../dist/rust/umya/shim.js';

let wasm;
export async function init() {
	wasm = await initWasm({ module_or_path: new URL('../../dist/rust/umya/shim_bg.wasm', import.meta.url) });
}
export const wasmMemoryBytes = () => wasm.memory.buffer.byteLength;
export const parse = (bytes) => parseWasm(bytes);
export const extract = (model) => JSON.parse(extractWasm(model));
export const dispose = (model) => model.free();
