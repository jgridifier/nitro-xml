// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/gridlines.ts; changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::intern::StringIntern;
use crate::model::ActiveWorksheet;
use crate::render::geometry::header_sizes;
use crate::render::grid::{LINES_ID, LINES_TOP_ID};
use crate::render::paint_encode::{place_style_attr, ChromeKeys};
use crate::render::viewport::visible_cells_for_host;
use crate::render::GridPaintViewport;
use crate::view::metrics::GridMetrics;

const LINE_BASE: u32 = 5_000;

pub fn paint<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    sheet: &ActiveWorksheet,
    metrics: &GridMetrics,
    viewport: &GridPaintViewport,
) {
    if !sheet.view.show_grid_lines {
        return;
    }
    let sizes = header_sizes(sheet, metrics, metrics.last_row);
    let vis = visible_cells_for_host(metrics, viewport, sizes.w, sizes.h);
    let div = intern.intern("div");
    let keys = ChromeKeys::new(intern);
    let class_v = intern.intern("xg-gl xg-gl-v");
    let class_h = intern.intern("xg-gl xg-gl-h");
    let mut i = 0u32;

    // Plane-relative coords (upstream gridlines.ts); scroll is on `.xg-plane`.
    // Paint per-quadrant row/col sets like grid-view.ts (main / top / left / corner).
    let mut line = |parent: u32, x: i32, y: i32, w: i32, h: i32| {
        let id = LINE_BASE + i;
        i += 1;
        buf.create_element(id, div, parent);
        buf.set_attr(id, keys.class, if w == 1 { class_v } else { class_h });
        // One style attr instead of four SetStyle ops (smaller first-frame buffer).
        place_style_attr(buf, intern, &keys, id, x, y, w, h);
    };

    let mut paint_set = |parent: u32, rows: &[u32], cols: &[u32]| {
        if rows.is_empty() || cols.is_empty() {
            return;
        }
        let first = rows[0];
        let last = rows[rows.len() - 1];
        let left = metrics.col_left(cols[0]);
        let right = metrics.col_left(cols[cols.len() - 1] + 1);
        let top = metrics.row_top(first);
        let bottom = metrics.row_top(last + 1);
        for col in cols {
            line(parent, metrics.col_left(col + 1) - 1, top, 1, bottom - top);
        }
        for row in rows {
            // Bottom edge of the frozen block is `.xg-freeze-h`, not a gridline (pane-chrome.ts).
            if viewport.frozen_rows > 0 && row + 1 == viewport.frozen_rows {
                continue;
            }
            line(parent, left, metrics.row_top(row + 1) - 1, right - left, 1);
        }
    };

    // Top quadrant: frozen rows; main quadrant: scrolling rows (grid-view.ts sets).
    if viewport.frozen_rows > 0 {
        paint_set(LINES_TOP_ID, &vis.frozen_rows, &vis.cols);
        paint_set(LINES_TOP_ID, &vis.frozen_rows, &vis.frozen_cols);
    }
    paint_set(LINES_ID, &vis.rows, &vis.cols);
    paint_set(LINES_ID, &vis.rows, &vis.frozen_cols);
}
