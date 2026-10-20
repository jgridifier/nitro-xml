// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/ui/grid/cell-items.ts; changes listed in PROVENANCE.md.

use std::collections::HashSet;

use crate::cf::CfPaintCache;
use crate::model::{ActiveWorksheet, CellRange, WorkbookSlice};
use crate::sheet_access::CellIndex;
use crate::view::cell_paint::{hashes, repeat_to_fill, FontCssMemo};
use crate::view::cell_view::{
    cell_view_with_cf_cache, is_blank_view, CellViewOptions, StylePaintCache,
};
use crate::view::metrics::GridMetrics;
use crate::view::number_fit::fit_number;
use crate::view::sheet_queries::overflow_extent;
use crate::view::text_measure::TextMeasure;
use crate::view::types::{CellItem, CellView};

const PADDING: i32 = 2;

fn cell_opts<'a>(
    input: &'a BuildInput<'a>,
    not_in_merge: bool,
    comment_at: Option<&'a dyn Fn(u32, u32) -> bool>,
    style_cache: Option<&'a mut StylePaintCache>,
) -> CellViewOptions<'a> {
    CellViewOptions {
        sheet_index: Some(input.sheet_index),
        all_sheets: Some(input.all_sheets),
        not_in_merge,
        comment_at,
        style_cache,
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
    /// Live grid only (upstream `grid-view.ts`); view-IR dumps omit this so FakeTextMeasure
    /// stays differential-clean against the TS oracle dump path.
    pub enable_number_fit: bool,
    /// Live painter only: drop blank views before `make_item` (upstream CellViewCache
    /// returns null). View-IR dumps keep blanks for the oracle (#30).
    pub skip_blank_items: bool,
    /// View-IR dumps need `key`; live paint never reads it (#30).
    pub include_item_keys: bool,
}

fn text_width_of(
    input: &BuildInput,
    measure: &mut dyn TextMeasure,
    font_memo: &mut FontCssMemo,
    view: &CellView,
    text: &str,
) -> f64 {
    if let Some(rich) = &view.rich {
        return rich
            .iter()
            .map(|run| {
                let font = font_memo.get_or_compute(&run.font, input.zoom);
                measure.measure(&run.text, font)
            })
            .sum();
    }
    let font = font_memo.get_or_compute(&view.font, input.zoom);
    let mut widest = 0.0f64;
    for line in text.split('\n') {
        widest = widest.max(measure.measure(line, font));
    }
    widest
}

fn make_item(
    input: &BuildInput,
    measure: &mut dyn TextMeasure,
    font_memo: &mut FontCssMemo,
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
        key: if input.include_item_keys {
            format!("{}:{}", row, col)
        } else {
            String::new()
        },
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
    let measured = text_width_of(input, measure, font_memo, &item.view, &item.view.text);
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
        let font = font_memo.get_or_compute(&item.view.font, input.zoom);
        // Catches: General numbers that still fit after Excel-style decimal drop must not
        // become `###` (poi-WithChartSheet D8/D10/D12 live paint vs TS).
        if input.enable_number_fit {
            let measure_ptr: *mut dyn TextMeasure = measure;
            let measure_fn = |text: &str| unsafe { (*measure_ptr).measure(text, font) };
            item.text = fit_number(
                input.workbook,
                input.sheet,
                input.index,
                row,
                col,
                inner,
                &measure_fn,
            )
            .unwrap_or_else(|| hashes(inner, measure.measure("#", font)));
        } else {
            item.text = hashes(inner, measure.measure("#", font));
        }
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
    // One CF range-scan cache for the whole frame (color-scale / data-bar / …).
    let mut cf_cache = CfPaintCache::default();
    // Style→font/fill/borders once per style_id (cells_1k shares style 0; #30).
    let mut style_cache = StylePaintCache::default();
    let comment_set: Option<HashSet<(u32, u32)>> = if input.sheet.comment_addresses.is_empty() {
        None
    } else {
        Some(
            input
                .sheet
                .comment_addresses
                .iter()
                .map(|a| (a.row, a.col))
                .collect(),
        )
    };
    let comment_lookup = |r: u32, c: u32| -> bool {
        comment_set
            .as_ref()
            .is_some_and(|set| set.contains(&(r, c)))
    };
    let comment_at: Option<&dyn Fn(u32, u32) -> bool> = if comment_set.is_some() {
        Some(&comment_lookup)
    } else {
        None
    };
    let mut font_memo = FontCssMemo::new();
    let mut out = Vec::with_capacity(rows.len() * cols.len() + merges.len());
    for merge in &merges {
        let view = cell_view_with_cf_cache(
            input.workbook,
            input.sheet,
            input.index,
            merge.start.row,
            merge.start.col,
            cell_opts(input, false, comment_at, Some(&mut style_cache)),
            Some(&mut cf_cache),
        );
        out.push(make_item(
            input,
            measure,
            &mut font_memo,
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
            let view = cell_view_with_cf_cache(
                input.workbook,
                input.sheet,
                input.index,
                row,
                col,
                // Covered cells skipped above — skip merges.iter().any per cell (#30).
                cell_opts(input, true, comment_at, Some(&mut style_cache)),
                Some(&mut cf_cache),
            );
            // Keep formula blanks — paint marks them `xg-stale` after edits (#30).
            if input.skip_blank_items
                && is_blank_view(&view)
                && !(input.has_formula)(row, col)
            {
                continue;
            }
            out.push(make_item(
                input,
                measure,
                &mut font_memo,
                row,
                col,
                view,
                None,
            ));
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
                let view = cell_view_with_cf_cache(
                    input.workbook,
                    input.sheet,
                    input.index,
                    row,
                    col,
                    cell_opts(input, false, comment_at, Some(&mut style_cache)),
                    Some(&mut cf_cache),
                );
                if !view.overflow {
                    continue;
                }
                let item = make_item(input, measure, &mut font_memo, row, col, view, None);
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
