// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/ui/grid/selection-ops.ts (row/col header state);
// changes listed in PROVENANCE.md.

use crate::address::{MAX_COL, MAX_ROW};
use crate::model::{CellRange, SheetSelection};

pub type HeaderState = &'static str;

fn is_whole_rows(range: &CellRange) -> bool {
    range.start.col == 0 && range.end.col == MAX_COL
}

fn is_whole_cols(range: &CellRange) -> bool {
    range.start.row == 0 && range.end.row == MAX_ROW
}

pub fn row_selected(selection: &SheetSelection, row: u32) -> HeaderState {
    let mut state = "none";
    for r in &selection.ranges {
        if row >= r.start.row && row <= r.end.row {
            if is_whole_rows(r) {
                return "full";
            }
            state = "part";
        }
    }
    state
}

pub fn col_selected(selection: &SheetSelection, col: u32) -> HeaderState {
    let mut state = "none";
    for r in &selection.ranges {
        if col >= r.start.col && col <= r.end.col {
            if is_whole_cols(r) {
                return "full";
            }
            state = "part";
        }
    }
    state
}
