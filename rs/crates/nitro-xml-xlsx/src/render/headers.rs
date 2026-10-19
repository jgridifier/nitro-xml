// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/headers.ts and pane-chrome.ts;
// changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::address::column_label;
use crate::intern::StringIntern;
use crate::model::ActiveWorksheet;
use crate::render::geometry::header_sizes;
use crate::render::grid::ROOT_ID;
use crate::render::selection_ops::{col_selected, paint_selection, row_selected};
use crate::render::viewport::{paint_indices, visible_cells_for_host};
use crate::render::GridPaintViewport;
use crate::view::metrics::GridMetrics;

const CORNER_ID: u32 = 2;
const HBOX_COL_ID: u32 = 2_100;
const HDR_COL_ID: u32 = 2_101;
const HBOX_ROW_ID: u32 = 2_102;
const HDR_ROW_ID: u32 = 2_103;
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

fn set_attr<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    id: u32,
    key: &str,
    val: &str,
) {
    let k = intern.intern(key);
    let v = intern.intern(val);
    buf.set_attr(id, k, v);
}

fn place<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    id: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    set_attr(
        buf,
        intern,
        id,
        "style",
        &format!("left:{x}px;top:{y}px;width:{w}px;height:{h}px"),
    );
}

fn header_font_px(zoom: f64) -> i32 {
    (11.0 * (zoom / 100.0)).round().max(6.0) as i32
}

fn apply_header_sel(state: &str, class: &mut String) {
    if state == "full" {
        class.push_str(" xg-hd-full");
    } else if state == "part" {
        class.push_str(" xg-hd-sel");
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
    let selection = paint_selection(sheet);
    let cells_w = (viewport.width - sizes.w).max(0);
    let cells_h = (viewport.height - sizes.h).max(0);

    // pane-chrome.ts: corner + xg-hbox / xg-hdr-plane wrappers (scroll panes; freeze TODO).
    buf.create_element(CORNER_ID, div, ROOT_ID);
    set_attr(buf, intern, CORNER_ID, "class", "xg-corner");
    place(buf, intern, CORNER_ID, 0, 0, sizes.w, sizes.h);

    buf.create_element(HBOX_COL_ID, div, ROOT_ID);
    set_attr(buf, intern, HBOX_COL_ID, "class", "xg-hbox xg-hbox-col");
    place(buf, intern, HBOX_COL_ID, sizes.w, 0, cells_w, sizes.h);
    buf.create_element(HDR_COL_ID, div, HBOX_COL_ID);
    set_attr(buf, intern, HDR_COL_ID, "class", "xg-hdr-plane xg-hdr-col");
    set_style(buf, intern, HDR_COL_ID, "height", &format!("{}px", sizes.h));
    set_style(
        buf,
        intern,
        HDR_COL_ID,
        "transform",
        &format!("translate3d({}px,0,0)", -scroll_x),
    );

    buf.create_element(HBOX_ROW_ID, div, ROOT_ID);
    set_attr(buf, intern, HBOX_ROW_ID, "class", "xg-hbox xg-hbox-row");
    place(buf, intern, HBOX_ROW_ID, 0, sizes.h, sizes.w, cells_h);
    buf.create_element(HDR_ROW_ID, div, HBOX_ROW_ID);
    set_attr(buf, intern, HDR_ROW_ID, "class", "xg-hdr-plane xg-hdr-row");
    set_style(buf, intern, HDR_ROW_ID, "width", &format!("{}px", sizes.w));
    set_style(
        buf,
        intern,
        HDR_ROW_ID,
        "transform",
        &format!("translate3d(0,{}px,0)", -scroll_y),
    );

    for (n, col) in cols.iter().enumerate() {
        let id = COL_HDR_BASE + n as u32;
        let left = metrics.col_left(*col);
        let w = metrics.col_width(*col);
        let screen_left = sizes.w + left - scroll_x;
        if screen_left + w < sizes.w || screen_left > viewport.width {
            continue;
        }
        buf.create_element(id, div, HDR_COL_ID);
        let mut hdr_class = String::from("xg-hd");
        apply_header_sel(col_selected(&selection, *col), &mut hdr_class);
        set_attr(buf, intern, id, "class", &hdr_class);
        set_attr(buf, intern, id, "role", "columnheader");
        set_attr(buf, intern, id, "data-index", &col.to_string());
        set_style(buf, intern, id, "left", &format!("{left}px"));
        set_style(buf, intern, id, "width", &format!("{w}px"));
        set_style(buf, intern, id, "fontSize", &format!("{font_px}px"));
        // Keep unitless 1.35 when font-size is set (matches upstream inherited shell LH).
        set_style(buf, intern, id, "lineHeight", "1.35");
        let label = intern.intern(&column_label(*col));
        buf.set_text(id, label);
    }

    for (n, row) in rows.iter().enumerate() {
        let id = ROW_HDR_BASE + n as u32;
        let top = metrics.row_top(*row);
        let h = metrics.row_height(*row);
        let screen_top = sizes.h + top - scroll_y;
        if screen_top + h < sizes.h || screen_top > viewport.height {
            continue;
        }
        buf.create_element(id, div, HDR_ROW_ID);
        let mut hdr_class = String::from("xg-hd");
        apply_header_sel(row_selected(&selection, *row), &mut hdr_class);
        set_attr(buf, intern, id, "class", &hdr_class);
        set_attr(buf, intern, id, "role", "rowheader");
        set_attr(buf, intern, id, "data-index", &row.to_string());
        set_style(buf, intern, id, "top", &format!("{top}px"));
        set_style(buf, intern, id, "height", &format!("{h}px"));
        set_style(buf, intern, id, "fontSize", &format!("{font_px}px"));
        set_style(buf, intern, id, "lineHeight", "1.35");
        let label = intern.intern(&((*row + 1).to_string()));
        buf.set_text(id, label);
    }
}
