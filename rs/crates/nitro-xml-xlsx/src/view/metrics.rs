// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/metrics.ts; changes listed in PROVENANCE.md.

use crate::address::MAX_COL;
use crate::address::MAX_ROW;
use crate::model::ActiveWorksheet;
use crate::sheet_access::used_range;
use crate::view::axis_metrics::AxisMetrics;
use crate::view::units::{
    column_width_to_pixels, default_column_pixels, points_to_pixels, DEFAULT_MAX_DIGIT_WIDTH,
};

#[derive(Debug, Clone)]
pub struct GridMetrics {
    pub zoom: f64,
    pub last_row: u32,
    pub last_col: u32,
    pub(crate) cols: AxisMetrics,
    pub(crate) rows: AxisMetrics,
}

#[derive(Default)]
pub struct GridMetricsOptions {
    pub zoom: Option<f64>,
    pub mdw: Option<f64>,
    pub extra_rows: Option<u32>,
    pub extra_cols: Option<u32>,
}

pub fn zoom_percent(zoom: Option<f64>) -> f64 {
    let z = zoom.unwrap_or(100.0);
    if !z.is_finite() || z <= 0.0 {
        return 100.0;
    }
    let percent = if z <= 4.0 { z * 100.0 } else { z };
    percent.clamp(10.0, 400.0)
}

fn column_overrides(sheet: &ActiveWorksheet, mdw: f64, default_px: i32) -> Vec<(u32, i32)> {
    let mut out = Vec::new();
    for info in &sheet.columns {
        let px = if info.hidden == Some(true) {
            0
        } else if let Some(w) = info.width {
            column_width_to_pixels(w, mdw)
        } else {
            default_px
        };
        if px == default_px {
            continue;
        }
        let last = info.max.min(MAX_COL);
        for c in info.min..=last {
            out.push((c, px));
        }
    }
    out
}

pub fn create_grid_metrics(sheet: &ActiveWorksheet, options: GridMetricsOptions) -> GridMetrics {
    let zoom = zoom_percent(options.zoom.or(Some(sheet.view.zoom)));
    let factor = zoom / 100.0;
    let mdw = options.mdw.unwrap_or(DEFAULT_MAX_DIGIT_WIDTH);
    let scale = |px: i32| -> i32 {
        if px > 0 {
            ((px as f64) * factor).round().max(1.0) as i32
        } else {
            0
        }
    };

    let default_col_px = default_column_pixels(sheet.default_col_width, mdw, 8.0);
    let col_overrides: Vec<(u32, i32)> = column_overrides(sheet, mdw, default_col_px)
        .into_iter()
        .map(|(c, px)| (c, scale(px)))
        .collect();
    let cols = AxisMetrics::new(scale(default_col_px), MAX_COL + 1, col_overrides);

    let default_row_px = points_to_pixels(sheet.default_row_height);
    let mut row_overrides = Vec::new();
    for (r, info) in &sheet.row_info {
        if info.hidden == Some(true) {
            row_overrides.push((*r, 0));
        } else if let Some(h) = info.height {
            row_overrides.push((*r, scale(points_to_pixels(h))));
        }
    }
    let rows = AxisMetrics::new(scale(default_row_px), MAX_ROW + 1, row_overrides);

    let used = used_range(sheet);
    let mut last_row_used = used.as_ref().map(|u| u.end.row).unwrap_or(0);
    for r in sheet.row_info.keys() {
        last_row_used = last_row_used.max(*r);
    }
    for m in &sheet.merges {
        last_row_used = last_row_used.max(m.end.row);
    }
    let mut last_col_used = used.as_ref().map(|u| u.end.col).unwrap_or(0);
    for m in &sheet.merges {
        last_col_used = last_col_used.max(m.end.col);
    }
    for info in &sheet.columns {
        if info.max < MAX_COL {
            last_col_used = last_col_used.max(info.max);
        }
    }
    let extra_rows = options.extra_rows.unwrap_or(50);
    let extra_cols = options.extra_cols.unwrap_or(20);
    let last_row = (last_row_used + extra_rows).min(MAX_ROW);
    let last_col = (last_col_used + extra_cols).min(MAX_COL);

    GridMetrics {
        zoom,
        last_row,
        last_col,
        cols,
        rows,
    }
}

impl GridMetrics {
    pub fn col_left(&self, c: u32) -> i32 {
        self.cols.start(c)
    }

    pub fn col_width(&self, c: u32) -> i32 {
        self.cols.size(c)
    }

    pub fn row_top(&self, r: u32) -> i32 {
        self.rows.start(r)
    }

    pub fn row_height(&self, r: u32) -> i32 {
        self.rows.size(r)
    }

    pub fn is_row_hidden(&self, r: u32) -> bool {
        self.rows.size(r) == 0
    }

    pub fn is_col_hidden(&self, c: u32) -> bool {
        self.cols.size(c) == 0
    }

    pub fn col_at(&self, offset: i32) -> u32 {
        self.cols.at(offset)
    }

    pub fn row_at(&self, offset: i32) -> u32 {
        self.rows.at(offset)
    }
}
