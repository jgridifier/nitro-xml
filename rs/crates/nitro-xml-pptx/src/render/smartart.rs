// SPDX-License-Identifier: MIT
//! P11 SmartArt — S2 stub until full cached-drawing port lands.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_inner_markup_stub};
use crate::slide::DumpElement;

pub fn paint_stub<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "smartArt");
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" role=\"img\"><text>SmartArt</text></svg>";
    set_inner_markup_stub(ctx, id, svg);
    ctx.view_ir.push_painted("smartArt", el, None);
}
