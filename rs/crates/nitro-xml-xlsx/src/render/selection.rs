// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/selection-layer.ts; changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::intern::StringIntern;
use crate::model::ActiveWorksheet;
use crate::render::grid::OVERLAY_ID;
use crate::render::GridPaintViewport;
use crate::sheet_access::merge_at;
use crate::view::metrics::GridMetrics;

const OUTLINE_ID: u32 = 6_000;

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

pub fn paint<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    sheet: &ActiveWorksheet,
    metrics: &GridMetrics,
    viewport: &GridPaintViewport,
) {
    let _ = viewport;
    let selection = sheet.view.selection.as_ref();
    let Some(selection) = selection else {
        return;
    };
    let active = selection.active;
    let range =
        merge_at(sheet, active.row, active.col)
            .cloned()
            .unwrap_or(crate::model::CellRange {
                start: active,
                end: active,
            });
    let end_row = range.end.row;
    let end_col = range.end.col;
    let x = metrics.col_left(range.start.col);
    let y = metrics.row_top(range.start.row);
    let w = metrics.col_left(end_col + 1) - x;
    let h = metrics.row_top(end_row + 1) - y;
    // Plane-relative (upstream selection-layer.ts); `.xg-overlay` is under `.xg-plane`.
    let left = x - 1;
    let top = y - 1;
    let div = intern.intern("div");
    buf.create_element(OUTLINE_ID, div, OVERLAY_ID);
    set_attr(buf, intern, OUTLINE_ID, "class", "xg-outline");
    set_style(buf, intern, OUTLINE_ID, "left", &format!("{}px", left));
    set_style(buf, intern, OUTLINE_ID, "top", &format!("{}px", top));
    set_style(buf, intern, OUTLINE_ID, "width", &format!("{}px", w + 1));
    set_style(buf, intern, OUTLINE_ID, "height", &format!("{}px", h + 1));
}
