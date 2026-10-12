// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/connector.ts; changes in PROVENANCE.md.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_inner_markup_stub};
use super::text::{paint_text_body, TextBodyPaint};
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "connector");
    let stroke = el.stroke_color.as_deref().unwrap_or("#333333");
    let width = el.stroke_width_px.unwrap_or(1.5);
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100%\" height=\"100%\" viewBox=\"0 0 {} {}\"><line x1=\"0\" y1=\"{}\" x2=\"{}\" y2=\"0\" stroke=\"{stroke}\" stroke-width=\"{width}\"/></svg>",
        el.width.max(1.0).round(),
        el.height.max(1.0).round(),
        (el.height * 0.5).round(),
        el.width.max(1.0).round()
    );
    set_inner_markup_stub(ctx, id, &svg);
    let text = el.text.as_deref().unwrap_or("");
    if !text.trim().is_empty() {
        paint_text_body(ctx, id, text, TextBodyPaint::from_element(el));
    }
    ctx.view_ir.push_painted("connector", el, None);
}
