// SPDX-License-Identifier: MIT
//! WebAssembly boundary for Nitro XML: the only crate that depends on
//! wasm-bindgen. It exposes [`nitro_xml`] to the JS loader.
//!
//! Boundary conventions (#14) live in [`handle`], [`transport`] and [`events`].
//! The minimal loader that applies command buffers and forwards events is #37.
//!
//! Replaces the browser-facing entry points of ChristopherVR/ooxml@e66aaec `src/core/index.ts`
//! (the upstream TS engine itself never ships).

pub use nitro_xml;
use wasm_bindgen::prelude::*;

mod events;
mod handle;
mod transport;

pub use handle::{Handle, OoxmlError, Warning};

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
