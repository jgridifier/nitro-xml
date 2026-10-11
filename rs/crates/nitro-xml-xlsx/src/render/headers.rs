// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/headers.ts and pane-chrome.ts;
// changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::address::column_label;
use crate::intern::StringIntern;
use crate::model::ActiveWorksheet;
use crate::render::geometry::header_sizes;
use crate::render::selection_ops::{col_selected, row_selected};
use crate::render::viewport::{paint_indices, visible_cells_for_host};
use crate::render::GridPaintViewport;
use crate::view::metrics::GridMetrics;

const CORNER_ID: u32 = 2;
const COL_HDR_BASE: u32 = 3_000;
const ROW_HDR_BASE: u32 = 4_000;

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

fn header_font_px(zoom: f64) -> i32 {
    (11.0 * (zoom / 100.0)).round().max(6.0) as i32
}

fn apply_header_sel<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    id: u32,
    state: &str,
    axis_col: bool,
) {
    if state == "none" {
        return;
    }
    if state == "full" {
        set_style(buf, intern, id, "background", "#cde6d7");
        set_style(buf, intern, id, "color", "#185c37");
    } else {
        set_style(buf, intern, id, "background", "#e1e1e1");
        set_style(buf, intern, id, "color", "#217346");
    }
    set_style(buf, intern, id, "fontWeight", "600");
    if axis_col {
        set_style(buf, intern, id, "boxShadow", "inset 0 -2px 0 #217346");
    } else {
        set_style(buf, intern, id, "boxShadow", "inset -2px 0 0 #217346");
    }
}

pub fn paint<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    sheet: &ActiveWorksheet,
    metrics: &GridMetrics,
    viewport: &GridPaintViewport,
) {
    let extent_row = metrics.last_row;
    let sizes = header_sizes(sheet, metrics, extent_row);
    if sizes.w <= 0 || sizes.h <= 0 {
        return;
    }
    let vis = visible_cells_for_host(metrics, viewport, sizes.w, sizes.h);
    let (rows, cols) = paint_indices(&vis);
    let div = intern.intern("div");
    let font_px = header_font_px(metrics.zoom);
    let scroll_x = viewport.scroll_x;
    let scroll_y = viewport.scroll_y;
    let selection = sheet.view.selection.as_ref();

    buf.create_element(CORNER_ID, div, 1);
    set_style(buf, intern, CORNER_ID, "position", "absolute");
    set_style(buf, intern, CORNER_ID, "left", "0");
    set_style(buf, intern, CORNER_ID, "top", "0");
    set_style(buf, intern, CORNER_ID, "width", &format!("{}px", sizes.w));
    set_style(buf, intern, CORNER_ID, "height", &format!("{}px", sizes.h));
    set_style(buf, intern, CORNER_ID, "boxSizing", "border-box");
    set_style(buf, intern, CORNER_ID, "background", "#f3f3f3");
    set_style(buf, intern, CORNER_ID, "borderRight", "1px solid #d4d4d4");
    set_style(buf, intern, CORNER_ID, "borderBottom", "1px solid #d4d4d4");
    set_style(buf, intern, CORNER_ID, "zIndex", "5");

    for (n, col) in cols.iter().enumerate() {
        let id = COL_HDR_BASE + n as u32;
        let left = sizes.w + metrics.col_left(*col) - scroll_x;
        let w = metrics.col_width(*col);
        if left + w < sizes.w || left > viewport.width {
            continue;
        }
        buf.create_element(id, div, 1);
        set_style(buf, intern, id, "position", "absolute");
        set_style(buf, intern, id, "left", &format!("{}px", left));
        set_style(buf, intern, id, "top", "0");
        set_style(buf, intern, id, "width", &format!("{}px", w));
        set_style(buf, intern, id, "height", &format!("{}px", sizes.h));
        set_style(buf, intern, id, "boxSizing", "border-box");
        set_style(buf, intern, id, "display", "flex");
        set_style(buf, intern, id, "alignItems", "center");
        set_style(buf, intern, id, "justifyContent", "center");
        set_style(buf, intern, id, "overflow", "hidden");
        set_style(buf, intern, id, "whiteSpace", "nowrap");
        set_style(buf, intern, id, "color", "#444");
        set_style(buf, intern, id, "background", "#f3f3f3");
        set_style(buf, intern, id, "borderRight", "1px solid #d4d4d4");
        set_style(buf, intern, id, "borderBottom", "1px solid #d4d4d4");
        set_style(buf, intern, id, "fontSize", &format!("{}px", font_px));
        set_style(buf, intern, id, "zIndex", "4");
        if let Some(sel) = selection {
            apply_header_sel(buf, intern, id, col_selected(sel, *col), true);
        }
        let label = intern.intern(&column_label(*col));
        buf.set_text(id, label);
    }

    for (n, row) in rows.iter().enumerate() {
        let id = ROW_HDR_BASE + n as u32;
        let top = sizes.h + metrics.row_top(*row) - scroll_y;
        let h = metrics.row_height(*row);
        if top + h < sizes.h || top > viewport.height {
            continue;
        }
        buf.create_element(id, div, 1);
        set_style(buf, intern, id, "position", "absolute");
        set_style(buf, intern, id, "left", "0");
        set_style(buf, intern, id, "top", &format!("{}px", top));
        set_style(buf, intern, id, "width", &format!("{}px", sizes.w));
        set_style(buf, intern, id, "height", &format!("{}px", h));
        set_style(buf, intern, id, "boxSizing", "border-box");
        set_style(buf, intern, id, "display", "flex");
        set_style(buf, intern, id, "alignItems", "center");
        set_style(buf, intern, id, "justifyContent", "center");
        set_style(buf, intern, id, "overflow", "hidden");
        set_style(buf, intern, id, "whiteSpace", "nowrap");
        set_style(buf, intern, id, "color", "#444");
        set_style(buf, intern, id, "background", "#f3f3f3");
        set_style(buf, intern, id, "borderRight", "1px solid #d4d4d4");
        set_style(buf, intern, id, "borderBottom", "1px solid #d4d4d4");
        set_style(buf, intern, id, "fontSize", &format!("{}px", font_px));
        set_style(buf, intern, id, "zIndex", "4");
        if let Some(sel) = selection {
            apply_header_sel(buf, intern, id, row_selected(sel, *row), false);
        }
        let label = intern.intern(&((*row + 1).to_string()));
        buf.set_text(id, label);
    }
}
