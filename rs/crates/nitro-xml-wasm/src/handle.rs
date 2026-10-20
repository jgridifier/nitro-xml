// SPDX-License-Identifier: MIT
//! Document handle conventions for the JS/WASM boundary (#14).
//!
//! Handles own the model in linear memory (`open → handle`, queries, commands,
//! `save`, `free`). The loader (#37) must also register each handle with a
//! `FinalizationRegistry` that calls `free()` if the embedder drops the last JS
//! reference without an explicit free. Crash recovery reloads into a fresh
//! worker (PRODUCT_SPEC.md §5(e)).

use crate::chrome::{EditorChrome, KeyEventLike};
use crate::docx::{is_docx_extension, DocxState};
use crate::events::{self, EventQueue};
use crate::pptx::{is_pptx_extension, PptxState};
use crate::shell::{
    apply_action, cap_chars, open_diagnostic_code, FindMatch, FormatKind, PendingHyperlink,
    ShellAction, ShellState, ShellUi,
};
use crate::transport::StringTable;
use crate::xlsx::{is_xlsx_extension, search_options_from_json, XlsxState};
use crate::xlsx_edit::{editor_key_json, EditorKeyEffect};

fn process_frame(handle: &mut Handle) {
    if handle.xlsx.is_some() {
        crate::xlsx::process_frame(handle);
    } else if handle.pptx.is_some() {
        crate::pptx::process_frame(handle);
        if let Some(pptx) = handle.pptx.as_ref() {
            let idx = pptx.active as u32;
            if handle.shell.nav_index != idx {
                handle.shell.nav_index = idx;
                handle.shell.dirty_ui = true;
            }
        }
    } else if handle.docx.is_some() {
        crate::docx::process_frame(handle);
        if let Some(docx) = handle.docx.as_ref() {
            let idx = docx.active as u32;
            if handle.shell.nav_index != idx {
                handle.shell.nav_index = idx;
                handle.shell.dirty_ui = true;
            }
        }
    }
}
use js_sys::{Array, Function, JsString};
use nitro_xml::opc::fonts::{resolve_webfont_plan, FontSource, FONT_SUBSTITUTION_CODE};
use nitro_xml_render::{
    CommandBuffer, LayoutReadBatch, LayoutReadKind, MeasureBatch, OutlineBatch,
};
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

/// Map package-open `Display` text (from `OpcError` / `ZipError` via `to_string()`) to a stable
/// machine code. Size-limit kinds stay distinct so the loader can show a graceful memory message
/// instead of a generic `open_failed` / crash.
pub(crate) fn classify_open_error(message: &str) -> &'static str {
    let m = message.to_ascii_lowercase();
    if m.contains("uncompressed content limit") {
        "total_too_large"
    } else if m.contains("compressed input limit") {
        "input_too_large"
    } else if m.contains("part limit") {
        "too_many_parts"
    } else if m.contains("uncompressed part size limit") {
        "part_too_large"
    } else if m.contains("compression-ratio limit") || m.contains("zip-bomb") {
        "compression_ratio"
    } else if m.contains("does not inflate to its declared size") {
        "size_mismatch"
    } else if m.starts_with("unsupported format") {
        "unsupported_format"
    } else if m.starts_with("missing package part") {
        "missing_part"
    } else if m.starts_with("not a valid zip") {
        "invalid_archive"
    } else {
        "open_failed"
    }
}

fn js_open_err(message: impl Into<String>) -> JsValue {
    let message = message.into();
    let code = classify_open_error(&message);
    JsValue::from(OoxmlError::with_code(code, message))
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
    /// One package buffer shared with xlsx [`FirstView`](nitro_xml_xlsx::FirstView) (Arc).
    pub(crate) bytes: std::sync::Arc<[u8]>,
    warnings: Vec<Warning>,
    font_source: FontSource,
    /// Optional host resolver: `(families: string[]) => string | null`.
    /// When set, overrides the built CSS2 href (mirror / default still apply
    /// only if the resolver returns undefined).
    font_source_resolver: Option<Function>,
    pub(crate) strings: StringTable,
    pub(crate) commands: CommandBuffer,
    pub(crate) measures: MeasureBatch,
    pub(crate) outlines: OutlineBatch,
    pub(crate) layout_reads: LayoutReadBatch,
    pub(crate) events: EventQueue,
    pub(crate) xlsx: Option<XlsxState>,
    pub(crate) pptx: Option<PptxState>,
    pub(crate) docx: Option<DocxState>,
    pub(crate) shell: ShellState,
    pub(crate) editor: EditorChrome,
    #[cfg(feature = "bench")]
    pub(crate) grid: crate::bench::SyntheticGrid,
}

#[wasm_bindgen]
impl Handle {
    /// Takes ownership of the package bytes.
    ///
    /// wasm-bindgen copies the host `Uint8Array` into linear memory once as a
    /// `Vec<u8>`; we then move that into an `Arc` shared with xlsx [`FirstView`].
    /// Prefer this over `&[u8]`, which left a second pass-buffer copy alive for
    /// the life of the allocation (double residency in WASM linear memory).
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: Vec<u8>) -> Handle {
        let mut strings = StringTable::new();
        crate::events::seed_input_keys(&mut strings);
        Handle {
            bytes: std::sync::Arc::<[u8]>::from(bytes),
            warnings: Vec::new(),
            font_source: FontSource::default(),
            font_source_resolver: None,
            strings,
            commands: CommandBuffer::new(),
            measures: MeasureBatch::new(),
            outlines: OutlineBatch::new(),
            layout_reads: LayoutReadBatch::new(),
            events: EventQueue::new(),
            xlsx: None,
            pptx: None,
            docx: None,
            shell: ShellState::default(),
            editor: EditorChrome::new(),
            #[cfg(feature = "bench")]
            grid: crate::bench::SyntheticGrid::new(),
        }
    }

    /// Read-only mode hides editor ribbon and disables editing commands (#53).
    #[wasm_bindgen(js_name = setReadOnly)]
    pub fn set_read_only(&mut self, read_only: bool) {
        self.editor.set_read_only(read_only);
        if !read_only {
            if let Some(xlsx) = self.xlsx.as_mut() {
                let _ = xlsx.ensure_edit(&self.bytes);
            } else if let Some(pptx) = self.pptx.as_mut() {
                let _ = pptx.ensure_edit(&self.bytes);
            } else if let Some(docx) = self.docx.as_mut() {
                let _ = docx.ensure_edit(&self.bytes);
            }
        }
        self.refresh_formula_bar();
        self.shell.dirty_ui = true;
    }

    /// Lazy-load editor chrome: register command stubs and enable ribbon HTML.
    #[wasm_bindgen(js_name = loadEditorChrome)]
    pub fn load_editor_chrome(&mut self) {
        self.editor.load();
        self.shell.dirty_ui = true;
    }

    /// Match a keyboard event against the shortcut table; returns JSON `{command?, action?}` or `{}`.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = matchShortcut)]
    pub fn match_shortcut(
        &self,
        key: &str,
        code: &str,
        ctrl: bool,
        meta: bool,
        alt: bool,
        shift: bool,
        mac: bool,
    ) -> String {
        let ev = KeyEventLike {
            key,
            code: if code.is_empty() { None } else { Some(code) },
            ctrl_key: ctrl,
            meta_key: meta,
            alt_key: alt,
            shift_key: shift,
        };
        match self.editor.match_shortcut(&ev, mac) {
            Some(m) => {
                let mut out = serde_json::Map::new();
                if let Some(c) = m.command {
                    out.insert("command".into(), serde_json::Value::String(c.to_string()));
                }
                if let Some(a) = m.action {
                    out.insert("action".into(), serde_json::Value::String(a.to_string()));
                }
                serde_json::Value::Object(out).to_string()
            }
            None => "{}".into(),
        }
    }

    /// Open a supported OOXML document (extension hint). Xlsx viewer path loads workbook + first paint.
    #[wasm_bindgen(js_name = openDocument)]
    pub fn open_document(&mut self, name: &str) -> Result<(), JsValue> {
        if is_xlsx_extension(name) {
            let t0 = now_ms();
            let (mut state, warnings) =
                XlsxState::open(std::sync::Arc::clone(&self.bytes)).map_err(js_open_err)?;
            self.shell = ShellState::default();
            self.shell.format = FormatKind::Xlsx;
            self.shell.diagnostics.clear_runtime();
            for w in &warnings {
                let code = open_diagnostic_code(w, "xlsx_warning");
                self.shell.diagnostics.ingest_warning(code, w, None);
                self.push_warning(code.into(), w.clone(), None);
            }
            self.shell.set_tabs(
                state
                    .sheets_json()
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| {
                                let index = s.get("index")?.as_u64()? as u32;
                                let name = s.get("name")?.as_str()?.to_owned();
                                Some((index, name))
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                state.active as u32,
            );
            // Seed shell zoom from sheet zoomScale (upstream grid-view.ts).
            self.shell.set_zoom(state.active_view_zoom());
            state.set_zoom_pct(self.shell.zoom_pct);
            if let Some(r) = state.selection_ref_now() {
                self.shell.selection_ref = r;
            }
            self.xlsx = Some(state);
            self.pptx = None;
            self.docx = None;
            self.shell.diagnostics.parse_ms = now_ms() - t0;
            let t1 = now_ms();
            process_frame(self);
            self.shell.diagnostics.paint_ms = now_ms() - t1;
            self.shell.dirty_ui = true;
            return Ok(());
        }
        if is_pptx_extension(name) {
            let t0 = now_ms();
            let mut state =
                PptxState::open(std::sync::Arc::clone(&self.bytes)).map_err(js_open_err)?;
            self.shell = ShellState::default();
            self.shell.format = FormatKind::Pptx;
            self.shell.diagnostics.clear_runtime();
            let n = state.pres.slide_count();
            self.shell.set_tabs(
                (0..n)
                    .map(|i| (i as u32, format!("Slide {}", i + 1)))
                    .collect(),
                state.active as u32,
            );
            match state.unsupported_labels() {
                Ok(kinds) => {
                    for kind in kinds {
                        self.shell
                            .diagnostics
                            .ingest_warning("unsupported_content", &kind, None);
                        self.push_warning("unsupported_content".into(), kind, None);
                    }
                }
                Err(e) => {
                    self.shell
                        .diagnostics
                        .ingest_warning("pptx_warning", &e, None);
                    self.push_warning("pptx_warning".into(), e, None);
                }
            }
            self.pptx = Some(state);
            self.xlsx = None;
            self.docx = None;
            self.shell.diagnostics.parse_ms = now_ms() - t0;
            let t1 = now_ms();
            process_frame(self);
            self.shell.diagnostics.paint_ms = now_ms() - t1;
            self.shell.dirty_ui = true;
            return Ok(());
        }
        if is_docx_extension(name) {
            let t0 = now_ms();
            let state = DocxState::open(&self.bytes).map_err(js_open_err)?;
            self.shell = ShellState::default();
            self.shell.format = FormatKind::Docx;
            self.shell.diagnostics.clear_runtime();
            for w in &state.session.warnings {
                let code = open_diagnostic_code(w, "docx_warning");
                self.shell.diagnostics.ingest_warning(code, w, None);
                self.push_warning(code.into(), w.clone(), None);
            }
            self.shell.set_tabs(state.page_tabs(), state.active as u32);
            self.docx = Some(state);
            self.xlsx = None;
            self.pptx = None;
            self.shell.diagnostics.parse_ms = now_ms() - t0;
            let t1 = now_ms();
            process_frame(self);
            self.shell.diagnostics.paint_ms = now_ms() - t1;
            self.shell.dirty_ui = true;
            return Ok(());
        }
        Err(JsValue::from(OoxmlError::with_code(
            "unsupported_format",
            format!("viewer open not implemented for {}", name),
        )))
    }

    /// Engine tick: apply input, repaint into the pending command buffer.
    #[wasm_bindgen(js_name = prepareFrame)]
    pub fn prepare_frame(&mut self) {
        process_frame(self);
    }

    /// True when the active xlsx sheet's post-`sheetData` shell (merges/CF/DV) is loaded.
    /// Visual parity waits on this after the first viewport paint (deferred SeekTail).
    /// Non-xlsx documents are always complete. Short `t` keeps loader brotli under budget.
    #[wasm_bindgen(getter, js_name = t)]
    pub fn sheet_tail_complete(&self) -> bool {
        match &self.xlsx {
            Some(xlsx) => xlsx.sheet_tail_complete(),
            None => true,
        }
    }

    /// Document format after open (`xlsx`, …) or empty when not opened.
    #[wasm_bindgen(js_name = documentFormat)]
    pub fn document_format(&self) -> String {
        if self.xlsx.is_some() {
            return "xlsx".into();
        }
        if self.pptx.is_some() {
            return "pptx".into();
        }
        if self.docx.is_some() {
            return "docx".into();
        }
        String::new()
    }

    /// Visible sheet tabs (JSON array).
    #[wasm_bindgen(js_name = sheets)]
    pub fn sheets(&self) -> Result<String, JsValue> {
        let xlsx = self
            .xlsx
            .as_ref()
            .ok_or_else(|| JsValue::from(OoxmlError::with_code("no_document", "not opened")))?;
        Ok(nitro_xml_xlsx::stringify_json(&xlsx.sheets_json()))
    }

    #[wasm_bindgen(js_name = setActiveSheet)]
    pub fn set_active_sheet(&mut self, index: u32) -> Result<(), JsValue> {
        if let Some(mut xlsx) = self.xlsx.take() {
            xlsx.set_active_sheet(&self.bytes, index as usize)
                .map_err(|e| JsValue::from(OoxmlError::with_code("bad_sheet", e)))?;
            self.xlsx = Some(xlsx);
            process_frame(self);
            return Ok(());
        }
        if let Some(mut docx) = self.docx.take() {
            docx.set_active_page(index as usize)
                .map_err(|e| JsValue::from(OoxmlError::with_code("bad_page", e)))?;
            self.docx = Some(docx);
            process_frame(self);
            return Ok(());
        }
        let mut pptx = self
            .pptx
            .take()
            .ok_or_else(|| JsValue::from(OoxmlError::with_code("no_document", "not opened")))?;
        pptx.set_active_slide(index as usize)
            .map_err(|e| JsValue::from(OoxmlError::with_code("bad_slide", e)))?;
        self.pptx = Some(pptx);
        process_frame(self);
        Ok(())
    }

    /// Slide list + active index for pptx (JSON).
    #[wasm_bindgen(js_name = slides)]
    pub fn slides(&self) -> Result<String, JsValue> {
        let pptx = self
            .pptx
            .as_ref()
            .ok_or_else(|| JsValue::from(OoxmlError::with_code("no_document", "not opened")))?;
        Ok(pptx.slides_json().to_string())
    }

    #[wasm_bindgen(js_name = cellText)]
    pub fn cell_text(&mut self, sheet_index: u32, range: &str) -> Result<String, JsValue> {
        let xlsx = self
            .xlsx
            .as_mut()
            .ok_or_else(|| JsValue::from(OoxmlError::with_code("no_document", "not opened")))?;
        xlsx.cell_text(&self.bytes, sheet_index as usize, range)
            .map_err(|e| JsValue::from(OoxmlError::with_code("bad_range", e)))
    }

    #[wasm_bindgen(js_name = hyperlinks)]
    pub fn hyperlinks(&mut self, sheet_index: u32) -> Result<String, JsValue> {
        if let Some(xlsx) = self.xlsx.as_mut() {
            let v = xlsx
                .hyperlinks(&self.bytes, sheet_index as usize)
                .map_err(|e| JsValue::from(OoxmlError::with_code("bad_sheet", e)))?;
            return Ok(nitro_xml_xlsx::stringify_json(&v));
        }
        if let Some(docx) = self.docx.as_ref() {
            let _ = sheet_index;
            return Ok(serde_json::Value::Array(docx.hyperlinks_json()).to_string());
        }
        if self.pptx.is_some() {
            // TODO(#27): a:hlinkClick is not stored on the slide model.
            let _ = sheet_index;
            return Ok("[]".into());
        }
        Err(JsValue::from(OoxmlError::with_code(
            "no_document",
            "not opened",
        )))
    }

    #[wasm_bindgen(js_name = hyperlinkAt)]
    pub fn hyperlink_at(
        &mut self,
        sheet_index: u32,
        row: u32,
        col: u32,
    ) -> Result<JsValue, JsValue> {
        let xlsx = self
            .xlsx
            .as_mut()
            .ok_or_else(|| JsValue::from(OoxmlError::with_code("no_document", "not opened")))?;
        match xlsx.hyperlink_at(&self.bytes, sheet_index as usize, row, col) {
            Ok(Some(v)) => Ok(JsValue::from_str(&nitro_xml_xlsx::stringify_json(&v))),
            Ok(None) => Ok(JsValue::NULL),
            Err(e) => Err(JsValue::from(OoxmlError::with_code("bad_sheet", e))),
        }
    }

    #[wasm_bindgen(js_name = search)]
    pub fn search(&mut self, query: &str, opts_json: &str) -> Result<String, JsValue> {
        let xlsx = self
            .xlsx
            .as_mut()
            .ok_or_else(|| JsValue::from(OoxmlError::with_code("no_document", "not opened")))?;
        let opts = search_options_from_json(opts_json);
        let v = xlsx
            .search(&self.bytes, query, opts)
            .map_err(|e| JsValue::from(OoxmlError::with_code("search_failed", e)))?;
        Ok(nitro_xml_xlsx::stringify_json(&v))
    }

    /// Apply a shell action (`zoom-in`, `find-query`, `sheet`, `copy`, `ui`, …).
    /// Returns JSON: `{ h?, c?, p?, copyText?, pendingHyperlink?, ui }`.
    /// Short keys (`h` chromeHtml, `c` copyText, `p` pendingHyperlink) keep the
    /// loader apply path tiny; long keys remain for harnesses. Op `ui` is snapshot-only.
    #[wasm_bindgen(js_name = shellAction)]
    pub fn shell_action(&mut self, op: &str, arg: &str) -> Result<String, JsValue> {
        if matches!(
            op,
            "formula-cancel"
                | "formula-commit"
                | "formula-fx"
                | "formula-expand"
                | "formula-focus"
                | "save"
        ) {
            return self.editor_chrome_op(op);
        }
        if op == "cmd" {
            return self.dispatch_chrome_command(arg);
        }
        if op == "ui" || op.is_empty() {
            self.refresh_formula_bar();
            let ui = ShellUi::to_json(&self.shell, &self.editor);
            let h = ui
                .get("chromeHtml")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let out = serde_json::json!({ "h": h, "ui": ui });
            return Ok(out.to_string());
        }
        let action = ShellAction::parse(op, arg).ok_or_else(|| {
            JsValue::from(OoxmlError::with_code(
                "bad_shell_action",
                format!("unknown shell action {op}"),
            ))
        })?;
        let prev_selection = self.shell.selection_ref.clone();
        let mut result = apply_action(&mut self.shell, &action);
        match &action {
            ShellAction::FindQuery(_) | ShellAction::FindOpts { .. } => {
                self.run_find()?;
                result.reopen_paint = false;
            }
            ShellAction::Copy => {
                let text = if self.xlsx.is_some() {
                    self.copy_selection_tsv()?
                } else if self.pptx.is_some() {
                    self.copy_pptx_selection()?
                } else if self.docx.is_some() {
                    self.copy_docx_plain()
                } else {
                    String::new()
                };
                self.shell.last_copy = text.clone();
                result.copy_text = Some(text);
            }
            ShellAction::SelectRef(r) => {
                if let Some(xlsx) = self.xlsx.as_mut() {
                    match xlsx.apply_selection_ref(r) {
                        Ok(normalized) => {
                            self.shell.selection_ref = normalized;
                            result.reopen_paint = true;
                        }
                        Err(e) => {
                            return Err(JsValue::from(OoxmlError::with_code("bad_selection", e)));
                        }
                    }
                } else if let Some(pptx) = self.pptx.as_mut() {
                    if looks_like_element_id(r) && pptx.select_element_id(r) {
                        self.shell.selection_ref = r.clone();
                        result.reopen_paint = true;
                    } else {
                        self.shell.selection_ref = prev_selection;
                    }
                } else if let Some(docx) = self.docx.as_ref() {
                    if docx.has_block_id(r) {
                        self.shell.selection_ref = r.clone();
                    } else {
                        self.shell.selection_ref = prev_selection;
                    }
                }
            }
            ShellAction::HyperlinkAt { sheet, row, col } => {
                if self.xlsx.is_some() {
                    self.activate_hyperlink(*sheet, *row, *col)?;
                } else if self.docx.is_some() {
                    self.activate_docx_hyperlink(*row, *col)?;
                } else if self.pptx.is_some() {
                    // TODO(#27): slide elements do not expose a:hlinkClick targets.
                    let _ = (sheet, row, col);
                    self.shell.pending_hyperlink = None;
                }
            }
            ShellAction::Sheet(_)
            | ShellAction::NavNext
            | ShellAction::NavPrev
            | ShellAction::ZoomIn
            | ShellAction::ZoomOut
            | ShellAction::ZoomSet(_) => {}
            _ => {}
        }
        if let Some(idx) = result.navigate_sheet {
            if let Some(xlsx) = self.xlsx.as_mut() {
                xlsx.set_active_sheet(&self.bytes, idx as usize)
                    .map_err(|e| JsValue::from(OoxmlError::with_code("bad_sheet", e)))?;
                self.shell.nav_index = idx;
            }
            if let Some(pptx) = self.pptx.as_mut() {
                pptx.set_active_slide(idx as usize)
                    .map_err(|e| JsValue::from(OoxmlError::with_code("bad_slide", e)))?;
                self.shell.nav_index = idx;
            }
            if let Some(docx) = self.docx.as_mut() {
                docx.set_active_page(idx as usize)
                    .map_err(|e| JsValue::from(OoxmlError::with_code("bad_page", e)))?;
                self.shell.nav_index = idx;
            }
        }
        if let Some((row, col)) = result.scroll_to {
            if let Some(xlsx) = self.xlsx.as_mut() {
                xlsx.scroll_to_cell(row, col);
            }
            result.reopen_paint = true;
        }
        if matches!(action, ShellAction::Sheet(_)) {
            if let Some(xlsx) = self.xlsx.as_ref() {
                self.shell.set_zoom(xlsx.active_view_zoom());
            }
        }
        if let Some(xlsx) = self.xlsx.as_mut() {
            xlsx.set_zoom_pct(self.shell.zoom_pct);
        }
        if result.reopen_paint
            || matches!(
                action,
                ShellAction::ZoomIn
                    | ShellAction::ZoomOut
                    | ShellAction::ZoomSet(_)
                    | ShellAction::Sheet(_)
                    | ShellAction::NavNext
                    | ShellAction::NavPrev
                    | ShellAction::FindNext
                    | ShellAction::FindPrev
            )
        {
            let t1 = now_ms();
            process_frame(self);
            self.shell.diagnostics.paint_ms = now_ms() - t1;
        }
        let pending = self.shell.take_pending_hyperlink().map(|h| {
            let mut m = serde_json::Map::new();
            m.insert("open".into(), serde_json::json!(h.open));
            if let Some(t) = h.target {
                m.insert("target".into(), serde_json::Value::String(t));
            }
            if let Some(l) = h.location {
                m.insert("location".into(), serde_json::Value::String(l));
            }
            if let Some(tip) = h.tooltip {
                m.insert("tooltip".into(), serde_json::Value::String(tip));
            }
            serde_json::Value::Object(m)
        });
        self.refresh_formula_bar();
        let ui = ShellUi::to_json(&self.shell, &self.editor);
        let h = ui
            .get("chromeHtml")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut out = serde_json::Map::new();
        out.insert("h".into(), serde_json::Value::String(h));
        out.insert("c".into(), serde_json::json!(result.copy_text));
        out.insert("copyText".into(), serde_json::json!(result.copy_text));
        out.insert(
            "p".into(),
            pending.clone().unwrap_or(serde_json::Value::Null),
        );
        out.insert(
            "pendingHyperlink".into(),
            pending.unwrap_or(serde_json::Value::Null),
        );
        out.insert("ui".into(), ui);
        Ok(serde_json::Value::Object(out).to_string())
    }

    /// Copy interned strings for the loader (`resolve` on the main thread / worker).
    #[wasm_bindgen(js_name = exportStrings)]
    pub fn export_strings(&self) -> Array {
        let out = Array::new();
        let n = self.strings.len();
        for i in 0..n {
            out.push(&JsValue::from(self.strings.resolve(i).unwrap_or("")));
        }
        out
    }

    /// Apply host `fontSource` in one call (loader API: omitted / string / false / `"off"` / function).
    ///
    /// - `undefined` → upstream Google Fonts CSS2 base
    /// - `false`, `"off"`, or `""` → remote fetch off
    /// - non-empty string → CSS2-compatible base URL (mirror)
    /// - function `(families: string[]) => href | null` → host resolver
    #[wasm_bindgen(js_name = setFontSource)]
    pub fn set_font_source(&mut self, value: JsValue) -> Result<(), JsValue> {
        if value.is_undefined() {
            self.set_font_source_default();
            return Ok(());
        }
        if value.is_null() {
            return Err(JsValue::from_str(
                "fontSource must be a CSS2 base URL string, false/\"off\", a resolver function, or omitted",
            ));
        }
        if let Some(b) = value.as_bool() {
            if b {
                return Err(JsValue::from_str(
                    "fontSource must be a CSS2 base URL string, false/\"off\", a resolver function, or omitted",
                ));
            }
            self.set_font_source_off();
            return Ok(());
        }
        if let Some(s) = value.as_string() {
            if s.is_empty() || s == "off" {
                self.set_font_source_off();
            } else {
                self.set_font_source_base_url(s);
            }
            return Ok(());
        }
        if value.is_function() {
            self.set_font_source_resolver(Function::from(value));
            return Ok(());
        }
        Err(JsValue::from_str(
            "fontSource must be a CSS2 base URL string, false/\"off\", a resolver function, or omitted",
        ))
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
            let (href, subs) = resolve_webfont_plan(&self.font_source, &fragments, &requested);
            for sub in subs {
                let message = sub.message();
                self.shell
                    .diagnostics
                    .ingest_warning(FONT_SUBSTITUTION_CODE, &message, None);
                self.shell.dirty_ui = true;
                self.warnings.push(Warning::new(
                    FONT_SUBSTITUTION_CODE.to_owned(),
                    message,
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

    /// Number of pending (not yet taken) measure requests.
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

    /// Take pending measure requests for the loader (clears the queue).
    ///
    /// Returns a JS array `[Uint32Array fonts, Float32Array sizes, Uint32Array texts]`.
    #[wasm_bindgen(js_name = takeMeasureReqs)]
    pub fn take_measure_reqs(&mut self) -> Array {
        let (fonts, sizes, texts) = self.measures.take_requests();
        let out = Array::new();
        out.push(&js_sys::Uint32Array::from(fonts.as_slice()).into());
        out.push(&js_sys::Float32Array::from(sizes.as_slice()).into());
        out.push(&js_sys::Uint32Array::from(texts.as_slice()).into());
        out
    }

    /// Loader writes measured widths back (one `f64` per taken/awaiting request).
    #[wasm_bindgen(js_name = setMeasureResults)]
    pub fn set_measure_results(&mut self, widths: &[f64]) -> Result<(), JsValue> {
        self.measures.set_results(widths).map_err(|e| {
            JsValue::from(OoxmlError::with_code("measure_len_mismatch", e.to_string()))
        })
    }

    /// Pending outline probe count (WordArt). Kept so the host can dynamic-import
    /// the outline chunk only when `outlineCount > 0` (#27).
    #[wasm_bindgen(getter, js_name = outlineCount)]
    pub fn outline_count(&self) -> usize {
        self.outlines.len()
    }

    /// Take pending outline requests (clears the queue).
    /// Returns `[Uint32Array fonts, Uint32Array chars, Float32Array sizes]`.
    #[wasm_bindgen(js_name = takeOutlineReqs)]
    pub fn take_outline_reqs(&mut self) -> Array {
        let (fonts, chars, sizes) = self.outlines.take_requests();
        let out = Array::new();
        out.push(&js_sys::Uint32Array::from(fonts.as_slice()).into());
        out.push(&js_sys::Uint32Array::from(chars.as_slice()).into());
        out.push(&js_sys::Float32Array::from(sizes.as_slice()).into());
        out
    }

    /// Packed contour results from `loader/lazy/glyph-outline.js` (no Reflect in glue).
    /// Per request: `ring_count`, then per ring `point_count` + `(x,y)` pairs; `0` = none.
    #[wasm_bindgen(js_name = setOutlineResults)]
    pub fn set_outline_results(&mut self, packed: &[f64]) -> Result<(), JsValue> {
        use nitro_xml_render::GlyphContours;

        let awaiting = self.outlines.awaiting();
        let mut parsed: Vec<Option<GlyphContours>> = Vec::with_capacity(awaiting);
        let mut i = 0usize;
        for _ in 0..awaiting {
            if i >= packed.len() {
                return Err(JsValue::from(OoxmlError::with_code(
                    "outline_len_mismatch",
                    "packed outline buffer truncated",
                )));
            }
            let ring_count = packed[i] as usize;
            i += 1;
            if ring_count == 0 {
                parsed.push(None);
                continue;
            }
            let mut contours: GlyphContours = Vec::with_capacity(ring_count);
            for _ in 0..ring_count {
                if i >= packed.len() {
                    return Err(JsValue::from(OoxmlError::with_code(
                        "outline_len_mismatch",
                        "packed outline buffer truncated",
                    )));
                }
                let point_count = packed[i] as usize;
                i += 1;
                let need = point_count.saturating_mul(2);
                if i + need > packed.len() {
                    return Err(JsValue::from(OoxmlError::with_code(
                        "outline_len_mismatch",
                        "packed outline buffer truncated",
                    )));
                }
                let mut ring = Vec::with_capacity(point_count);
                for _ in 0..point_count {
                    ring.push((packed[i], packed[i + 1]));
                    i += 2;
                }
                if !ring.is_empty() {
                    contours.push(ring);
                }
            }
            parsed.push(if contours.is_empty() {
                None
            } else {
                Some(contours)
            });
        }
        self.outlines
            .set_results(parsed)
            .map_err(|e| JsValue::from(OoxmlError::with_code("outline_len_mismatch", e)))
    }

    /// Queue a post-apply layout read-back (answered on the next frame after apply).
    ///
    /// `kind` is [`LayoutReadKind`] as `u8`. `prop_id` is a string-table id used
    /// only for [`LayoutReadKind::ComputedStyle`]; pass `0` for other kinds.
    #[wasm_bindgen(js_name = pushLayoutReadReq)]
    pub fn push_layout_read_req(
        &mut self,
        node_id: u32,
        kind: u8,
        prop_id: u32,
    ) -> Result<(), JsValue> {
        let kind = LayoutReadKind::from_u8(kind).ok_or_else(|| {
            JsValue::from(OoxmlError::with_code(
                "layout_read_kind",
                format!("unknown layout read kind {kind}"),
            ))
        })?;
        self.layout_reads.push(node_id, kind, prop_id);
        Ok(())
    }

    /// Pending layout requests not yet taken for a frame.
    #[wasm_bindgen(getter, js_name = layoutCount)]
    pub fn layout_count(&self) -> usize {
        self.layout_reads.len()
    }

    #[wasm_bindgen(js_name = layoutNodePtr)]
    pub fn layout_node_ptr(&self) -> *const u32 {
        self.layout_reads.node_id.as_ptr()
    }

    #[wasm_bindgen(js_name = layoutKindPtr)]
    pub fn layout_kind_ptr(&self) -> *const u8 {
        self.layout_reads.kind.as_ptr()
    }

    #[wasm_bindgen(js_name = layoutPropPtr)]
    pub fn layout_prop_ptr(&self) -> *const u32 {
        self.layout_reads.prop_id.as_ptr()
    }

    /// Take pending layout requests for post-apply read-back (clears the queue).
    ///
    /// Returns a JS array `[Uint32Array nodes, Uint8Array kinds, Uint32Array props]`.
    #[wasm_bindgen(js_name = takeLayoutReqs)]
    pub fn take_layout_reqs(&mut self) -> Array {
        let (nodes, kinds, props) = self.layout_reads.take_requests();
        let out = Array::new();
        out.push(&js_sys::Uint32Array::from(nodes.as_slice()).into());
        out.push(&js_sys::Uint8Array::from(kinds.as_slice()).into());
        out.push(&js_sys::Uint32Array::from(props.as_slice()).into());
        out
    }

    /// Loader writes packed layout results (`awaiting * 4` f64s) in one call.
    #[wasm_bindgen(js_name = setLayoutResults)]
    pub fn set_layout_results(&mut self, values: &[f64]) -> Result<(), JsValue> {
        self.layout_reads.set_results(values).map_err(|e| {
            JsValue::from(OoxmlError::with_code(
                "layout_read_len_mismatch",
                e.to_string(),
            ))
        })
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

    /// Chrome HTML when the shell UI changed (formula bar follows the selection). `None` if clean.
    #[wasm_bindgen(js_name = takeChromeIfDirty)]
    pub fn take_chrome_if_dirty(&mut self) -> Option<String> {
        if !self.shell.dirty_ui {
            return None;
        }
        self.shell.dirty_ui = false;
        if self.editor.read_only() || !self.editor.loaded() {
            return None;
        }
        self.refresh_formula_bar();
        let ui = ShellUi::to_json(&self.shell, &self.editor);
        ui.get("chromeHtml")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
    }

    /// Drain pending events (engine tick). Returns how many were dropped.
    #[wasm_bindgen(js_name = drainEvents)]
    pub fn drain_events(&mut self) -> u32 {
        self.events.drain() as u32
    }

    /// Save the open document. Unedited documents return the original bytes (#68).
    #[wasm_bindgen(js_name = saveDocument)]
    pub fn save_document(&mut self) -> Result<Vec<u8>, JsValue> {
        if self.xlsx.is_some() {
            let xlsx = self
                .xlsx
                .as_mut()
                .ok_or_else(|| JsValue::from(OoxmlError::with_code("no_document", "not opened")))?;
            if !self.editor.read_only() {
                let _ = xlsx.ensure_edit(&self.bytes);
            }
            return xlsx
                .save_bytes(&self.bytes)
                .map_err(|e| JsValue::from(OoxmlError::with_code("save_failed", e)));
        }
        if self.pptx.is_some() {
            let pptx = self
                .pptx
                .as_mut()
                .ok_or_else(|| JsValue::from(OoxmlError::with_code("no_document", "not opened")))?;
            if !self.editor.read_only() {
                let _ = pptx.ensure_edit(&self.bytes);
            }
            return pptx
                .save_bytes(&self.bytes)
                .map_err(|e| JsValue::from(OoxmlError::with_code("save_failed", e)));
        }
        if self.docx.is_some() {
            let docx = self
                .docx
                .as_mut()
                .ok_or_else(|| JsValue::from(OoxmlError::with_code("no_document", "not opened")))?;
            if !self.editor.read_only() {
                let _ = docx.ensure_edit(&self.bytes);
            }
            return docx
                .save_bytes(&self.bytes)
                .map_err(|e| JsValue::from(OoxmlError::with_code("save_failed", e)));
        }
        Err(JsValue::from(OoxmlError::with_code(
            "no_document",
            "not opened",
        )))
    }

    /// Headless edit: `{"command":"home.bold"}` or `{"command":"setCellInput","sheet":0,...}`.
    #[wasm_bindgen(js_name = applyEdit)]
    pub fn apply_edit(&mut self, spec: &str) -> Result<String, JsValue> {
        if self.editor.read_only() {
            return Err(JsValue::from(OoxmlError::with_code(
                "read_only",
                "editing is disabled",
            )));
        }
        if self.xlsx.is_some() {
            let xlsx = self
                .xlsx
                .as_mut()
                .ok_or_else(|| JsValue::from(OoxmlError::with_code("no_document", "not opened")))?;
            xlsx.ensure_edit(&self.bytes)
                .map_err(|e| JsValue::from(OoxmlError::with_code("open_failed", e)))?;
            let effect = xlsx
                .apply_raw_edit(spec)
                .map_err(|e| JsValue::from(OoxmlError::with_code("edit_failed", e)))?;
            if matches!(
                effect,
                nitro_xml_xlsx::editor::ChromeEffect::Applied { structure: true }
            ) {
                self.refresh_xlsx_tabs();
            }
            process_frame(self);
            self.refresh_formula_bar();
            return Ok(self.chrome_payload(false));
        }
        self.apply_office_edit(spec)
    }

    /// Formula bar / in-cell keys. F2 and typing start an edit; Enter commits; Escape cancels.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = editorKey)]
    pub fn editor_key(
        &mut self,
        key: &str,
        ctrl: bool,
        meta: bool,
        alt: bool,
        shift: bool,
        mac: bool,
    ) -> Result<String, JsValue> {
        if self.editor.read_only() {
            return Ok(editor_key_json(EditorKeyEffect::Ignored, None));
        }
        if self.xlsx.is_none() {
            return self.editor_key_office(key, ctrl, meta, alt);
        }
        let xlsx = self
            .xlsx
            .as_mut()
            .ok_or_else(|| JsValue::from(OoxmlError::with_code("no_document", "not opened")))?;
        xlsx.ensure_edit(&self.bytes)
            .map_err(|e| JsValue::from(OoxmlError::with_code("open_failed", e)))?;
        let effect = xlsx
            .editor_key(key, shift, ctrl, meta, alt, mac)
            .map_err(|e| JsValue::from(OoxmlError::with_code("edit_failed", e)))?;
        if !matches!(effect, EditorKeyEffect::Ignored) {
            if let Some(sel) = self.xlsx.as_ref().and_then(|x| x.selection_ref_now()) {
                self.shell.selection_ref = sel;
            }
            process_frame(self);
            self.refresh_formula_bar();
            self.shell.dirty_ui = true;
        }
        let selection = self.xlsx.as_ref().and_then(|x| x.selection_ref_now());
        let mut payload: serde_json::Value =
            serde_json::from_str(&editor_key_json(effect, selection))
                .unwrap_or(serde_json::json!({}));
        if payload.get("handled").and_then(|v| v.as_bool()) == Some(true) {
            let ui = ShellUi::to_json(&self.shell, &self.editor);
            if let Some(obj) = payload.as_object_mut() {
                if let Some(h) = ui.get("chromeHtml").and_then(|v| v.as_str()) {
                    obj.insert("h".into(), serde_json::Value::String(h.to_owned()));
                }
            }
        }
        Ok(payload.to_string())
    }
}

impl Handle {
    fn refresh_formula_bar(&mut self) {
        if self.editor.read_only() || !self.editor.loaded() {
            self.editor.set_formula_html(String::new());
            self.editor.set_office_ribbon(String::new());
            return;
        }
        let html = self
            .xlsx
            .as_ref()
            .map(|xlsx| xlsx.formula_bar_html(false))
            .unwrap_or_default();
        self.editor.set_formula_html(html);
        self.refresh_office_ribbon();
    }

    /// Pptx/docx command strip. Xlsx leaves it empty so the Home ribbon is unchanged.
    fn refresh_office_ribbon(&mut self) {
        let html = if let Some(pptx) = self.pptx.as_ref() {
            pptx.ribbon_html()
        } else if let Some(docx) = self.docx.as_ref() {
            docx.ribbon_html()
        } else {
            String::new()
        };
        self.editor.set_office_ribbon(html);
    }

    fn sync_office_tabs(&mut self) {
        if let Some(pptx) = self.pptx.as_ref() {
            let n = pptx.pres.slide_count();
            let tabs: Vec<(u32, String)> = (0..n)
                .map(|i| (i as u32, format!("Slide {}", i + 1)))
                .collect();
            let active = pptx.active as u32;
            if self.shell.tabs.len() != n || self.shell.nav_index != active {
                self.shell.set_tabs(tabs, active);
            }
        } else if let Some(docx) = self.docx.as_ref() {
            let tabs = docx.page_tabs();
            let active = docx.active as u32;
            if self.shell.tabs.len() != tabs.len() || self.shell.nav_index != active {
                self.shell.set_tabs(tabs, active);
            }
        }
    }

    fn refresh_office_view(&mut self) -> Result<(), JsValue> {
        if let Some(pptx) = self.pptx.as_mut() {
            pptx.refresh_view(&self.bytes)
                .map_err(|e| JsValue::from(OoxmlError::with_code("save_failed", e)))?;
        } else if let Some(docx) = self.docx.as_mut() {
            docx.refresh_view(&self.bytes)
                .map_err(|e| JsValue::from(OoxmlError::with_code("save_failed", e)))?;
        }
        Ok(())
    }

    fn dispatch_office_command(
        &mut self,
        id: &str,
    ) -> Result<crate::office_edit::OfficeEffect, JsValue> {
        if let Some(pptx) = self.pptx.as_mut() {
            pptx.ensure_edit(&self.bytes)
                .map_err(|e| JsValue::from(OoxmlError::with_code("open_failed", e)))?;
            return pptx
                .dispatch_chrome(id)
                .map_err(|e| JsValue::from(OoxmlError::with_code("edit_failed", e)));
        }
        if let Some(docx) = self.docx.as_mut() {
            docx.ensure_edit(&self.bytes)
                .map_err(|e| JsValue::from(OoxmlError::with_code("open_failed", e)))?;
            return docx
                .dispatch_chrome(id)
                .map_err(|e| JsValue::from(OoxmlError::with_code("edit_failed", e)));
        }
        Err(JsValue::from(OoxmlError::with_code(
            "no_document",
            "not opened",
        )))
    }

    fn editor_key_office(
        &mut self,
        key: &str,
        ctrl: bool,
        meta: bool,
        alt: bool,
    ) -> Result<String, JsValue> {
        let handled = if let Some(pptx) = self.pptx.as_mut() {
            pptx.ensure_edit(&self.bytes)
                .map_err(|e| JsValue::from(OoxmlError::with_code("open_failed", e)))?;
            pptx.type_key(key, ctrl, meta, alt)
                .map_err(|e| JsValue::from(OoxmlError::with_code("edit_failed", e)))?
        } else if let Some(docx) = self.docx.as_mut() {
            docx.ensure_edit(&self.bytes)
                .map_err(|e| JsValue::from(OoxmlError::with_code("open_failed", e)))?;
            docx.type_key(key, ctrl, meta, alt)
                .map_err(|e| JsValue::from(OoxmlError::with_code("edit_failed", e)))?
        } else {
            return Err(JsValue::from(OoxmlError::with_code(
                "no_document",
                "not opened",
            )));
        };
        if !handled {
            return Ok(editor_key_json(EditorKeyEffect::Ignored, None));
        }
        self.refresh_office_view()?;
        self.sync_office_tabs();
        process_frame(self);
        self.refresh_formula_bar();
        self.shell.dirty_ui = true;
        let mut payload: serde_json::Value =
            serde_json::from_str(&editor_key_json(EditorKeyEffect::Handled, None))
                .unwrap_or(serde_json::json!({}));
        let ui = ShellUi::to_json(&self.shell, &self.editor);
        if let Some(obj) = payload.as_object_mut() {
            if let Some(h) = ui.get("chromeHtml").and_then(|v| v.as_str()) {
                obj.insert("h".into(), serde_json::Value::String(h.to_owned()));
            }
        }
        Ok(payload.to_string())
    }

    fn apply_office_edit(&mut self, spec: &str) -> Result<String, JsValue> {
        let effect = if let Some(pptx) = self.pptx.as_mut() {
            pptx.ensure_edit(&self.bytes)
                .map_err(|e| JsValue::from(OoxmlError::with_code("open_failed", e)))?;
            pptx.apply_raw_edit(spec)
                .map_err(|e| JsValue::from(OoxmlError::with_code("edit_failed", e)))?
        } else if let Some(docx) = self.docx.as_mut() {
            docx.ensure_edit(&self.bytes)
                .map_err(|e| JsValue::from(OoxmlError::with_code("open_failed", e)))?;
            docx.apply_raw_edit(spec)
                .map_err(|e| JsValue::from(OoxmlError::with_code("edit_failed", e)))?
        } else {
            return Err(JsValue::from(OoxmlError::with_code(
                "no_document",
                "not opened",
            )));
        };
        let save = matches!(effect, crate::office_edit::OfficeEffect::Save);
        if !save {
            self.refresh_office_view()?;
            self.sync_office_tabs();
            process_frame(self);
        }
        Ok(self.chrome_payload(save))
    }

    fn refresh_xlsx_tabs(&mut self) {
        let Some(xlsx) = self.xlsx.as_ref() else {
            return;
        };
        let tabs = xlsx
            .sheets_json()
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| {
                        let index = s.get("index")?.as_u64()? as u32;
                        let name = s.get("name")?.as_str()?.to_owned();
                        Some((index, name))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let active = xlsx.active as u32;
        self.shell.set_tabs(tabs, active);
    }

    fn chrome_payload(&mut self, save: bool) -> String {
        self.refresh_formula_bar();
        let ui = ShellUi::to_json(&self.shell, &self.editor);
        let h = ui
            .get("chromeHtml")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        serde_json::json!({
            "h": h,
            "ui": ui,
            "save": save,
            "ribbonAction": self.editor.commands.last_ribbon_action,
        })
        .to_string()
    }

    fn editor_chrome_op(&mut self, op: &str) -> Result<String, JsValue> {
        if self.editor.read_only() {
            return Ok(self.chrome_payload(false));
        }
        if let Some(xlsx) = self.xlsx.as_mut() {
            xlsx.ensure_edit(&self.bytes)
                .map_err(|e| JsValue::from(OoxmlError::with_code("open_failed", e)))?;
            match op {
                "formula-cancel" => xlsx.formula_cancel(),
                "formula-commit" => {
                    xlsx.formula_commit()
                        .map_err(|e| JsValue::from(OoxmlError::with_code("edit_failed", e)))?;
                }
                "formula-fx" => {
                    xlsx.formula_fx()
                        .map_err(|e| JsValue::from(OoxmlError::with_code("edit_failed", e)))?;
                }
                "formula-expand" => xlsx.formula_expand(),
                "formula-focus" => {
                    xlsx.formula_focus()
                        .map_err(|e| JsValue::from(OoxmlError::with_code("edit_failed", e)))?;
                }
                "save" => {}
                _ => {}
            }
        }
        let save = op == "save";
        if !save {
            process_frame(self);
        }
        Ok(self.chrome_payload(save))
    }

    fn dispatch_chrome_command(&mut self, id: &str) -> Result<String, JsValue> {
        let mut save = false;
        let mut structure = false;
        let mut office_applied = false;
        if !self.editor.read_only() {
            if let Some(xlsx) = self.xlsx.as_mut() {
                xlsx.ensure_edit(&self.bytes)
                    .map_err(|e| JsValue::from(OoxmlError::with_code("open_failed", e)))?;
                match xlsx.dispatch_chrome(id) {
                    Ok(nitro_xml_xlsx::editor::ChromeEffect::Save) => save = true,
                    Ok(nitro_xml_xlsx::editor::ChromeEffect::Applied { structure: s }) => {
                        structure = s;
                    }
                    Ok(nitro_xml_xlsx::editor::ChromeEffect::Unmapped) | Err(_) => {}
                }
            } else if self.pptx.is_some() || self.docx.is_some() {
                match self.dispatch_office_command(id)? {
                    crate::office_edit::OfficeEffect::Save => save = true,
                    crate::office_edit::OfficeEffect::Applied => office_applied = true,
                }
            }
        }
        let host: &mut dyn std::any::Any = &mut ();
        let _ran = self.editor.dispatch_command(id, host);
        if structure {
            self.refresh_xlsx_tabs();
        }
        if office_applied {
            self.refresh_office_view()?;
            self.sync_office_tabs();
        }
        if !save {
            process_frame(self);
        }
        Ok(self.chrome_payload(save))
    }

    fn set_font_source_default(&mut self) {
        self.font_source = FontSource::google_fonts();
        self.font_source_resolver = None;
    }

    fn set_font_source_base_url(&mut self, base_url: String) {
        self.font_source = FontSource::mirror(base_url);
        self.font_source_resolver = None;
    }

    fn set_font_source_off(&mut self) {
        self.font_source = FontSource::off();
        self.font_source_resolver = None;
    }

    fn set_font_source_resolver(&mut self, resolve: Function) {
        // Resolver implies remote fetch is allowed; keep default base for any
        // engine-built href passed into future painter paths.
        if !self.font_source.allows_remote_fetch() {
            self.font_source = FontSource::google_fonts();
        }
        self.font_source_resolver = Some(resolve);
    }

    fn record_font_substitutions(&mut self, requested: &[String]) {
        for family in requested {
            let sub = nitro_xml::opc::fonts::FontSubstitution::local(family.clone());
            let message = sub.message();
            self.shell
                .diagnostics
                .ingest_warning(FONT_SUBSTITUTION_CODE, &message, None);
            self.shell.dirty_ui = true;
            self.warnings.push(Warning::new(
                FONT_SUBSTITUTION_CODE.to_owned(),
                message,
                None,
            ));
        }
    }

    fn run_find(&mut self) -> Result<(), JsValue> {
        let query = self.shell.find.query.clone();
        let case_sensitive = self.shell.find.case_sensitive;
        let match_entire = self.shell.find.match_entire;
        let matches = if self.xlsx.is_some() {
            self.find_xlsx(&query, case_sensitive, match_entire)?
        } else if self.docx.is_some() {
            let bytes = std::sync::Arc::clone(&self.bytes);
            if let Some(docx) = self.docx.as_mut() {
                let _ = docx.flush_pending_body(bytes.as_ref());
                if docx.session.body_truncated {
                    let _ = docx.session.ensure_body_complete(bytes.as_ref());
                }
            }
            self.docx
                .as_ref()
                .map(|d| d.search_body(&query, case_sensitive, match_entire))
                .unwrap_or_default()
        } else if let Some(pptx) = self.pptx.as_mut() {
            pptx.search(&query, case_sensitive, match_entire)
                .map_err(|e| JsValue::from(OoxmlError::with_code("search_failed", e)))?
        } else {
            Vec::new()
        };
        self.install_find_matches(matches);
        Ok(())
    }

    fn find_xlsx(
        &mut self,
        query: &str,
        case_sensitive: bool,
        match_entire: bool,
    ) -> Result<Vec<FindMatch>, JsValue> {
        let opts = nitro_xml_xlsx::SearchOptions {
            case_sensitive,
            match_entire,
        };
        let xlsx = self
            .xlsx
            .as_mut()
            .ok_or_else(|| JsValue::from(OoxmlError::with_code("no_document", "not opened")))?;
        let v = xlsx
            .search(&self.bytes, query, opts)
            .map_err(|e| JsValue::from(OoxmlError::with_code("search_failed", e)))?;
        let mut matches = Vec::new();
        if let Some(arr) = v.as_array() {
            for item in arr {
                let sheet = item.get("sheet").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                let row = item.get("row").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                let col = item.get("col").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                let text = item
                    .get("text")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_owned();
                matches.push(FindMatch {
                    sheet,
                    row,
                    col,
                    text,
                });
            }
        }
        Ok(matches)
    }

    fn install_find_matches(&mut self, matches: Vec<FindMatch>) {
        self.shell.find.matches = matches;
        self.shell.find.index = if self.shell.find.matches.is_empty() {
            None
        } else {
            Some(0)
        };
        self.shell.dirty_ui = true;
    }

    fn copy_docx_plain(&self) -> String {
        let selection = self.shell.selection_ref.clone();
        self.docx
            .as_ref()
            .map(|d| d.copy_plain(&selection))
            .unwrap_or_default()
    }

    fn activate_docx_hyperlink(&mut self, row: u32, col: u32) -> Result<(), JsValue> {
        let hit = self.docx.as_ref().and_then(|d| d.hyperlink_at(row, col));
        let Some(hit) = hit else {
            self.shell.pending_hyperlink = None;
            self.shell.dirty_ui = true;
            return Ok(());
        };
        if let Some(page) = hit.page {
            if let Some(docx) = self.docx.as_mut() {
                docx.set_active_page(page as usize)
                    .map_err(|e| JsValue::from(OoxmlError::with_code("bad_page", e)))?;
            }
            self.shell.nav_index = page;
        }
        if let Some(id) = hit.block_id {
            if !id.is_empty() {
                self.shell.selection_ref = id;
            }
        }
        self.shell.pending_hyperlink = Some(hit.pending);
        self.shell.dirty_ui = true;
        Ok(())
    }

    /// Shell projection for the #29 differential (zoom, tabs, find, copy, links, diagnostics).
    /// `negative` forces zoom `0` so the comparator must reject the snapshot.
    pub fn shell_parity_json(&mut self, negative: bool) -> String {
        if self.pptx.is_some() {
            let id = self.pptx.as_mut().and_then(|p| p.first_text_element_id());
            if let Some(id) = id {
                let _ = self.shell_action("select", &id);
            }
        }
        let _ = self.shell_action("find-query", "a");
        let _ = self.shell_action("copy", "");
        // Docx shell dump matches the TS oracle's whole-body `copySample`; interactive
        // `copy` without a block id stays empty so live selection can use DOM text.
        let docx_body = self.docx.as_ref().map(|d| d.body_plain());
        if let Some(body) = docx_body {
            self.shell.last_copy = body;
        }
        let links = if let Some(docx) = self.docx.as_ref() {
            docx.hyperlinks_json()
        } else {
            // TODO(#27): pptx a:hlinkClick targets are not on the slide model.
            Vec::new()
        };
        let find: Vec<serde_json::Value> = self
            .shell
            .find
            .matches
            .iter()
            .take(20)
            .map(|m| {
                serde_json::json!({
                    "sheet": m.sheet,
                    "row": m.row,
                    "col": m.col,
                    "text": m.text,
                })
            })
            .collect();
        let tabs: Vec<serde_json::Value> = self
            .shell
            .tabs
            .iter()
            .map(|(index, name)| {
                serde_json::json!({
                    "index": index,
                    "name": name,
                    "active": *index == self.shell.nav_index,
                })
            })
            .collect();
        let selection_kind = if self.pptx.is_some() {
            if self.shell.selection_ref.is_empty() || self.shell.selection_ref == "A1" {
                "none"
            } else {
                "shape"
            }
        } else if self.docx.is_some() {
            if self.shell.selection_ref.is_empty() || self.shell.selection_ref == "A1" {
                "body"
            } else {
                "paragraph"
            }
        } else {
            "cell"
        };
        let mut diagnostics = self.shell.diagnostics.to_json();
        if let Some(obj) = diagnostics.as_object_mut() {
            obj.remove("parseMs");
            obj.remove("paintMs");
            for key in ["warnings", "fontSubstitutions", "unsupported"] {
                if let Some(arr) = obj.get_mut(key).and_then(|v| v.as_array_mut()) {
                    arr.sort_by(|a, b| {
                        let message = |v: &serde_json::Value| {
                            v.get("message")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_owned()
                        };
                        let code = |v: &serde_json::Value| {
                            v.get("code")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_owned()
                        };
                        (message(a), code(a)).cmp(&(message(b), code(b)))
                    });
                }
            }
        }
        serde_json::json!({
            "format": self.shell.format.as_str(),
            "zoom": if negative { 0.0 } else { self.shell.zoom_pct },
            "navIndex": self.shell.nav_index,
            "tabs": tabs,
            "selectionKind": selection_kind,
            "copySample": cap_chars(&self.shell.last_copy, 4096),
            "hyperlinks": links,
            "findSample": find,
            "diagnostics": diagnostics,
        })
        .to_string()
    }

    fn copy_pptx_selection(&mut self) -> Result<String, JsValue> {
        let pptx = self
            .pptx
            .as_mut()
            .ok_or_else(|| JsValue::from(OoxmlError::with_code("no_document", "not opened")))?;
        Ok(pptx.selected_text())
    }

    fn copy_selection_tsv(&mut self) -> Result<String, JsValue> {
        let range = self.shell.selection_ref.clone();
        let sheet = self.shell.nav_index;
        let xlsx = self
            .xlsx
            .as_mut()
            .ok_or_else(|| JsValue::from(OoxmlError::with_code("no_document", "not opened")))?;
        xlsx.cell_text(&self.bytes, sheet as usize, &range)
            .map_err(|e| JsValue::from(OoxmlError::with_code("bad_range", e)))
    }

    fn activate_hyperlink(&mut self, sheet: u32, row: u32, col: u32) -> Result<(), JsValue> {
        let xlsx = self
            .xlsx
            .as_mut()
            .ok_or_else(|| JsValue::from(OoxmlError::with_code("no_document", "not opened")))?;
        match xlsx.hyperlink_at(&self.bytes, sheet as usize, row, col) {
            Ok(Some(v)) => {
                let target = v.get("target").and_then(|x| x.as_str()).map(str::to_owned);
                let location = v
                    .get("location")
                    .and_then(|x| x.as_str())
                    .map(str::to_owned);
                let open = v.get("open").and_then(|x| x.as_bool()).unwrap_or(false);
                let tooltip = v.get("tooltip").and_then(|x| x.as_str()).map(str::to_owned);
                self.shell.pending_hyperlink = Some(PendingHyperlink {
                    target,
                    location,
                    open,
                    tooltip,
                });
                Ok(())
            }
            Ok(None) => {
                self.shell.pending_hyperlink = None;
                Ok(())
            }
            Err(e) => Err(JsValue::from(OoxmlError::with_code("bad_sheet", e))),
        }
    }
}

/// A non-A1 argument that is not an A1 range (pptx shape / element id).
fn looks_like_element_id(r: &str) -> bool {
    let t = r.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("A1") {
        return false;
    }
    nitro_xml_xlsx::parse_range(t).is_none()
}

fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
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
        let mut h = Handle::new(Vec::new());
        let a = h.intern("Calibri");
        let b = h.intern("Calibri");
        let c = h.intern("Arial");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(h.resolve(a).as_deref(), Some("Calibri"));
    }
}
