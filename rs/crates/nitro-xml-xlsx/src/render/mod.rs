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

use crate::model::{ActiveWorksheet, WorkbookSlice};
use crate::view::metrics::{create_grid_metrics, GridMetricsOptions};

pub struct GridPaintViewport {
    pub scroll_x: i32,
    pub scroll_y: i32,
    pub width: i32,
    pub height: i32,
    pub frozen_rows: u32,
    pub frozen_cols: u32,
}

/// Build one frame's command buffer for the cell grid (X1–X6). Wasm handle wiring: TODO(#19).
pub fn paint_grid_frame(
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
    viewport: &GridPaintViewport,
) -> Vec<u8> {
    let metrics = create_grid_metrics(sheet, GridMetricsOptions::default());
    let mut buf = CommandBuffer::new();
    buf.begin();
    grid::paint_frame(&mut buf, viewport, &metrics);
    gridlines::paint(&mut buf, sheet, &metrics, viewport);
    headers::paint(&mut buf, sheet, &metrics, viewport);
    cells::paint(&mut buf, workbook, sheet, &metrics, viewport);
    cond_format::paint(&mut buf);
    selection::paint(&mut buf, viewport);
    buf.take()
}
