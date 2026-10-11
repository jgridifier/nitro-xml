// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/ui/grid/cell-items.ts; changes listed in PROVENANCE.md.

use crate::model::{ActiveWorksheet, CellRange, WorkbookSlice};
use crate::sheet_access::CellIndex;
use crate::view::cell_paint::{css_font, hashes, repeat_to_fill};
use crate::view::cell_view::{cell_view, CellViewOptions};
use crate::view::metrics::GridMetrics;
use crate::view::sheet_queries::overflow_extent;
use crate::view::text_measure::TextMeasure;
use crate::view::types::{CellItem, CellView};

const PADDING: i32 = 2;

fn cell_opts<'a>(input: &'a BuildInput<'a>) -> CellViewOptions<'a> {
    CellViewOptions {
        sheet_index: Some(input.sheet_index),
        all_sheets: Some(input.all_sheets),
        ..CellViewOptions::default()
    }
}

pub struct BuildInput<'a> {
    pub workbook: &'a WorkbookSlice,
    pub sheet: &'a ActiveWorksheet,
    pub sheet_index: usize,
    pub all_sheets: &'a [Option<ActiveWorksheet>],
    pub index: &'a CellIndex,
    pub metrics: &'a GridMetrics,
    pub zoom: f64,
    pub rows: &'a [u32],
    pub cols: &'a [u32],
    pub has_formula: &'a dyn Fn(u32, u32) -> bool,
}

fn text_width_of(
    input: &BuildInput,
    measure: &mut dyn TextMeasure,
    view: &CellView,
    text: &str,
) -> f64 {
    if let Some(rich) = &view.rich {
        return rich
            .iter()
            .map(|run| measure.measure(&run.text, &css_font(&run.font, input.zoom)))
            .sum();
    }
    let font = css_font(&view.font, input.zoom);
    let mut widest = 0.0f64;
    for line in text.split('\n') {
        widest = widest.max(measure.measure(line, &font));
    }
    widest
}

fn make_item(
    input: &BuildInput,
    measure: &mut dyn TextMeasure,
    row: u32,
    col: u32,
    view: CellView,
    merge: Option<&CellRange>,
) -> CellItem {
    let metrics = input.metrics;
    let x = metrics.col_left(col);
    let y = metrics.row_top(row);
    let w = if let Some(m) = merge {
        metrics.col_left(m.end.col + 1) - x
    } else {
        metrics.col_width(col)
    };
    let h = if let Some(m) = merge {
        metrics.row_top(m.end.row + 1) - y
    } else {
        metrics.row_height(row)
    };
    let text = view.text.clone();
    let is_error = view.is_error;
    let mut item = CellItem {
        key: format!("{}:{}", row, col),
        row,
        col,
        x,
        y,
        w,
        h,
        view,
        text,
        text_left: 0,
        text_width: w,
        overflowing: false,
        font_scale: 1.0,
        merged: merge.is_some(),
        error_mark: is_error && (input.has_formula)(row, col),
    };
    if item.view.text.is_empty()
        || item.view.wrap
        || item.view.vertical_text
        || item.view.rotation != 0.0
    {
        return item;
    }
    let inner = (w - 2 * PADDING) as f64 - item.view.indent_px * (input.zoom / 100.0);
    let measured = text_width_of(input, measure, &item.view, &item.view.text);
    if item.view.h_align == "fill" {
        item.text = repeat_to_fill(&item.view.text, measured, inner);
        return item;
    }
    if item.view.shrink {
        if measured > inner && measured > 0.0 {
            item.font_scale = (inner / measured).max(0.1);
        }
        return item;
    }
    if item.view.is_number && measured > inner && item.view.rich.is_none() {
        let font = css_font(&item.view.font, input.zoom);
        item.text = hashes(inner, measure.measure("#", &font));
        return item;
    }
    if item.view.overflow && merge.is_none() && measured > inner {
        let extent = overflow_extent(
            input.sheet,
            input.index,
            metrics,
            row,
            col,
            measured + 2.0 * PADDING as f64,
            &item.view.h_align,
            256,
        );
        item.text_left = extent.left - x;
        item.text_width = extent.right - extent.left;
        item.overflowing = extent.start_col != col || extent.end_col != col;
    }
    item
}

fn merges_in(sheet: &ActiveWorksheet, rows: &[u32], cols: &[u32]) -> Vec<CellRange> {
    if sheet.merges.is_empty() || rows.is_empty() || cols.is_empty() {
        return Vec::new();
    }
    let r0 = rows[0];
    let r1 = rows[rows.len() - 1];
    let c0 = cols[0];
    let c1 = cols[cols.len() - 1];
    sheet
        .merges
        .iter()
        .filter(|m| m.start.row <= r1 && m.end.row >= r0 && m.start.col <= c1 && m.end.col >= c0)
        .cloned()
        .collect()
}

pub fn build_items(input: &BuildInput, measure: &mut dyn TextMeasure) -> Vec<CellItem> {
    let rows = input.rows;
    let cols = input.cols;
    if rows.is_empty() || cols.is_empty() {
        return Vec::new();
    }
    let merges = merges_in(input.sheet, rows, cols);
    let covered = |row: u32, col: u32| {
        merges.iter().any(|m| {
            row >= m.start.row && row <= m.end.row && col >= m.start.col && col <= m.end.col
        })
    };
    let mut out = Vec::new();
    for merge in &merges {
        let view = cell_view(
            input.workbook,
            input.sheet,
            input.index,
            merge.start.row,
            merge.start.col,
            cell_opts(input),
        );
        out.push(make_item(
            input,
            measure,
            merge.start.row,
            merge.start.col,
            view,
            Some(merge),
        ));
    }
    let first_col = cols[0];
    let last_col = cols[cols.len() - 1];
    for &row in rows {
        for &col in cols {
            if !merges.is_empty() && covered(row, col) {
                continue;
            }
            let view = cell_view(
                input.workbook,
                input.sheet,
                input.index,
                row,
                col,
                cell_opts(input),
            );
            out.push(make_item(input, measure, row, col, view, None));
        }
        if let Some(stored) = input.index.row_cols(row) {
            for &col in stored.keys() {
                if col >= first_col && col <= last_col {
                    continue;
                }
                let cell = input.index.get(input.sheet, row, col);
                let is_string = matches!(
                    cell.map(|c| &c.value),
                    Some(crate::model::DumpCellValue::String(s)) if !s.is_empty()
                );
                if !is_string {
                    continue;
                }
                let dist = if col < first_col {
                    first_col - col
                } else {
                    col - last_col
                };
                if dist > 64 {
                    continue;
                }
                let view = cell_view(
                    input.workbook,
                    input.sheet,
                    input.index,
                    row,
                    col,
                    cell_opts(input),
                );
                if !view.overflow {
                    continue;
                }
                let item = make_item(input, measure, row, col, view, None);
                let left = item.x + item.text_left;
                let right = left + item.text_width;
                let view_left = input.metrics.col_left(first_col);
                let view_right = input.metrics.col_left(last_col + 1);
                if item.overflowing && right > view_left && left < view_right {
                    out.push(item);
                }
            }
        }
    }
    out
}
