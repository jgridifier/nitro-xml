// SPDX-License-Identifier: MIT
//! WebAssembly boundary for Nitro XML: the only crate that depends on
//! wasm-bindgen. It exposes [`nitro_xml`] to the JS loader.
//!
//! Replaces the browser-facing entry points of ChristopherVR/ooxml@e66aaec `src/core/index.ts`
//! (the upstream TS engine itself never ships). No exports yet.

pub use nitro_xml;
