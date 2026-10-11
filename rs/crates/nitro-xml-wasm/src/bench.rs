// SPDX-License-Identifier: MIT
//! Bench-only synthetic grids and exports (`cargo build --features bench`).
//!
//! Not linked into the shipped wasm (`scripts/build-wasm.sh` leaves `bench` off).
//! Development comes first; speed testing is deferred (Jared, 2026-10-11).

use crate::handle::{Handle, OoxmlError};
use crate::transport::StringTable;
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// One logical cell record, used only for serde/JSON bench arms.
#[derive(Serialize)]
pub struct CellRecord {
    pub row: u32,
    pub col: u32,
    pub value: f64,
    pub style: u32,
    pub text_id: u32,
}

/// Struct-of-arrays grid held in wasm linear memory for zero-copy TypedArray views.
#[derive(Debug, Default)]
pub struct SyntheticGrid {
    pub row: Vec<u32>,
    pub col: Vec<u32>,
    pub value: Vec<f64>,
    pub style: Vec<u32>,
    pub text_id: Vec<u32>,
}

impl SyntheticGrid {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.row.len()
    }

    pub fn fill(&mut self, n: u32, strings: &mut StringTable) {
        let n = n as usize;
        self.row.clear();
        self.col.clear();
        self.value.clear();
        self.style.clear();
        self.text_id.clear();
        self.row.reserve(n);
        self.col.reserve(n);
        self.value.reserve(n);
        self.style.reserve(n);
        self.text_id.reserve(n);
        let labels = [
            strings.intern(""),
            strings.intern("n"),
            strings.intern("s"),
            strings.intern("b"),
        ];
        for i in 0..n {
            let row = (i / 26) as u32;
            let col = (i % 26) as u32;
            self.row.push(row);
            self.col.push(col);
            self.value.push((i as f64) * 0.5 + 1.0);
            self.style.push((i % 16) as u32);
            self.text_id.push(labels[i % labels.len()]);
        }
    }

    pub fn touch(&self, i: u32) -> f64 {
        self.value.get(i as usize).copied().unwrap_or(f64::NAN)
    }

    pub fn to_records(&self) -> Vec<CellRecord> {
        (0..self.len())
            .map(|i| CellRecord {
                row: self.row[i],
                col: self.col[i],
                value: self.value[i],
                style: self.style[i],
                text_id: self.text_id[i],
            })
            .collect()
    }

    pub fn to_serde_js(&self) -> Result<JsValue, serde_wasm_bindgen::Error> {
        serde_wasm_bindgen::to_value(&self.to_records())
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.to_records())
    }
}

#[wasm_bindgen]
impl Handle {
    /// Fill the synthetic SoA grid with `n` cell records for boundary benches.
    #[wasm_bindgen(js_name = fillSyntheticGrid)]
    pub fn fill_synthetic_grid(&mut self, n: u32) {
        self.grid.fill(n, &mut self.strings);
    }

    #[wasm_bindgen(getter, js_name = gridLen)]
    pub fn grid_len(&self) -> usize {
        self.grid.len()
    }

    #[wasm_bindgen(js_name = gridRowPtr)]
    pub fn grid_row_ptr(&self) -> *const u32 {
        self.grid.row.as_ptr()
    }

    #[wasm_bindgen(js_name = gridColPtr)]
    pub fn grid_col_ptr(&self) -> *const u32 {
        self.grid.col.as_ptr()
    }

    #[wasm_bindgen(js_name = gridValuePtr)]
    pub fn grid_value_ptr(&self) -> *const f64 {
        self.grid.value.as_ptr()
    }

    #[wasm_bindgen(js_name = gridStylePtr)]
    pub fn grid_style_ptr(&self) -> *const u32 {
        self.grid.style.as_ptr()
    }

    #[wasm_bindgen(js_name = gridTextIdPtr)]
    pub fn grid_text_id_ptr(&self) -> *const u32 {
        self.grid.text_id.as_ptr()
    }

    /// Export the synthetic grid as a JS array of objects (`serde-wasm-bindgen`).
    #[wasm_bindgen(js_name = exportRecordsSerde)]
    pub fn export_records_serde(&self) -> Result<JsValue, JsValue> {
        self.grid
            .to_serde_js()
            .map_err(|e| JsValue::from(OoxmlError::with_code("serde_export", e.to_string())))
    }

    /// Export the synthetic grid as a JSON string.
    #[wasm_bindgen(js_name = exportRecordsJson)]
    pub fn export_records_json(&self) -> Result<String, JsValue> {
        self.grid
            .to_json()
            .map_err(|e| JsValue::from(OoxmlError::with_code("json_export", e.to_string())))
    }

    /// Trivial per-call touch (bench contrast only; never on the render path).
    #[wasm_bindgen(js_name = touchCell)]
    pub fn touch_cell(&mut self, i: u32) -> f64 {
        self.grid.touch(i)
    }

    /// Encode a demo frame of `n_ops` mixed draw/DOM ops for the microbench.
    #[wasm_bindgen(js_name = encodeDemoFrame)]
    pub fn encode_demo_frame(&mut self, n_ops: u32) {
        self.commands.encode_demo(n_ops);
    }
}
