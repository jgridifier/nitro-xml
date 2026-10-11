// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/navigate.ts (arrow keys only)
// and src/ui/src/xlsx/grid/grid-selection.ts (merge far-edge departure);
// changes listed in PROVENANCE.md.

use crate::address::{MAX_COL, MAX_ROW};
use crate::model::{ActiveWorksheet, CellAddress, CellRange, SheetSelection};
use crate::sheet_access::merge_at;
use crate::view::metrics::GridMetrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrow {
    Left,
    Right,
    Up,
    Down,
}

impl Arrow {
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "ArrowLeft" => Some(Self::Left),
            "ArrowRight" => Some(Self::Right),
            "ArrowUp" => Some(Self::Up),
            "ArrowDown" => Some(Self::Down),
            _ => None,
        }
    }
}

/// One visible step. Hidden rows/cols (size 0) are skipped; `index` stays put at the edge.
pub fn navigate_arrow(metrics: &GridMetrics, from: CellAddress, arrow: Arrow) -> CellAddress {
    match arrow {
        Arrow::Up => CellAddress {
            row: step(from.row, -1, MAX_ROW, |r| metrics.is_row_hidden(r)),
            col: from.col,
        },
        Arrow::Down => CellAddress {
            row: step(from.row, 1, MAX_ROW, |r| metrics.is_row_hidden(r)),
            col: from.col,
        },
        Arrow::Left => CellAddress {
            row: from.row,
            col: step(from.col, -1, MAX_COL, |c| metrics.is_col_hidden(c)),
        },
        Arrow::Right => CellAddress {
            row: from.row,
            col: step(from.col, 1, MAX_COL, |c| metrics.is_col_hidden(c)),
        },
    }
}

/// Where an unshifted arrow starts. Down/Right leave a merge from its far edge.
pub fn arrow_origin(
    sheet: &ActiveWorksheet,
    selection: &SheetSelection,
    arrow: Arrow,
) -> CellAddress {
    let active = selection.active;
    let far = merge_at(sheet, active.row, active.col)
        .cloned()
        .unwrap_or(CellRange {
            start: active,
            end: active,
        });
    match arrow {
        Arrow::Down => CellAddress {
            row: far.end.row,
            col: active.col,
        },
        Arrow::Right => CellAddress {
            row: active.row,
            col: far.end.col,
        },
        Arrow::Up | Arrow::Left => active,
    }
}

fn step(index: u32, dir: i32, max: u32, hidden: impl Fn(u32) -> bool) -> u32 {
    let mut i = index as i64 + i64::from(dir);
    let max = i64::from(max);
    while i >= 0 && i <= max {
        let at = i as u32;
        if !hidden(at) {
            return at;
        }
        i += i64::from(dir);
    }
    index
}
