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
use js_sys::{Array, Function, JsString};
use nitro_xml::opc::fonts::{
    resolve_webfont_plan, FontSource, FONT_SUBSTITUTION_CODE,
};
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
    font_source: FontSource,
    /// Optional host resolver: `(families: string[]) => string | null`.
    /// When set, overrides the built CSS2 href (mirror / default still apply
    /// only if the resolver returns undefined).
    font_source_resolver: Option<Function>,
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
            font_source: FontSource::default(),
            font_source_resolver: None,
            strings: StringTable::new(),
            commands: CommandBuffer::new(),
            measures: MeasureBatch::new(),
            events: EventQueue::new(),
            #[cfg(feature = "bench")]
            grid: crate::bench::SyntheticGrid::new(),
        }
    }

    /// Reset to upstream Google Fonts CSS2 base (`fonts.googleapis.com`).
    #[wasm_bindgen(js_name = setFontSourceDefault)]
    pub fn set_font_source_default(&mut self) {
        self.font_source = FontSource::google_fonts();
        self.font_source_resolver = None;
    }

    /// Point remote webfont CSS2 fetches at an internal mirror (or any
    /// CSS2-compatible base URL). Clears any resolver callback.
    #[wasm_bindgen(js_name = setFontSourceBaseUrl)]
    pub fn set_font_source_base_url(&mut self, base_url: String) {
        self.font_source = FontSource::mirror(base_url);
        self.font_source_resolver = None;
    }

    /// Disable remote webfont fetching; painters use local/embedded fonts and
    /// record substitutions via [`Self::plan_webfont_stylesheet`].
    #[wasm_bindgen(js_name = setFontSourceOff)]
    pub fn set_font_source_off(&mut self) {
        self.font_source = FontSource::off();
        self.font_source_resolver = None;
    }

    /// Host resolver callback `(families: string[]) => href | null`.
    /// The engine still decides when to resolve and records diagnostics; the
    /// callback only maps candidates to a stylesheet URL.
    #[wasm_bindgen(js_name = setFontSourceResolver)]
    pub fn set_font_source_resolver(&mut self, resolve: Function) {
        // Resolver implies remote fetch is allowed; keep default base for any
        // engine-built href passed into future painter paths.
        if !self.font_source.allows_remote_fetch() {
            self.font_source = FontSource::google_fonts();
        }
        self.font_source_resolver = Some(resolve);
    }

    /// Whether remote webfont network requests are permitted for this document.
    #[wasm_bindgen(js_name = fontSourceAllowsRemoteFetch)]
    pub fn font_source_allows_remote_fetch(&self) -> bool {
        self.font_source.allows_remote_fetch() || self.font_source_resolver.is_some()
    }

    /// CSS2 base URL when using default/mirror mode (`undefined` when off).
    #[wasm_bindgen(js_name = fontSourceBaseUrl)]
    pub fn font_source_base_url(&self) -> Option<String> {
        self.font_source.base_url().map(str::to_owned)
    }

    /// Plan a webfont stylesheet for already-matched catalogue fragments.
    ///
    /// Returns the href the host may inject as `<link rel=stylesheet>`, or
    /// `undefined` when nothing should be fetched. With fetch off, pushes
    /// `font_substitution` warnings for each requested family and returns
    /// `undefined` (no network request).
    ///
    /// `catalogue_fragments` / `requested_families` are JS string arrays.
    #[wasm_bindgen(js_name = planWebfontStylesheet)]
    pub fn plan_webfont_stylesheet(
        &mut self,
        catalogue_fragments: Array,
        requested_families: Array,
    ) -> Option<String> {
        let fragments = array_to_strings(&catalogue_fragments);
        let requested = array_to_strings(&requested_families);

        if let Some(resolve) = self.font_source_resolver.clone() {
            // Host resolver owns the final URL; still refuse to call it when
            // the configured source is Off (resolver cleared by setFontSourceOff).
            if !self.font_source.allows_remote_fetch() {
                self.record_font_substitutions(&requested);
                return None;
            }
            let families_js = Array::new();
            for f in &requested {
                families_js.push(&JsValue::from_str(f));
            }
            match resolve.call1(&JsValue::NULL, &families_js) {
                Ok(v) if v.is_null() || v.is_undefined() => {
                    self.record_font_substitutions(&requested);
                    None
                }
                Ok(v) => v.as_string().or_else(|| {
                    // JsString / other stringy values
                    let s: String = JsString::from(v).into();
                    if s.is_empty() || s == "undefined" || s == "null" {
                        self.record_font_substitutions(&requested);
                        None
                    } else {
                        Some(s)
                    }
                }),
                Err(_) => {
                    self.record_font_substitutions(&requested);
                    None
                }
            }
        } else {
            let (href, subs) =
                resolve_webfont_plan(&self.font_source, &fragments, &requested);
            for sub in subs {
                self.warnings.push(Warning::new(
                    FONT_SUBSTITUTION_CODE.to_owned(),
                    sub.message(),
                    None,
                ));
            }
            href
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

impl Handle {
    fn record_font_substitutions(&mut self, requested: &[String]) {
        for family in requested {
            let sub = nitro_xml::opc::fonts::FontSubstitution::local(family.clone());
            self.warnings.push(Warning::new(
                FONT_SUBSTITUTION_CODE.to_owned(),
                sub.message(),
                None,
            ));
        }
    }
}

fn array_to_strings(arr: &Array) -> Vec<String> {
    let mut out = Vec::with_capacity(arr.length() as usize);
    for i in 0..arr.length() {
        let v = arr.get(i);
        if let Some(s) = v.as_string() {
            out.push(s);
        } else if !v.is_null() && !v.is_undefined() {
            out.push(JsString::from(v).into());
        }
    }
    out
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
