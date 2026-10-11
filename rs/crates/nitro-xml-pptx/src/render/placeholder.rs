// SPDX-License-Identifier: MIT
//! P22 unsupported / not-yet-painted element placeholders.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_style, set_text_content};
use crate::slide::DumpElement;

pub fn paint_unknown_element<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    el: &DumpElement,
    parent: u32,
) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", &el.type_);
    set_style(ctx, id, "border", "1px dashed #c00");
    set_style(ctx, id, "font", "11px sans-serif");
    set_style(ctx, id, "color", "#900");
    set_text_content(ctx, id, &format!("[{}]", el.type_));
    ctx.view_ir.push_painted(&el.type_, el, None);
}

pub fn paint_other_kind<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    kind: &str,
    index: usize,
    parent: u32,
    canvas_w: f64,
    canvas_h: f64,
) {
    let id = create_child(ctx, parent, "div");
    let band_h = 24.0;
    let y = (index as f64 * band_h).min(canvas_h - band_h);
    position_box(ctx, id, 0.0, y, canvas_w.min(200.0), band_h);
    set_attr(ctx, id, "data-pptx-other-kind", kind);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_style(ctx, id, "font", "10px sans-serif");
    set_style(ctx, id, "background", "#fff3cd");
    set_text_content(ctx, id, &format!("[{}]", kind));
    ctx.view_ir.push_other_kind(kind, index);
}

pub fn paint_typed_stub<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    kind: &str,
    el: &DumpElement,
    parent: u32,
    label: &str,
) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", kind);
    set_style(ctx, id, "display", "flex");
    set_style(ctx, id, "align-items", "center");
    set_style(ctx, id, "justify-content", "center");
    set_style(ctx, id, "background", "#f0f0f0");
    set_style(ctx, id, "font", "12px sans-serif");
    set_text_content(ctx, id, label);
    ctx.view_ir.push_painted(kind, el, None);
}
