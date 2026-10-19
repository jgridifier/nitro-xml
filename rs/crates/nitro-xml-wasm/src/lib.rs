// SPDX-License-Identifier: MIT
//! WebAssembly boundary for Nitro XML: the only crate that depends on
//! wasm-bindgen. It exposes [`nitro_xml`] to the JS loader.
//!
//! Boundary conventions (#14) live in [`handle`], [`transport`] and [`events`].
//! Per-frame command-buffer types, the measure batch and post-apply layout
//! read-back live in [`nitro_xml_render`] (shared with format painters). Bench-only synthetic
//! grids and serde/JSON export arms are behind the non-default `bench` feature
//! and are never in the shipped package. The minimal loader that applies
//! command buffers and forwards events is #37.
//!
//! Replaces the browser-facing entry points of ChristopherVR/ooxml@e66aaec `src/core/index.ts`
//! (the upstream TS engine itself never ships).

pub use nitro_xml;
use wasm_bindgen::prelude::*;

#[cfg(feature = "bench")]
mod bench;
mod chrome;
mod docx;
mod docx_edit;
mod edit;
mod events;
mod handle;
mod office_edit;
mod pptx;
mod pptx_edit;
mod shell;
mod text;
mod transport;
mod xlsx;
mod xlsx_edit;

pub use handle::{Handle, OoxmlError, Warning};

/// Version of the Nitro XML engine (the crate version), for the loader and diagnostics.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// Lazy editor chrome module hook for the loader (`loadEditorModule`).
#[wasm_bindgen(js_name = loadEditorChromeModule)]
pub fn load_editor_chrome_module() -> bool {
    let _ = chrome::SHORTCUTS.len();
    true
}

/// Diagnostic part graph for an OOXML package (`Package::open` + content types / rels).
///
/// Returns JSON: format, parts, defaults, overrides, rels, targets, signatures, properties.
/// On failure returns [`OoxmlError`] with `code` `unsupported_format` (and message = detected)
/// or a zip/xml kind.
#[wasm_bindgen(js_name = inspectPackage)]
pub fn inspect_package(bytes: &[u8]) -> Result<String, OoxmlError> {
    use nitro_xml::opc::opc::{OpcError, Package};
    match Package::open(bytes) {
        Ok(mut pkg) => pkg.inspect_json().map_err(|e| match e {
            OpcError::UnsupportedFormat { detected } => {
                OoxmlError::with_code("unsupported_format", detected)
            }
            other => OoxmlError::with_code(other.kind(), other.to_string()),
        }),
        Err(OpcError::UnsupportedFormat { detected }) => {
            Err(OoxmlError::with_code("unsupported_format", detected))
        }
        Err(other) => Err(OoxmlError::with_code(other.kind(), other.to_string())),
    }
}

/// Routes Rust panics to `console.error` in debug builds only; release builds keep the hook out.
#[cfg(debug_assertions)]
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}
