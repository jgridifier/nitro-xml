// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/viewport.ts (visibleCells);
// changes listed in PROVENANCE.md.

use crate::address::{MAX_COL, MAX_ROW};
use crate::render::GridPaintViewport;
use crate::view::metrics::GridMetrics;

#[derive(Debug, Clone, Default)]
pub struct VisibleCells {
    pub rows: Vec<u32>,
    pub cols: Vec<u32>,
    pub frozen_rows: Vec<u32>,
    pub frozen_cols: Vec<u32>,
}

fn frozen_extent(count: u32, start: impl Fn(u32) -> i32) -> i32 {
    if count > 0 {
        start(count)
    } else {
        0
    }
}

fn collect(
    from: i32,
    to: i32,
    limit: u32,
    at: impl Fn(i32) -> u32,
    start: impl Fn(u32) -> i32,
    size: impl Fn(u32) -> i32,
    min_index: u32,
) -> Vec<u32> {
    let mut out = Vec::new();
    if to <= from {
        return out;
    }
    let mut i = at(from).max(min_index);
    while i <= limit && start(i) < to {
        if size(i) > 0 {
            out.push(i);
        }
        i += 1;
    }
    out
}

fn frozen_list(count: u32, size: impl Fn(u32) -> i32, limit: u32) -> Vec<u32> {
    let mut out = Vec::new();
    for i in 0..count.min(limit + 1) {
        if size(i) > 0 {
            out.push(i);
        }
    }
    out
}

/// Visible row/column indices for painting (matches upstream `cellsW + 40` / `cellsH + 30` slop).
pub fn visible_cells_for_host(
    metrics: &GridMetrics,
    viewport: &GridPaintViewport,
    header_w: i32,
    header_h: i32,
) -> VisibleCells {
    let vp = GridPaintViewport {
        scroll_x: viewport.scroll_x,
        scroll_y: viewport.scroll_y,
        width: (viewport.width - header_w).max(0) + 40,
        height: (viewport.height - header_h).max(0) + 30,
        frozen_rows: viewport.frozen_rows,
        frozen_cols: viewport.frozen_cols,
        zoom_pct: viewport.zoom_pct,
    };
    visible_cells(metrics, &vp)
}

fn visible_cells(metrics: &GridMetrics, viewport: &GridPaintViewport) -> VisibleCells {
    let f_rows = viewport.frozen_rows;
    let f_cols = viewport.frozen_cols;
    let frozen_h = frozen_extent(f_rows, |n| metrics.row_top(n));
    let frozen_w = frozen_extent(f_cols, |n| metrics.col_left(n));
    let top = frozen_h + viewport.scroll_y.max(0);
    let left = frozen_w + viewport.scroll_x.max(0);
    let rows = collect(
        top,
        top + (viewport.height - frozen_h).max(0),
        MAX_ROW,
        |o| metrics.row_at(o),
        |r| metrics.row_top(r),
        |r| metrics.row_height(r),
        f_rows,
    );
    let cols = collect(
        left,
        left + (viewport.width - frozen_w).max(0),
        MAX_COL,
        |o| metrics.col_at(o),
        |c| metrics.col_left(c),
        |c| metrics.col_width(c),
        f_cols,
    );
    VisibleCells {
        rows,
        cols,
        frozen_rows: frozen_list(f_rows, |r| metrics.row_height(r), MAX_ROW),
        frozen_cols: frozen_list(f_cols, |c| metrics.col_width(c), MAX_COL),
    }
}

/// Rows and columns to paint in the scrolling area (frozen indices included once).
pub fn paint_indices(vis: &VisibleCells) -> (Vec<u32>, Vec<u32>) {
    let mut rows = vis.frozen_rows.clone();
    for r in &vis.rows {
        if !rows.contains(r) {
            rows.push(*r);
        }
    }
    let mut cols = vis.frozen_cols.clone();
    for c in &vis.cols {
        if !cols.contains(c) {
            cols.push(*c);
        }
    }
    rows.sort_unstable();
    cols.sort_unstable();
    (rows, cols)
}
