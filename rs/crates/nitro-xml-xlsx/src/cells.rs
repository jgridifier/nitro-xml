// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/cells.ts (storage layout); changes listed in
// PROVENANCE.md.
//! Columnar cell store (parallel arrays + side maps).
//!
//! Shared-string cells keep an SST index (`CellTypeTag::Shared`) instead of cloning text.
//! Numeric cells store `f64` only — gate `__NUM__:` text is formatted at dump time.

use crate::fast_hash::FastMap;

use crate::model::{CellAddress, CellRange, RichTextRun};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellTypeTag {
    Empty,
    Number,
    /// Inline / formula / non-SST string (`CellStore.strings`).
    String,
    /// Shared string table index in `value_indices` (no text copy).
    Shared,
    Bool,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CellStore {
    rows: Vec<u32>,
    cols: Vec<u32>,
    types: Vec<CellTypeTag>,
    value_indices: Vec<u32>,
    style_indices: Vec<u32>,
    numbers: Vec<f64>,
    strings: Vec<String>,
    bools: Vec<bool>,
    errors: Vec<String>,
    formulas: FastMap<usize, String>,
    rich_text: FastMap<usize, Vec<RichTextRun>>,
    array_range: FastMap<usize, CellRange>,
    legacy_formula: FastMap<usize, bool>,
    dynamic_array: FastMap<usize, bool>,
    spill_anchor: FastMap<usize, CellAddress>,
    cell_metadata: FastMap<usize, u32>,
    value_metadata: FastMap<usize, u32>,
}

impl CellStore {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            cols: Vec::new(),
            types: Vec::new(),
            value_indices: Vec::new(),
            style_indices: Vec::new(),
            numbers: Vec::new(),
            strings: Vec::new(),
            bools: Vec::new(),
            errors: Vec::new(),
            formulas: FastMap::default(),
            rich_text: FastMap::default(),
            array_range: FastMap::default(),
            legacy_formula: FastMap::default(),
            dynamic_array: FastMap::default(),
            spill_anchor: FastMap::default(),
            cell_metadata: FastMap::default(),
            value_metadata: FastMap::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn reserve(&mut self, cells: usize) {
        self.rows.reserve(cells);
        self.cols.reserve(cells);
        self.types.reserve(cells);
        self.value_indices.reserve(cells);
        self.style_indices.reserve(cells);
        self.numbers.reserve(cells);
    }

    pub fn shrink_to_fit(&mut self) {
        self.rows.shrink_to_fit();
        self.cols.shrink_to_fit();
        self.types.shrink_to_fit();
        self.value_indices.shrink_to_fit();
        self.style_indices.shrink_to_fit();
        self.numbers.shrink_to_fit();
        self.strings.shrink_to_fit();
        self.bools.shrink_to_fit();
        self.errors.shrink_to_fit();
    }

    pub fn push_number(
        &mut self,
        row: u32,
        col: u32,
        style: u32,
        n: f64,
        _raw: Option<&str>,
    ) -> usize {
        self.push_cell(
            row,
            col,
            style,
            CellTypeTag::Number,
            self.numbers.len() as u32,
        );
        self.numbers.push(n);
        self.rows.len() - 1
    }

    pub fn push_string(&mut self, row: u32, col: u32, style: u32, s: String) -> usize {
        self.push_cell(
            row,
            col,
            style,
            CellTypeTag::String,
            self.strings.len() as u32,
        );
        self.strings.push(s);
        self.rows.len() - 1
    }

    pub fn push_shared(&mut self, row: u32, col: u32, style: u32, sst_idx: u32) -> usize {
        self.push_cell(row, col, style, CellTypeTag::Shared, sst_idx);
        self.rows.len() - 1
    }

    pub fn push_bool(&mut self, row: u32, col: u32, style: u32, b: bool) -> usize {
        self.push_cell(row, col, style, CellTypeTag::Bool, self.bools.len() as u32);
        self.bools.push(b);
        self.rows.len() - 1
    }

    pub fn push_error(&mut self, row: u32, col: u32, style: u32, code: String) -> usize {
        self.push_cell(
            row,
            col,
            style,
            CellTypeTag::Error,
            self.errors.len() as u32,
        );
        self.errors.push(code);
        self.rows.len() - 1
    }

    pub fn push_style_only(&mut self, row: u32, col: u32, style: u32) -> usize {
        self.push_cell(row, col, style, CellTypeTag::Empty, 0);
        self.rows.len() - 1
    }

    fn push_cell(&mut self, row: u32, col: u32, style: u32, tag: CellTypeTag, value_idx: u32) {
        self.rows.push(row);
        self.cols.push(col);
        self.types.push(tag);
        self.value_indices.push(value_idx);
        self.style_indices.push(style);
    }

    pub fn set_formula(&mut self, idx: usize, formula: String) {
        self.formulas.insert(idx, formula);
    }

    pub fn set_rich_text(&mut self, idx: usize, runs: Vec<RichTextRun>) {
        self.rich_text.insert(idx, runs);
    }

    pub fn set_array_range(&mut self, idx: usize, range: CellRange) {
        self.array_range.insert(idx, range);
    }

    pub fn set_legacy_formula(&mut self, idx: usize) {
        self.legacy_formula.insert(idx, true);
    }

    pub fn set_dynamic_array(&mut self, idx: usize) {
        self.dynamic_array.insert(idx, true);
    }

    pub fn set_spill_anchor(&mut self, idx: usize, anchor: CellAddress) {
        self.spill_anchor.insert(idx, anchor);
    }

    pub fn set_cell_metadata(&mut self, idx: usize, cm: u32) {
        self.cell_metadata.insert(idx, cm);
    }

    pub fn set_value_metadata(&mut self, idx: usize, vm: u32) {
        self.value_metadata.insert(idx, vm);
    }

    pub fn clear_legacy_dynamic(&mut self, idx: usize) {
        self.legacy_formula.remove(&idx);
        self.dynamic_array.remove(&idx);
    }

    pub fn iter_sorted(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.len()).collect();
        indices.sort_by_key(|&i| (self.rows[i], self.cols[i]));
        indices
    }

    pub fn row(&self, i: usize) -> u32 {
        self.rows[i]
    }

    pub fn col(&self, i: usize) -> u32 {
        self.cols[i]
    }

    pub fn style(&self, i: usize) -> u32 {
        self.style_indices[i]
    }

    pub fn cell_type(&self, i: usize) -> CellTypeTag {
        self.types[i]
    }

    pub fn number(&self, i: usize) -> f64 {
        self.numbers[self.value_indices[i] as usize]
    }

    pub fn string(&self, i: usize) -> &str {
        &self.strings[self.value_indices[i] as usize]
    }

    pub fn shared_index(&self, i: usize) -> u32 {
        self.value_indices[i]
    }

    pub fn bool_val(&self, i: usize) -> bool {
        self.bools[self.value_indices[i] as usize]
    }

    pub fn error(&self, i: usize) -> &str {
        &self.errors[self.value_indices[i] as usize]
    }

    pub fn formula(&self, i: usize) -> Option<&str> {
        self.formulas.get(&i).map(String::as_str)
    }

    pub fn rich_text(&self, i: usize) -> Option<&[RichTextRun]> {
        self.rich_text.get(&i).map(Vec::as_slice)
    }

    pub fn array_range(&self, i: usize) -> Option<&CellRange> {
        self.array_range.get(&i)
    }

    pub fn legacy_formula(&self, i: usize) -> bool {
        self.legacy_formula.get(&i).copied().unwrap_or(false)
    }

    pub fn dynamic_array(&self, i: usize) -> bool {
        self.dynamic_array.get(&i).copied().unwrap_or(false)
    }

    pub fn spill_anchor(&self, i: usize) -> Option<&CellAddress> {
        self.spill_anchor.get(&i)
    }

    pub fn cell_metadata(&self, i: usize) -> Option<u32> {
        self.cell_metadata.get(&i).copied()
    }

    pub fn value_metadata(&self, i: usize) -> Option<u32> {
        self.value_metadata.get(&i).copied()
    }
}

impl Default for CellStore {
    fn default() -> Self {
        Self::new()
    }
}
