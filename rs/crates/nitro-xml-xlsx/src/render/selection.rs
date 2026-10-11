// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/selection-layer.ts; changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::intern::StringIntern;
use crate::model::{ActiveWorksheet, CellRange};
use crate::render::geometry::header_sizes;
use crate::render::GridPaintViewport;
use crate::sheet_access::merge_at;
use crate::view::metrics::GridMetrics;

const OUTLINE_ID: u32 = 6_000;
const ACTIVE_ID: u32 = 6_001;

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

#[allow(clippy::too_many_arguments)] // mirrors upstream place(node, x, y, w, h) plus buf/intern/id/class
fn place_box<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    id: u32,
    class: &str,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
) {
    let div = intern.intern("div");
    buf.create_element(id, div, 1);
    set_attr(buf, intern, id, "class", class);
    set_style(buf, intern, id, "left", &format!("{}px", left));
    set_style(buf, intern, id, "top", &format!("{}px", top));
    set_style(buf, intern, id, "width", &format!("{}px", width));
    set_style(buf, intern, id, "height", &format!("{}px", height));
}

fn range_box(
    metrics: &GridMetrics,
    range: &CellRange,
    header_w: i32,
    header_h: i32,
    viewport: &GridPaintViewport,
) -> (i32, i32, i32, i32) {
    let x = metrics.col_left(range.start.col);
    let y = metrics.row_top(range.start.row);
    let w = metrics.col_left(range.end.col + 1) - x;
    let h = metrics.row_top(range.end.row + 1) - y;
    let left = header_w + x - viewport.scroll_x - 1;
    let top = header_h + y - viewport.scroll_y - 1;
    (left, top, w + 1, h + 1)
}

fn is_single_cell(range: &CellRange) -> bool {
    range.start.row == range.end.row && range.start.col == range.end.col
}

pub fn paint<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    sheet: &ActiveWorksheet,
    metrics: &GridMetrics,
    viewport: &GridPaintViewport,
) {
    let selection = sheet.view.selection.as_ref();
    let Some(selection) = selection else {
        return;
    };
    let header = header_sizes(sheet, metrics, metrics.last_row);

    let active = selection.active;
    let active_range = merge_at(sheet, active.row, active.col)
        .cloned()
        .unwrap_or(CellRange {
            start: active,
            end: active,
        });

    // Upstream: outline wraps the sole selected range; `.xg-active` marks the
    // active cell when the selection is larger than one cell.
    let last = selection
        .ranges
        .last()
        .cloned()
        .unwrap_or(active_range.clone());
    let single = selection.ranges.len() <= 1;
    if single {
        let (left, top, w, h) = range_box(metrics, &last, header.w, header.h, viewport);
        place_box(buf, intern, OUTLINE_ID, "xg-outline", left, top, w, h);
    }
    if !single || !is_single_cell(&last) {
        let (left, top, w, h) = range_box(metrics, &active_range, header.w, header.h, viewport);
        place_box(buf, intern, ACTIVE_ID, "xg-active", left, top, w, h);
    }
}
