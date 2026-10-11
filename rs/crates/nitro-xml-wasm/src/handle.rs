// SPDX-License-Identifier: MIT
//! Document handle conventions for the JS/WASM boundary (#14).
//!
//! Handles own the model in linear memory (`open → handle`, queries, commands,
//! `save`, `free`). The loader (#37) must also register each handle with a
//! `FinalizationRegistry` that calls `free()` if the embedder drops the last JS
//! reference without an explicit free. Crash recovery reloads into a fresh
//! worker (PRODUCT_SPEC.md §5(e)).

use crate::events::{self, EventQueue};
use crate::transport::StringTable;
use js_sys::Array;
use nitro_xml_render::{CommandBuffer, MeasureBatch};
use wasm_bindgen::prelude::*;

/// Typed engine error surfaced to JS. Stable `code` strings are the machine API;
/// `message` is for humans; `part` names an OPC part when the failure is part-local.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct OoxmlError {
    code: String,
    message: String,
    part: Option<String>,
}

#[wasm_bindgen]
impl OoxmlError {
    #[wasm_bindgen(constructor)]
    pub fn new(code: String, message: String, part: Option<String>) -> OoxmlError {
        OoxmlError {
            code,
            message,
            part,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn code(&self) -> String {
        self.code.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn part(&self) -> Option<String> {
        self.part.clone()
    }
}

impl OoxmlError {
    pub(crate) fn with_code(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
            part: None,
        }
    }
}

/// Non-fatal diagnostic. Same shape as [`OoxmlError`]; collected on the handle.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct Warning {
    code: String,
    message: String,
    part: Option<String>,
}

#[wasm_bindgen]
impl Warning {
    #[wasm_bindgen(constructor)]
    pub fn new(code: String, message: String, part: Option<String>) -> Warning {
        Warning {
            code,
            message,
            part,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn code(&self) -> String {
        self.code.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn part(&self) -> Option<String> {
        self.part.clone()
    }
}

/// Document handle: owns bytes and boundary state for one open package.
///
/// Created by bulk transfer of an `ArrayBuffer` (worker: transfer zero-copy into
/// the worker, then one copy into wasm linear memory). Explicit `free()` (from
/// wasm-bindgen) releases the Rust side; see module docs for the
/// `FinalizationRegistry` safety net (#37).
#[wasm_bindgen]
pub struct Handle {
    bytes: Vec<u8>,
    warnings: Vec<Warning>,
    pub(crate) strings: StringTable,
    pub(crate) commands: CommandBuffer,
    pub(crate) measures: MeasureBatch,
    pub(crate) events: EventQueue,
    #[cfg(feature = "bench")]
    pub(crate) grid: crate::bench::SyntheticGrid,
}

#[wasm_bindgen]
impl Handle {
    /// Takes a copy of the document bytes (one bulk copy across the boundary).
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Handle {
        Handle {
            bytes: bytes.to_vec(),
            warnings: Vec::new(),
            strings: StringTable::new(),
            commands: CommandBuffer::new(),
            measures: MeasureBatch::new(),
            events: EventQueue::new(),
            #[cfg(feature = "bench")]
            grid: crate::bench::SyntheticGrid::new(),
        }
    }

    /// Size of the held document in bytes.
    #[wasm_bindgen(getter, js_name = byteLength)]
    pub fn byte_length(&self) -> usize {
        self.bytes.len()
    }

    /// Non-fatal warnings accumulated while opening or querying.
    pub fn warnings(&self) -> Array {
        let out = Array::new();
        for w in &self.warnings {
            out.push(&JsValue::from(w.clone()));
        }
        out
    }

    /// Warning count without crossing into a JS Array (usable from native tests).
    #[wasm_bindgen(getter, js_name = warningCount)]
    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }

    /// Append a warning (used by parsers).
    #[wasm_bindgen(js_name = pushWarning)]
    pub fn push_warning(&mut self, code: String, message: String, part: Option<String>) {
        self.warnings.push(Warning::new(code, message, part));
    }

    /// Intern `s` into this document's string table; returns a stable id.
    pub fn intern(&mut self, s: &str) -> u32 {
        self.strings.intern(s)
    }

    /// Resolve an interned string id, or `undefined` if unknown.
    pub fn resolve(&self, id: u32) -> Option<String> {
        self.strings.resolve(id).map(str::to_owned)
    }

    /// Number of interned strings (including id 0 = empty).
    #[wasm_bindgen(getter, js_name = stringCount)]
    pub fn string_count(&self) -> u32 {
        self.strings.len()
    }

    /// Start a new frame's command buffer (discards any unflushed ops).
    #[wasm_bindgen(js_name = beginFrame)]
    pub fn begin_frame(&mut self) {
        self.commands.begin();
    }

    /// Take the current frame's command bytes (loader applies them in one pass).
    #[wasm_bindgen(js_name = takeFrameCommands)]
    pub fn take_frame_commands(&mut self) -> Vec<u8> {
        self.commands.take()
    }

    /// Queue a text-measurement request for the next layout batch.
    #[wasm_bindgen(js_name = pushMeasureReq)]
    pub fn push_measure_req(&mut self, font_id: u32, size_px: f32, text_id: u32) {
        self.measures.push(font_id, size_px, text_id);
    }

    /// Number of pending measure requests.
    #[wasm_bindgen(getter, js_name = measureCount)]
    pub fn measure_count(&self) -> usize {
        self.measures.len()
    }

    #[wasm_bindgen(js_name = measureFontPtr)]
    pub fn measure_font_ptr(&self) -> *const u32 {
        self.measures.font_id.as_ptr()
    }

    #[wasm_bindgen(js_name = measureSizePtr)]
    pub fn measure_size_ptr(&self) -> *const f32 {
        self.measures.size_px.as_ptr()
    }

    #[wasm_bindgen(js_name = measureTextPtr)]
    pub fn measure_text_ptr(&self) -> *const u32 {
        self.measures.text_id.as_ptr()
    }

    /// Loader writes measured widths back (one `f64` per pending request).
    #[wasm_bindgen(js_name = setMeasureResults)]
    pub fn set_measure_results(&mut self, widths: &[f64]) -> Result<(), JsValue> {
        self.measures.set_results(widths).map_err(|e| {
            JsValue::from(OoxmlError::with_code("measure_len_mismatch", e.to_string()))
        })
    }

    #[wasm_bindgen(js_name = measureWidthPtr)]
    pub fn measure_width_ptr(&self) -> *const f64 {
        self.measures.widths.as_ptr()
    }

    /// Decode and enqueue a compact event batch from the loader (≤1 batch/frame).
    #[wasm_bindgen(js_name = pushEventBatch)]
    pub fn push_event_batch(&mut self, bytes: &[u8]) -> Result<u32, JsValue> {
        match events::decode_batch(bytes) {
            Ok(batch) => {
                let n = batch.len() as u32;
                self.events.extend(batch);
                Ok(n)
            }
            Err(e) => Err(JsValue::from(e)),
        }
    }

    /// Number of events waiting for the engine to drain.
    #[wasm_bindgen(getter, js_name = pendingEventCount)]
    pub fn pending_event_count(&self) -> usize {
        self.events.len()
    }

    /// Drain pending events (engine tick). Returns how many were dropped.
    #[wasm_bindgen(js_name = drainEvents)]
    pub fn drain_events(&mut self) -> u32 {
        self.events.drain() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ooxml_error_part_is_optional() {
        // Catches: JS consumers must treat missing `part` as undefined, not "".
        let e = OoxmlError::new("unsupported_format".into(), "CSV".into(), None);
        assert!(e.part().is_none());
        let e2 = OoxmlError::new(
            "bad_part".into(),
            "x".into(),
            Some("xl/workbook.xml".into()),
        );
        assert_eq!(e2.part().as_deref(), Some("xl/workbook.xml"));
    }

    #[test]
    fn intern_deduplicates_per_document() {
        // Catches: duplicate strings must reuse the same id (interned table, not per-call copies).
        let mut h = Handle::new(&[]);
        let a = h.intern("Calibri");
        let b = h.intern("Calibri");
        let c = h.intern("Arial");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(h.resolve(a).as_deref(), Some("Calibri"));
    }
}
