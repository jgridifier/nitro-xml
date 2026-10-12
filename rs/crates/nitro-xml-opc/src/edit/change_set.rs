// SPDX-License-Identifier: MIT
//! Change-sets drive incremental repaint / relayout after a committed transaction (#63, #81).

use serde::{Deserialize, Serialize};

/// What kind of model mutation a change-set summarises (upstream `WorkbookChange.kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Cells,
    Format,
    Structure,
    Sheets,
    Batch,
    Undo,
    Redo,
    Meta,
    Other,
}

/// Scope recorded for history snapshots (upstream `EditScope`). Full codecs land in #64.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EditScope {
    Cells {
        sheet: u32,
        ranges: Vec<String>,
    },
    Parts {
        sheet: u32,
        parts: Vec<String>,
    },
    Sheet {
        sheet: u32,
    },
    Refs,
    Shift {
        sheet: u32,
        axis: String,
        at: u32,
        count: i32,
    },
    Meta,
    Workbook,
}

/// Stable dirty summary after a successful commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSet {
    pub kind: ChangeKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ranges: Vec<String>,
    #[serde(default)]
    pub structural: bool,
    /// OPC part names that must be rewritten on save (#65); empty until writers land.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dirty_parts: Vec<String>,
}

impl ChangeSet {
    pub fn new(kind: ChangeKind, label: impl Into<String>) -> Self {
        Self {
            kind,
            label: label.into(),
            sheet: None,
            ranges: Vec::new(),
            structural: false,
            dirty_parts: Vec::new(),
        }
    }

    pub fn with_sheet(mut self, sheet: u32) -> Self {
        self.sheet = Some(sheet);
        self
    }

    pub fn with_ranges(mut self, ranges: Vec<String>) -> Self {
        self.ranges = ranges;
        self
    }

    pub fn structural(mut self) -> Self {
        self.structural = true;
        self
    }
}
