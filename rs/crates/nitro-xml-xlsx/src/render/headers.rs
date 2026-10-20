// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/headers.ts and pane-chrome.ts;
// changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::address::column_label;
use crate::intern::StringIntern;
use crate::model::ActiveWorksheet;
use crate::render::geometry::header_sizes;
use crate::render::grid::ROOT_ID;
use crate::render::paint_encode::{
    intern_px, intern_translate3d, place_style_attr, set_attr_v, ChromeKeys,
};
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

/// `left:Xpx;width:Wpx;font-size:Fpx;line-height:1.35` for column headers.
/// (CSS `style` attr needs kebab-case; SetStyle uses camelCase JS props.)
fn intern_col_hdr_style(intern: &mut impl StringIntern, left: i32, w: i32, font_px: i32) -> u32 {
    fn push_bytes(buf: &mut [u8], o: &mut usize, s: &[u8]) {
        buf[*o..*o + s.len()].copy_from_slice(s);
        *o += s.len();
    }
    fn push_i32(buf: &mut [u8], o: &mut usize, n: i32) {
        let mut tmp = [0u8; 16];
        let mut x = n.unsigned_abs();
        let neg = n < 0;
        let mut i = tmp.len();
        if x == 0 {
            i -= 1;
            tmp[i] = b'0';
        } else {
            while x > 0 {
                i -= 1;
                tmp[i] = b'0' + (x % 10) as u8;
                x /= 10;
            }
        }
        if neg {
            i -= 1;
            tmp[i] = b'-';
        }
        push_bytes(buf, o, &tmp[i..]);
    }
    let mut buf = [0u8; 96];
    let mut o = 0usize;
    push_bytes(&mut buf, &mut o, b"left:");
    push_i32(&mut buf, &mut o, left);
    push_bytes(&mut buf, &mut o, b"px;width:");
    push_i32(&mut buf, &mut o, w);
    push_bytes(&mut buf, &mut o, b"px;font-size:");
    push_i32(&mut buf, &mut o, font_px);
    push_bytes(&mut buf, &mut o, b"px;line-height:1.35");
    intern.intern(unsafe { std::str::from_utf8_unchecked(&buf[..o]) })
}

/// `top:Ypx;height:Hpx;font-size:Fpx;line-height:1.35` for row headers.
fn intern_row_hdr_style(intern: &mut impl StringIntern, top: i32, h: i32, font_px: i32) -> u32 {
    fn push_bytes(buf: &mut [u8], o: &mut usize, s: &[u8]) {
        buf[*o..*o + s.len()].copy_from_slice(s);
        *o += s.len();
    }
    fn push_i32(buf: &mut [u8], o: &mut usize, n: i32) {
        let mut tmp = [0u8; 16];
        let mut x = n.unsigned_abs();
        let neg = n < 0;
        let mut i = tmp.len();
        if x == 0 {
            i -= 1;
            tmp[i] = b'0';
        } else {
            while x > 0 {
                i -= 1;
                tmp[i] = b'0' + (x % 10) as u8;
                x /= 10;
            }
        }
        if neg {
            i -= 1;
            tmp[i] = b'-';
        }
        push_bytes(buf, o, &tmp[i..]);
    }
    let mut buf = [0u8; 96];
    let mut o = 0usize;
    push_bytes(&mut buf, &mut o, b"top:");
    push_i32(&mut buf, &mut o, top);
    push_bytes(&mut buf, &mut o, b"px;height:");
    push_i32(&mut buf, &mut o, h);
    push_bytes(&mut buf, &mut o, b"px;font-size:");
    push_i32(&mut buf, &mut o, font_px);
    push_bytes(&mut buf, &mut o, b"px;line-height:1.35");
    intern.intern(unsafe { std::str::from_utf8_unchecked(&buf[..o]) })
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
    let keys = ChromeKeys::new(intern);
    let font_px = header_font_px(metrics.zoom);
    let scroll_x = viewport.scroll_x;
    let scroll_y = viewport.scroll_y;
    let selection = paint_selection(sheet);
    let cells_w = (viewport.width - sizes.w).max(0);
    let cells_h = (viewport.height - sizes.h).max(0);

    let class_corner = intern.intern("xg-corner");
    let class_hbox_col = intern.intern("xg-hbox xg-hbox-col");
    let class_hdr_col = intern.intern("xg-hdr-plane xg-hdr-col");
    let class_hbox_row = intern.intern("xg-hbox xg-hbox-row");
    let class_hdr_row = intern.intern("xg-hdr-plane xg-hdr-row");
    let role_col = intern.intern("columnheader");
    let role_row = intern.intern("rowheader");

    // pane-chrome.ts: corner + xg-hbox / xg-hdr-plane wrappers (scroll panes; freeze TODO).
    buf.create_element(CORNER_ID, div, ROOT_ID);
    buf.set_attr(CORNER_ID, keys.class, class_corner);
    place_style_attr(buf, intern, &keys, CORNER_ID, 0, 0, sizes.w, sizes.h);

    buf.create_element(HBOX_COL_ID, div, ROOT_ID);
    buf.set_attr(HBOX_COL_ID, keys.class, class_hbox_col);
    place_style_attr(
        buf,
        intern,
        &keys,
        HBOX_COL_ID,
        sizes.w,
        0,
        cells_w,
        sizes.h,
    );
    buf.create_element(HDR_COL_ID, div, HBOX_COL_ID);
    buf.set_attr(HDR_COL_ID, keys.class, class_hdr_col);
    buf.set_style(HDR_COL_ID, keys.height, intern_px(intern, sizes.h));
    buf.set_style(
        HDR_COL_ID,
        keys.transform,
        intern_translate3d(intern, -scroll_x, 0),
    );

    buf.create_element(HBOX_ROW_ID, div, ROOT_ID);
    buf.set_attr(HBOX_ROW_ID, keys.class, class_hbox_row);
    place_style_attr(
        buf,
        intern,
        &keys,
        HBOX_ROW_ID,
        0,
        sizes.h,
        sizes.w,
        cells_h,
    );
    buf.create_element(HDR_ROW_ID, div, HBOX_ROW_ID);
    buf.set_attr(HDR_ROW_ID, keys.class, class_hdr_row);
    buf.set_style(HDR_ROW_ID, keys.width, intern_px(intern, sizes.w));
    buf.set_style(
        HDR_ROW_ID,
        keys.transform,
        intern_translate3d(intern, 0, -scroll_y),
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
        set_attr_v(buf, intern, id, keys.class, &hdr_class);
        buf.set_attr(id, keys.role, role_col);
        set_attr_v(buf, intern, id, keys.data_index, &col.to_string());
        // Keep unitless 1.35 when font-size is set (matches upstream inherited shell LH).
        buf.set_attr(
            id,
            keys.style,
            intern_col_hdr_style(intern, left, w, font_px),
        );
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
        set_attr_v(buf, intern, id, keys.class, &hdr_class);
        buf.set_attr(id, keys.role, role_row);
        set_attr_v(buf, intern, id, keys.data_index, &row.to_string());
        buf.set_attr(
            id,
            keys.style,
            intern_row_hdr_style(intern, top, h, font_px),
        );
        let label = intern.intern(&((*row + 1).to_string()));
        buf.set_text(id, label);
    }
}
