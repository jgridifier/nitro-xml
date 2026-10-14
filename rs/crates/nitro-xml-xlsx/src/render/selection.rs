// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/selection-layer.ts; changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::intern::StringIntern;
use crate::model::{ActiveWorksheet, CellRange};
use crate::render::grid::OVERLAY_ID;
use crate::render::selection_ops::paint_selection;
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
    // Plane-relative (upstream selection-layer.ts); `.xg-overlay` is under `.xg-plane`.
    buf.create_element(id, div, OVERLAY_ID);
    set_attr(buf, intern, id, "class", class);
    set_style(buf, intern, id, "left", &format!("{}px", left));
    set_style(buf, intern, id, "top", &format!("{}px", top));
    set_style(buf, intern, id, "width", &format!("{}px", width));
    set_style(buf, intern, id, "height", &format!("{}px", height));
}

/// Plane-relative box for a cell range (no header offset; plane scrolls with content).
fn range_box(metrics: &GridMetrics, range: &CellRange) -> (i32, i32, i32, i32) {
    let x = metrics.col_left(range.start.col);
    let y = metrics.row_top(range.start.row);
    let w = metrics.col_left(range.end.col + 1) - x;
    let h = metrics.row_top(range.end.row + 1) - y;
    (x - 1, y - 1, w + 1, h + 1)
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
    let _ = viewport;
    let selection = paint_selection(sheet);

    let active = selection.active;
    let active_range = merge_at(sheet, active.row, active.col)
        .cloned()
        .unwrap_or(CellRange {
            start: active,
            end: active,
        });

    // Upstream selection-layer.ts: outline wraps the sole selected range; when
    // that range is one cell but the active cell sits in a merge, the outline
    // grows to `activeRange` (merge-sized). `.xg-active` marks the active cell
    // only when the selection is larger than one cell.
    let last = selection
        .ranges
        .last()
        .cloned()
        .unwrap_or(active_range.clone());
    let single = selection.ranges.len() == 1;
    if single {
        let outline_range = if is_single_cell(&last) {
            &active_range
        } else {
            &last
        };
        let (left, top, w, h) = range_box(metrics, outline_range);
        place_box(buf, intern, OUTLINE_ID, "xg-outline", left, top, w, h);
    }
    if !single || !is_single_cell(&last) {
        let (left, top, w, h) = range_box(metrics, &active_range);
        place_box(buf, intern, ACTIVE_ID, "xg-active", left, top, w, h);
    }
}
