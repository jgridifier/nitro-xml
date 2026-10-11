// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/group.ts; changes in PROVENANCE.md.

use super::context::{PaintContext, StringIntern};
use super::dispatch::paint_element;
use super::dom::{create_child, position_box, set_attr, set_style};
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-pptx-kind", "group");
    set_style(ctx, id, "overflow", "visible");
    let start = ctx.view_ir.nodes.len();
    if let Some(children) = &el.children {
        for child in children {
            paint_element(ctx, child, id);
        }
    }
    let child_ir = ctx.view_ir.nodes[start..].to_vec();
    ctx.view_ir.nodes.truncate(start);
    ctx.view_ir.push_painted("group", el, Some(child_ir));
}
