// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/text-block.ts; changes in PROVENANCE.md.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_style, set_text_content};
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-pptx-kind", "text");
    let text = el.text.as_deref().unwrap_or("");
    paint_text_body(ctx, id, text);
    ctx.view_ir.push_painted("text", el, None);
}

pub fn paint_text_body<I: StringIntern>(ctx: &mut PaintContext<'_, I>, parent: u32, text: &str) {
    let block = create_child(ctx, parent, "div");
    set_style(ctx, block, "white-space", "pre-wrap");
    set_style(ctx, block, "word-wrap", "break-word");
    set_style(
        ctx,
        block,
        "font",
        "18px Calibri, Carlito, Arial, sans-serif",
    );
    set_text_content(ctx, block, text);
}
