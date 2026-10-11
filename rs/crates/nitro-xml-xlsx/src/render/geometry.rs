// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/ui/grid/geometry.ts (headerW/H);
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
