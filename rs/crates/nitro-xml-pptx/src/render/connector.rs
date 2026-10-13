// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/connector.ts; changes in PROVENANCE.md.

use super::connector_geometry::connector_stroke_geometry;
use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_inner_markup_stub, set_style};
use super::text::{paint_text_body, TextBodyPaint};
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "connector");
    if let Some(rot) = el.rotation {
        if rot.abs() > 0.01 {
            set_style(ctx, id, "transform", &format!("rotate({}deg)", rot));
        }
    }
    let stroke = el.stroke_color.as_deref().unwrap_or("#333333");
    let stroke_width = el.stroke_width_px.unwrap_or(1.5);
    let w_view = el.width.max(1.0).round();
    let h_view = el.height.max(1.0).round();
    let geom = connector_stroke_geometry(
        el.shape_type.as_deref(),
        el.width,
        el.height,
        el.flip_horizontal,
        el.flip_vertical,
        stroke,
        stroke_width,
    );
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100%\" height=\"100%\" viewBox=\"0 0 {} {}\" style=\"overflow:visible;display:block\">{}</svg>",
        w_view,
        h_view,
        geom.markup
    );
    set_inner_markup_stub(ctx, id, &svg);
    let text = el.text.as_deref().unwrap_or("");
    if !text.trim().is_empty() {
        paint_text_body(ctx, id, text, TextBodyPaint::from_element(el));
    }
    ctx.view_ir.push_painted("connector", el, None);
}
