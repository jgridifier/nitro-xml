// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/gridlines.ts; changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::intern::StringIntern;
use crate::model::ActiveWorksheet;
use crate::render::geometry::header_sizes;
use crate::render::viewport::{paint_indices, visible_cells_for_host};
use crate::render::GridPaintViewport;
use crate::view::metrics::GridMetrics;

const LINE_BASE: u32 = 5_000;

fn set_style<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    id: u32,
    prop: &str,
    val: &str,
) {
    let p = intern.intern(prop);
    let v = intern.intern(val);
    buf.set_style(id, p, v);
}

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
    let (rows, cols) = paint_indices(&vis);
    if rows.is_empty() || cols.is_empty() {
        return;
    }
    let first = rows[0];
    let last = rows[rows.len() - 1];
    let left = metrics.col_left(cols[0]);
    let right = metrics.col_left(cols[cols.len() - 1] + 1);
    let top = metrics.row_top(first);
    let bottom = metrics.row_top(last + 1);
    let ox = sizes.w;
    let oy = sizes.h;
    let scroll_x = viewport.scroll_x;
    let scroll_y = viewport.scroll_y;
    let div = intern.intern("div");
    let mut i = 0u32;

    let mut line = |x: i32, y: i32, w: i32, h: i32| {
        let id = LINE_BASE + i;
        i += 1;
        buf.create_element(id, div, 1);
        set_style(buf, intern, id, "position", "absolute");
        set_style(buf, intern, id, "left", &format!("{}px", x));
        set_style(buf, intern, id, "top", &format!("{}px", y));
        set_style(buf, intern, id, "width", &format!("{}px", w));
        set_style(buf, intern, id, "height", &format!("{}px", h));
        set_style(buf, intern, id, "background", "#e1e1e1");
        set_style(buf, intern, id, "pointerEvents", "none");
    };

    for col in cols {
        let x = ox + metrics.col_left(col + 1) - 1 - scroll_x;
        line(x, oy + top - scroll_y, 1, bottom - top);
    }
    for row in rows {
        let y = oy + metrics.row_top(row + 1) - 1 - scroll_y;
        line(ox + left - scroll_x, y, right - left, 1);
    }
}
