// SPDX-License-Identifier: MIT
//! Bulk transfer and interned strings for the JS/WASM boundary (#14).
//!
//! Per-frame command buffer and measure batches live in [`nitro_xml_render`]
//! (shared with format painters). Decision (see `docs/boundary.md`):
//! struct-of-arrays typed-array views over wasm memory for bulk numeric data;
//! never per-cell JS↔WASM calls on the render path.

/// Per-document interned string table. Id `0` is always the empty string.
///
/// Backed by a `Vec` (linear scan on intern) so the shipped wasm does not pull
/// in `hashbrown` / `RandomState`. Document string tables stay small; bulk
/// traffic uses the numeric ids.
#[derive(Debug, Default)]
pub struct StringTable {
    by_id: Vec<String>,
}

impl StringTable {
    pub fn new() -> Self {
        let mut t = Self { by_id: Vec::new() };
        t.intern("");
        t
    }

    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(id) = self.by_id.iter().position(|x| x == s) {
            return id as u32;
        }
        let id = self.by_id.len() as u32;
        self.by_id.push(s.to_owned());
        id
    }

    pub fn resolve(&self, id: u32) -> Option<&str> {
        self.by_id.get(id as usize).map(String::as_str)
    }

    pub fn len(&self) -> u32 {
        self.by_id.len() as u32
    }
}
