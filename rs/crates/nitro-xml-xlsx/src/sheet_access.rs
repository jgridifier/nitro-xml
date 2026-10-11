// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/cells.ts (getCell, usedRange, mergeAt);
// changes listed in PROVENANCE.md.

use std::collections::HashMap;

use crate::model::{ActiveWorksheet, CellRange, DumpCell, DumpCellValue};

/// Row-major cell index built from [`ActiveWorksheet::cells`].
#[derive(Debug, Default)]
pub struct CellIndex {
    rows: HashMap<u32, HashMap<u32, usize>>,
}

impl CellIndex {
    pub fn build(sheet: &ActiveWorksheet) -> Self {
        let mut rows = HashMap::new();
        for (idx, cell) in sheet.cells.iter().enumerate() {
            rows.entry(cell.row)
                .or_insert_with(HashMap::new)
                .insert(cell.col, idx);
        }
        Self { rows }
    }

    pub fn get<'a>(&self, sheet: &'a ActiveWorksheet, row: u32, col: u32) -> Option<&'a DumpCell> {
        let idx = self.rows.get(&row)?.get(&col)?;
        sheet.cells.get(*idx)
    }

    pub fn row_cols(&self, row: u32) -> Option<&HashMap<u32, usize>> {
        self.rows.get(&row)
    }
}

pub fn get_cell<'a>(
    sheet: &'a ActiveWorksheet,
    index: &'a CellIndex,
    row: u32,
    col: u32,
) -> Option<&'a DumpCell> {
    index.get(sheet, row, col)
}

pub fn used_range(sheet: &ActiveWorksheet) -> Option<CellRange> {
    let mut min_row = u32::MAX;
    let mut max_row = 0u32;
    let mut min_col = u32::MAX;
    let mut max_col = 0u32;
    let mut any = false;
    for cell in &sheet.cells {
        any = true;
        min_row = min_row.min(cell.row);
        max_row = max_row.max(cell.row);
        min_col = min_col.min(cell.col);
        max_col = max_col.max(cell.col);
    }
    if !any {
        return None;
    }
    Some(CellRange {
        start: crate::model::CellAddress {
            row: min_row,
            col: min_col,
        },
        end: crate::model::CellAddress {
            row: max_row,
            col: max_col,
        },
    })
}

pub fn merge_at(sheet: &ActiveWorksheet, row: u32, col: u32) -> Option<&CellRange> {
    sheet
        .merges
        .iter()
        .find(|m| row >= m.start.row && row <= m.end.row && col >= m.start.col && col <= m.end.col)
}

pub fn range_contains(range: &CellRange, row: u32, col: u32) -> bool {
    row >= range.start.row && row <= range.end.row && col >= range.start.col && col <= range.end.col
}

pub fn dump_value_to_cell_value(value: &DumpCellValue) -> crate::numfmt::CellValue {
    match value {
        DumpCellValue::Null => crate::numfmt::CellValue::Blank,
        DumpCellValue::Number(n) => crate::numfmt::CellValue::Number(*n),
        DumpCellValue::NumberRaw(s) => s
            .parse::<f64>()
            .map(crate::numfmt::CellValue::Number)
            .unwrap_or(crate::numfmt::CellValue::Text(s.clone())),
        DumpCellValue::String(s) => crate::numfmt::CellValue::Text(s.clone()),
        DumpCellValue::Bool(b) => crate::numfmt::CellValue::Bool(*b),
        DumpCellValue::Error(e) => crate::numfmt::CellValue::Error(e.clone()),
    }
}

pub fn is_cell_error(value: &DumpCellValue) -> bool {
    matches!(value, DumpCellValue::Error(_))
}
