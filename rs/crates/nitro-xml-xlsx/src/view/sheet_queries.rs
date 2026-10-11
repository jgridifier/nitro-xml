// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/sheet-queries.ts (overflowExtent);
// changes listed in PROVENANCE.md.

use crate::address::MAX_COL;
use crate::model::ActiveWorksheet;
use crate::sheet_access::{get_cell, merge_at, CellIndex};
use crate::view::metrics::GridMetrics;

pub struct OverflowExtent {
    pub start_col: u32,
    pub end_col: u32,
    pub left: i32,
    pub right: i32,
}

fn is_overflow_target(sheet: &ActiveWorksheet, index: &CellIndex, row: u32, col: u32) -> bool {
    if let Some(cell) = get_cell(sheet, index, row, col) {
        let has_value = match &cell.value {
            crate::model::DumpCellValue::Null => false,
            crate::model::DumpCellValue::String(s) => !s.is_empty(),
            _ => true,
        } || cell.formula.is_some();
        if has_value {
            return false;
        }
    }
    merge_at(sheet, row, col).is_none()
}

#[allow(clippy::too_many_arguments)]
pub fn overflow_extent(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    metrics: &GridMetrics,
    row: u32,
    col: u32,
    text_width: i32,
    h_align: &str,
    max_span: u32,
) -> OverflowExtent {
    let width = metrics.col_width(col);
    let mut start_col = col;
    let mut end_col = col;
    if text_width <= width || merge_at(sheet, row, col).is_some() {
        return OverflowExtent {
            start_col,
            end_col,
            left: metrics.col_left(start_col),
            right: metrics.col_left(end_col) + metrics.col_width(end_col),
        };
    }
    let extend = |mut c: u32, need: i32, step: i32| -> u32 {
        let mut gained = 0;
        for _ in 0..max_span {
            if gained >= need {
                break;
            }
            let next = c as i32 + step;
            if next < 0 || next > MAX_COL as i32 {
                break;
            }
            let next = next as u32;
            if !is_overflow_target(sheet, index, row, next) {
                break;
            }
            c = next;
            gained += metrics.col_width(c);
        }
        c
    };
    if h_align == "left" || h_align == "justify" || h_align == "distributed" {
        end_col = extend(col, text_width - width, 1);
    } else if h_align == "right" {
        start_col = extend(col, text_width - width, -1);
    } else if h_align == "center" || h_align == "centerContinuous" {
        let side = (text_width - width) / 2;
        start_col = extend(col, side, -1);
        end_col = extend(col, side, 1);
    }
    OverflowExtent {
        start_col,
        end_col,
        left: metrics.col_left(start_col),
        right: metrics.col_left(end_col) + metrics.col_width(end_col),
    }
}
