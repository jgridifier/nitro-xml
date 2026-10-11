// SPDX-License-Identifier: MIT
//! WebAssembly boundary for Nitro XML: the only crate that depends on
//! wasm-bindgen. It exposes [`nitro_xml`] to the JS loader.
//!
//! Replaces the browser-facing entry points of ChristopherVR/ooxml@e66aaec `src/core/index.ts`
//! (the upstream TS engine itself never ships).

pub use nitro_xml;
use wasm_bindgen::prelude::*;

/// Version of the Nitro XML engine (the crate version), for the loader and diagnostics.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// Routes Rust panics to `console.error` in debug builds only; release builds keep the hook out.
#[cfg(debug_assertions)]
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// Shape of every engine handle: created by `open`-style calls, owned by JS, and released with the
/// generated `free()` (wasm memory is not garbage collected). The real document handles
/// (`open → handle`, queries, commands, `save`, `free`) replace this example.
#[wasm_bindgen]
pub struct Handle {
    bytes: Vec<u8>,
}

#[wasm_bindgen]
impl Handle {
    /// Takes a copy of the document bytes (one bulk copy across the boundary).
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Handle {
        Handle {
            bytes: bytes.to_vec(),
        }
    }

    /// Size of the held document in bytes.
    #[wasm_bindgen(getter, js_name = byteLength)]
    pub fn byte_length(&self) -> usize {
        self.bytes.len()
    }
}
