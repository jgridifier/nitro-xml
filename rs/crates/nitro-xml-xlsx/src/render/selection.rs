// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/selection-layer.ts; changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::intern::StringIntern;
use crate::model::{ActiveWorksheet, CellRange};
use crate::render::geometry::header_sizes;
use crate::render::grid::{OVERLAY_ID, OVERLAY_TOP_ID};
use crate::render::selection_ops::paint_selection;
use crate::render::viewport::visible_cells_for_host;
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
    parent: u32,
    class: &str,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
) {
    let div = intern.intern("div");
    // Plane-relative (upstream selection-layer.ts); `.xg-overlay` is under `.xg-plane`.
    buf.create_element(id, div, parent);
    set_attr(buf, intern, id, "class", class);
    set_style(buf, intern, id, "left", &format!("{}px", left));
    set_style(buf, intern, id, "top", &format!("{}px", top));
    set_style(buf, intern, id, "width", &format!("{}px", width));
    set_style(buf, intern, id, "height", &format!("{}px", height));
}

/// Plane-relative box clipped to the painted extent (upstream `rectOf` / maxRow / maxCol).
fn range_box(
    metrics: &GridMetrics,
    range: &CellRange,
    max_row: u32,
    max_col: u32,
) -> (i32, i32, i32, i32) {
    let end_row = range.end.row.min(max_row);
    let end_col = range.end.col.min(max_col);
    let x = metrics.col_left(range.start.col);
    let y = metrics.row_top(range.start.row);
    let w = metrics.col_left(end_col + 1) - x;
    let h = metrics.row_top(end_row + 1) - y;
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
    let sizes = header_sizes(sheet, metrics, metrics.last_row);
    let vis = visible_cells_for_host(metrics, viewport, sizes.w, sizes.h);
    let max_row = vis
        .rows
        .last()
        .copied()
        .max(vis.frozen_rows.last().copied())
        .unwrap_or(0)
        .saturating_add(1);
    let max_col = vis
        .cols
        .last()
        .copied()
        .max(vis.frozen_cols.last().copied())
        .unwrap_or(0)
        .saturating_add(1);

    let selection = paint_selection(sheet);

    let active = selection.active;
    let active_range = merge_at(sheet, active.row, active.col)
        .cloned()
        .unwrap_or(CellRange {
            start: active,
            end: active,
        });

    // Upstream selection-layer.ts: outline wraps the sole selected range; when that range is a
    // single cell, the outline is the merge-sized active range. `.xg-active` is hidden for a
    // one-cell selection (even when the active cell is merged).
    let last = selection
        .ranges
        .last()
        .cloned()
        .unwrap_or(active_range.clone());
    let single = selection.ranges.len() == 1;
    let (outline_box, active_box) = if single {
        let outline = if is_single_cell(&last) {
            range_box(metrics, &active_range, max_row, max_col)
        } else {
            range_box(metrics, &last, max_row, max_col)
        };
        let active = if !is_single_cell(&last) {
            Some(range_box(metrics, &active_range, max_row, max_col))
        } else {
            None
        };
        (Some(outline), active)
    } else {
        (
            None,
            Some(range_box(metrics, &active_range, max_row, max_col)),
        )
    };

    // Upstream paints the overlay in every visible quadrant; each clips its part.
    let mut parents = vec![OVERLAY_ID];
    if viewport.frozen_rows > 0 {
        parents.push(OVERLAY_TOP_ID);
    }
    for (qi, parent) in parents.into_iter().enumerate() {
        let id_off = (qi as u32) * 10;
        if let Some((left, top, w, h)) = outline_box {
            place_box(
                buf,
                intern,
                OUTLINE_ID + id_off,
                parent,
                "xg-outline",
                left,
                top,
                w,
                h,
            );
        }
        if let Some((left, top, w, h)) = active_box {
            place_box(
                buf,
                intern,
                ACTIVE_ID + id_off,
                parent,
                "xg-active",
                left,
                top,
                w,
                h,
            );
        }
    }
}
