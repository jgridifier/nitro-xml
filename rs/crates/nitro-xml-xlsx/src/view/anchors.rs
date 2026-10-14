// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/anchors.ts; changes listed in
// PROVENANCE.md.
//! Drawing anchor → plane pixel box at the metrics' zoom.

use nitro_xml_opc::drawingml::EMU_PER_PIXEL;

use crate::model::DrawingAnchor;
use crate::view::metrics::GridMetrics;

/// Default one-cell extent when `ext` is missing (200px at 100% → 1_905_000 EMU).
pub const DEFAULT_ANCHOR_EXTENT_EMU: i64 = 1_905_000;
/// Smallest edge of a two-cell box so a collapsed anchor stays selectable.
pub const MIN_TWO_CELL_PIXELS: i32 = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelBox {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

fn marker_point(
    metrics: &GridMetrics,
    row: u32,
    col: u32,
    row_off: i64,
    col_off: i64,
    f: f64,
) -> (f64, f64) {
    (
        metrics.col_left(col) as f64 + (col_off as f64 / EMU_PER_PIXEL as f64) * f,
        metrics.row_top(row) as f64 + (row_off as f64 / EMU_PER_PIXEL as f64) * f,
    )
}

/// Plane rectangle of an anchor (origin: top-left of A1) at `metrics.zoom`.
pub fn anchor_to_pixel_box(anchor: &DrawingAnchor, metrics: &GridMetrics) -> PixelBox {
    let f = metrics.zoom / 100.0;
    let (fx, fy) = marker_point(
        metrics,
        anchor.from.row,
        anchor.from.col,
        anchor.from.row_offset,
        anchor.from.col_offset,
        f,
    );
    if let Some(to) = &anchor.to {
        let (tx, ty) = marker_point(metrics, to.row, to.col, to.row_offset, to.col_offset, f);
        PixelBox {
            x: fx.round() as i32,
            y: fy.round() as i32,
            w: ((tx - fx).round() as i32).max(MIN_TWO_CELL_PIXELS),
            h: ((ty - fy).round() as i32).max(MIN_TWO_CELL_PIXELS),
        }
    } else {
        let (cx, cy) = anchor
            .ext
            .unwrap_or((DEFAULT_ANCHOR_EXTENT_EMU, DEFAULT_ANCHOR_EXTENT_EMU));
        PixelBox {
            x: fx.round() as i32,
            y: fy.round() as i32,
            w: ((cx as f64 / EMU_PER_PIXEL as f64) * f).round().max(1.0) as i32,
            h: ((cy as f64 / EMU_PER_PIXEL as f64) * f).round().max(1.0) as i32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ActiveWorksheet;
    use crate::model::DrawingMarker;
    use crate::sheet_props::default_sheet_view;
    use crate::view::metrics::{create_grid_metrics, GridMetricsOptions};
    use std::collections::BTreeMap;

    fn sheet() -> ActiveWorksheet {
        ActiveWorksheet {
            name: "S".into(),
            sheet_id: 1,
            state: crate::model::SheetState::Visible,
            part_name: None,
            default_row_height: 15.0,
            default_col_width: None,
            format: None,
            view: default_sheet_view(),
            columns: Vec::new(),
            merges: Vec::new(),
            row_info: BTreeMap::new(),
            cells: Vec::new(),
            data_validations: Vec::new(),
            comment_addresses: Vec::new(),
            hyperlinks: Vec::new(),
            ext_lst: Vec::new(),
            sparkline_groups: Vec::new(),
            drawings: Vec::new(),
            conditional_formats: Vec::new(),
            dimension: None,
        }
    }

    #[test]
    fn catches_anchor_box_ignoring_zoom_scale() {
        // Catches: drawing anchors ignoring metrics.zoom so placeholders miss TS pixel boxes.
        let metrics = create_grid_metrics(&sheet(), GridMetricsOptions::default());
        let box_ = anchor_to_pixel_box(
            &DrawingAnchor {
                from: DrawingMarker {
                    row: 0,
                    col: 1,
                    row_offset: 0,
                    col_offset: 0,
                },
                to: None,
                ext: Some((100 * EMU_PER_PIXEL, 50 * EMU_PER_PIXEL)),
            },
            &metrics,
        );
        assert_eq!(box_.x, metrics.col_left(1));
        assert_eq!(box_.y, 0);
        assert_eq!(box_.w, 100);
        assert_eq!(box_.h, 50);
    }
}
