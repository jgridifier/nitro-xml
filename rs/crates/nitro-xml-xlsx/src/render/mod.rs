// SPDX-License-Identifier: MIT
//! Xlsx painters: grid (X1–X6, #18) and chart adapter (#28).

mod cells;
mod cond_format;
mod grid;
mod gridlines;
mod headers;
mod selection;

pub mod chart;

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::intern::StringIntern;
use crate::model::{ActiveWorksheet, WorkbookSlice};
use crate::view::metrics::{create_grid_metrics, GridMetricsOptions};

pub struct GridPaintViewport {
    pub scroll_x: i32,
    pub scroll_y: i32,
    pub width: i32,
    pub height: i32,
    pub frozen_rows: u32,
    pub frozen_cols: u32,
    /// Zoom percent \[10, 400\]; `None` uses the sheet's stored zoom (#29 shell).
    pub zoom_pct: Option<f64>,
}

/// Build one frame's command buffer for the cell grid (X1–X6).
#[allow(clippy::too_many_arguments)]
pub fn paint_grid_frame<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
    sheet_index: usize,
    all_sheets: &[Option<ActiveWorksheet>],
    viewport: &GridPaintViewport,
) {
    let metrics = create_grid_metrics(
        sheet,
        GridMetricsOptions {
            zoom: viewport.zoom_pct,
            ..GridMetricsOptions::default()
        },
    );
    buf.begin();
    grid::paint_frame(buf, intern, viewport, &metrics);
    gridlines::paint(buf, sheet, &metrics, viewport);
    headers::paint(buf, sheet, &metrics, viewport);
    cells::paint(
        buf,
        intern,
        workbook,
        sheet,
        sheet_index,
        all_sheets,
        &metrics,
        viewport,
    );
    cond_format::paint(buf);
    selection::paint(buf, viewport);
}

/// Convenience: allocate a fresh buffer (tests / dumps).
pub fn paint_grid_frame_vec<I: StringIntern>(
    intern: &mut I,
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
    sheet_index: usize,
    all_sheets: &[Option<ActiveWorksheet>],
    viewport: &GridPaintViewport,
) -> Vec<u8> {
    let mut buf = CommandBuffer::new();
    paint_grid_frame(
        &mut buf,
        intern,
        workbook,
        sheet,
        sheet_index,
        all_sheets,
        viewport,
    );
    buf.take()
}
