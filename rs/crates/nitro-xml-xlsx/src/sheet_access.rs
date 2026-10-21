// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/cells.ts (getCell, usedRange, mergeAt);
// changes listed in PROVENANCE.md.

use crate::fast_hash::FastMap;

use crate::model::{ActiveWorksheet, CellRange, DumpCell, DumpCellValue};

/// Row-major cell index built from [`ActiveWorksheet::cells`].
///
/// `PartialEq` is always true: this is a cache, not part of sheet identity.
#[derive(Debug, Default, Clone)]
pub struct CellIndex {
    rows: FastMap<u32, FastMap<u32, usize>>,
    /// `cells.len()` when this index was last fully synced.
    len: usize,
    /// Cached bounding box; maintained on insert / full rebuild.
    used: Option<CellRange>,
}

impl PartialEq for CellIndex {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for CellIndex {}

impl CellIndex {
    pub fn build(sheet: &ActiveWorksheet) -> Self {
        let mut rows: FastMap<u32, FastMap<u32, usize>> = FastMap::default();
        let mut min_row = u32::MAX;
        let mut max_row = 0u32;
        let mut min_col = u32::MAX;
        let mut max_col = 0u32;
        for (idx, cell) in sheet.cells.iter().enumerate() {
            rows.entry(cell.row).or_default().insert(cell.col, idx);
            min_row = min_row.min(cell.row);
            max_row = max_row.max(cell.row);
            min_col = min_col.min(cell.col);
            max_col = max_col.max(cell.col);
        }
        let used = if sheet.cells.is_empty() {
            None
        } else {
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
        };
        Self {
            rows,
            len: sheet.cells.len(),
            used,
        }
    }

    #[inline]
    pub fn index_of(&self, row: u32, col: u32) -> Option<usize> {
        self.rows.get(&row)?.get(&col).copied()
    }

    pub fn get<'a>(&self, sheet: &'a ActiveWorksheet, row: u32, col: u32) -> Option<&'a DumpCell> {
        let idx = self.index_of(row, col)?;
        sheet.cells.get(idx)
    }

    pub fn row_cols(&self, row: u32) -> Option<&FastMap<u32, usize>> {
        self.rows.get(&row)
    }

    pub fn synced_len(&self) -> usize {
        self.len
    }

    /// Bounding box of indexed cells, or `None` when empty.
    pub fn used_range(&self) -> Option<CellRange> {
        self.used.clone()
    }

    pub fn insert(&mut self, row: u32, col: u32, idx: usize) {
        self.rows.entry(row).or_default().insert(col, idx);
        match &mut self.used {
            None => {
                self.used = Some(CellRange {
                    start: crate::model::CellAddress { row, col },
                    end: crate::model::CellAddress { row, col },
                });
            }
            Some(u) => {
                u.start.row = u.start.row.min(row);
                u.start.col = u.start.col.min(col);
                u.end.row = u.end.row.max(row);
                u.end.col = u.end.col.max(col);
            }
        }
    }

    pub fn clear_at(&mut self, row: u32, col: u32) {
        let mut empty = false;
        if let Some(cols) = self.rows.get_mut(&row) {
            cols.remove(&col);
            empty = cols.is_empty();
        }
        if empty {
            self.rows.remove(&row);
        }
        // Shrink only when the removed cell sat on the cached edge.
        if let Some(u) = &self.used {
            if row == u.start.row || row == u.end.row || col == u.start.col || col == u.end.col {
                self.recompute_used();
            }
        }
    }

    fn recompute_used(&mut self) {
        if self.rows.is_empty() {
            self.used = None;
            return;
        }
        let mut min_row = u32::MAX;
        let mut max_row = 0u32;
        let mut min_col = u32::MAX;
        let mut max_col = 0u32;
        for (&row, cols) in &self.rows {
            min_row = min_row.min(row);
            max_row = max_row.max(row);
            for &col in cols.keys() {
                min_col = min_col.min(col);
                max_col = max_col.max(col);
            }
        }
        self.used = Some(CellRange {
            start: crate::model::CellAddress {
                row: min_row,
                col: min_col,
            },
            end: crate::model::CellAddress {
                row: max_row,
                col: max_col,
            },
        });
    }

    /// After `cells.remove(i)`: drop the removed key and shift indices above `i`.
    pub fn remove_shifting(&mut self, removed_row: u32, removed_col: u32, i: usize) {
        self.clear_at(removed_row, removed_col);
        for cols in self.rows.values_mut() {
            for idx in cols.values_mut() {
                if *idx > i {
                    *idx -= 1;
                }
            }
        }
        self.len = self.len.saturating_sub(1);
    }
}

impl ActiveWorksheet {
    pub fn find_cell_pos(&self, row: u32, col: u32) -> Option<usize> {
        if self.cell_pos.synced_len() == self.cells.len() {
            return self.cell_pos.index_of(row, col);
        }
        // Stale cache (bulk replace forgot rebuild) — linear fallback.
        self.cells.iter().position(|c| c.row == row && c.col == col)
    }

    pub fn ensure_cell_pos(&mut self) {
        if self.cell_pos.synced_len() != self.cells.len() {
            self.rebuild_cell_pos();
        }
    }

    pub fn rebuild_cell_pos(&mut self) {
        self.cell_pos = CellIndex::build(self);
    }

    pub fn push_cell(&mut self, cell: DumpCell) {
        let row = cell.row;
        let col = cell.col;
        self.cells.push(cell);
        let idx = self.cells.len() - 1;
        self.cell_pos.insert(row, col, idx);
        self.cell_pos.len += 1;
    }

    pub fn remove_cell_pos(&mut self, i: usize) {
        let removed = self.cells.remove(i);
        self.cell_pos.remove_shifting(removed.row, removed.col, i);
    }

    pub fn upsert_dump_cell(&mut self, cell: DumpCell) {
        self.ensure_cell_pos();
        if let Some(i) = self.cell_pos.index_of(cell.row, cell.col) {
            self.cells[i] = cell;
        } else {
            self.push_cell(cell);
        }
    }

    pub fn remove_cell_at_coord(&mut self, row: u32, col: u32) {
        self.ensure_cell_pos();
        if let Some(i) = self.cell_pos.index_of(row, col) {
            self.remove_cell_pos(i);
        }
    }

    pub fn replace_cells(&mut self, cells: Vec<DumpCell>) {
        self.cells = cells;
        self.rebuild_cell_pos();
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
    // Prefer `<dimension>` (lazy first-view / explicit) — O(1).
    if let Some(dim) = &sheet.dimension {
        return Some(dim.clone());
    }
    // Maintained index: O(occupied rows), not O(cells). Scanning cells_1m here
    // cost ~4ms+ per paint and raised edit-cell L median after ensure_edit.
    if sheet.cell_pos.synced_len() == sheet.cells.len() {
        return sheet.cell_pos.used_range();
    }
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
