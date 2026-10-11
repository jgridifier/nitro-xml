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
    text_width: f64,
    h_align: &str,
    max_span: u32,
) -> OverflowExtent {
    let width = metrics.col_width(col) as f64;
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
    let extend = |mut c: u32, need: f64, step: i32| -> u32 {
        let mut gained = 0.0;
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
            gained += metrics.col_width(c) as f64;
        }
        c
    };
    if h_align == "left" || h_align == "justify" || h_align == "distributed" {
        end_col = extend(col, text_width - width, 1);
    } else if h_align == "right" {
        start_col = extend(col, text_width - width, -1);
    } else if h_align == "center" || h_align == "centerContinuous" {
        // Keep f64 like upstream so a side of 47.5 still consumes the next column after 47px.
        let side = (text_width - width) / 2.0;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ActiveWorksheet, DumpCell, DumpCellValue, SheetState};
    use crate::sheet_access::CellIndex;
    use crate::sheet_props::default_sheet_view;
    use crate::view::axis_metrics::AxisMetrics;
    use crate::view::metrics::GridMetrics;
    use std::collections::BTreeMap;

    fn sheet_with_hidden_center() -> (ActiveWorksheet, CellIndex, GridMetrics) {
        let sheet = ActiveWorksheet {
            name: "t".into(),
            sheet_id: 1,
            state: SheetState::Visible,
            part_name: None,
            default_row_height: 15.0,
            default_col_width: None,
            format: None,
            view: default_sheet_view(),
            columns: vec![],
            merges: vec![],
            row_info: BTreeMap::new(),
            cells: vec![DumpCell {
                row: 0,
                col: 3,
                value: DumpCellValue::String("Zustand Pilot".into()),
                type_tag: Some("s".into()),
                formula: None,
                style_id: None,
                rich_text: None,
                array_range: None,
                legacy_formula: None,
                dynamic_array: None,
                spill_anchor: None,
                cell_metadata: None,
                value_metadata: None,
            }],
            data_validations: vec![],
            comment_addresses: vec![],
            hyperlinks: vec![],
            ext_lst: vec![],
            sparkline_groups: vec![],
        };
        let index = CellIndex::build(&sheet);
        // cols: 0=26, 1=103, 2=47, 3=0 (hidden), 4+=default
        let cols = AxisMetrics::new(64, 16, vec![(0, 26), (1, 103), (2, 47), (3, 0)]);
        let rows = AxisMetrics::new(16, 16, vec![]);
        let metrics = GridMetrics {
            zoom: 100.0,
            last_row: 10,
            last_col: 10,
            cols,
            rows,
        };
        (sheet, index, metrics)
    }

    #[test]
    fn center_overflow_keeps_fractional_side_need() {
        // Catches: i32 side=(95-0)/2 truncates to 47 so a 47px neighbour stops extend early;
        // JS keeps 47.5 and continues into the next empty column (poi-57181 centerContinuous).
        let (sheet, index, metrics) = sheet_with_hidden_center();
        let extent = overflow_extent(
            &sheet,
            &index,
            &metrics,
            0,
            3,
            95.0,
            "centerContinuous",
            256,
        );
        assert_eq!(extent.start_col, 1);
        assert_eq!(extent.left, 26);
        assert_eq!(extent.right - extent.left, 150);
    }
}
