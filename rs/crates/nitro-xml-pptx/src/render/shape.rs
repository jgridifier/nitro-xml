// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/text-shape.ts; changes in PROVENANCE.md.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_style};
use super::text::paint_text_body;
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "shape");
    if let Some(name) = &el.name {
        set_attr(ctx, id, "data-pptx-name", name);
    }
    if let Some(st) = &el.shape_type {
        set_attr(ctx, id, "data-shape-type", st);
        if st != "rect" {
            set_style(ctx, id, "border", "1px solid #4a86c8");
            set_style(ctx, id, "border-radius", "2px");
        }
    }
    if let Some(rot) = el.rotation {
        if rot.abs() > 0.01 {
            set_style(ctx, id, "transform", &format!("rotate({}deg)", rot));
        }
    }
    let text = el.text.as_deref().unwrap_or("");
    if !text.trim().is_empty() {
        paint_text_body(ctx, id, text);
    }
    ctx.view_ir
        .push_painted(if el.type_ == "text" { "text" } else { "shape" }, el, None);
}
