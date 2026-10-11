// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/table.ts; changes in PROVENANCE.md.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_style};
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-pptx-kind", "table");
    let rows = el.table_row_count.unwrap_or(1).max(1);
    let table = create_child(ctx, id, "table");
    set_style(ctx, table, "width", "100%");
    set_style(ctx, table, "height", "100%");
    set_style(ctx, table, "border-collapse", "collapse");
    for r in 0..rows.min(12) {
        let tr = create_child(ctx, table, "tr");
        let td = create_child(ctx, tr, "td");
        set_style(ctx, td, "border", "1px solid #999");
        set_attr(ctx, td, "data-row", &r.to_string());
    }
    ctx.view_ir.push_painted("table", el, None);
}
