// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/ui/grid/geometry.ts
// (headerW/H, colAtX, rowAtY, hit, clampedHit);
// changes listed in PROVENANCE.md.

use crate::model::ActiveWorksheet;
use crate::view::metrics::GridMetrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderSizes {
    pub w: i32,
    pub h: i32,
}

/// Excel row-header width for the largest row number in view.
pub fn row_header_width(extent_row: u32, zoom: f64) -> i32 {
    let digits = (extent_row + 1).to_string().len();
    ((digits.max(2) * 7 + 12) as f64 * (zoom / 100.0)).round() as i32
}

pub fn column_header_height(zoom: f64) -> i32 {
    (20.0 * (zoom / 100.0)).round() as i32
}

pub fn header_sizes(
    sheet: &ActiveWorksheet,
    metrics: &GridMetrics,
    extent_row: u32,
) -> HeaderSizes {
    if !sheet.view.show_headers {
        return HeaderSizes { w: 0, h: 0 };
    }
    HeaderSizes {
        w: row_header_width(extent_row, metrics.zoom),
        h: column_header_height(metrics.zoom),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitArea {
    Cell,
    ColHeader,
    RowHeader,
    Corner,
    Outside,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub area: HitArea,
    pub row: i32,
    pub col: i32,
}

/// View-space geometry for hit testing (headers included, origin at the grid's top-left).
pub struct HitInput<'a> {
    pub metrics: &'a GridMetrics,
    pub header_w: i32,
    pub header_h: i32,
    pub width: i32,
    pub height: i32,
    pub scroll_x: i32,
    pub scroll_y: i32,
    pub frozen_rows: u32,
    pub frozen_cols: u32,
}

impl HitInput<'_> {
    fn frozen_w(&self) -> i32 {
        if self.frozen_cols > 0 {
            self.metrics.col_left(self.frozen_cols)
        } else {
            0
        }
    }

    fn frozen_h(&self) -> i32 {
        if self.frozen_rows > 0 {
            self.metrics.row_top(self.frozen_rows)
        } else {
            0
        }
    }
}

/// Column under a view x (clamped into the cell area). Upstream `colAtX`.
pub fn col_at_x(g: &HitInput<'_>, x: i32) -> u32 {
    let cx = (x - g.header_w).max(0);
    let scroll = g.scroll_x.max(0);
    if cx < g.frozen_w() {
        g.metrics.col_at(cx)
    } else {
        g.metrics.col_at(cx + scroll)
    }
}

/// Row under a view y (clamped into the cell area). Upstream `rowAtY`.
pub fn row_at_y(g: &HitInput<'_>, y: i32) -> u32 {
    let cy = (y - g.header_h).max(0);
    let scroll = g.scroll_y.max(0);
    if cy < g.frozen_h() {
        g.metrics.row_at(cy)
    } else {
        g.metrics.row_at(cy + scroll)
    }
}

/// Hit test in view coordinates. Upstream `GridGeometry.hit`.
pub fn hit(g: &HitInput<'_>, x: i32, y: i32) -> Hit {
    if x < 0 || y < 0 || x > g.width || y > g.height {
        return Hit {
            area: HitArea::Outside,
            row: -1,
            col: -1,
        };
    }
    let in_header_x = x < g.header_w;
    let in_header_y = y < g.header_h;
    if in_header_x && in_header_y {
        return Hit {
            area: HitArea::Corner,
            row: -1,
            col: -1,
        };
    }
    if in_header_y {
        return Hit {
            area: HitArea::ColHeader,
            row: -1,
            col: col_at_x(g, x) as i32,
        };
    }
    if in_header_x {
        return Hit {
            area: HitArea::RowHeader,
            row: row_at_y(g, y) as i32,
            col: -1,
        };
    }
    Hit {
        area: HitArea::Cell,
        row: row_at_y(g, y) as i32,
        col: col_at_x(g, x) as i32,
    }
}

/// Drag hit clamped into the cell area. Upstream `clampedHit`.
pub fn clamped_hit(g: &HitInput<'_>, x: i32, y: i32) -> (u32, u32) {
    let cx = x.max(g.header_w + 1).min(g.width - 2);
    let cy = y.max(g.header_h + 1).min(g.height - 2);
    (row_at_y(g, cy), col_at_x(g, cx))
}
