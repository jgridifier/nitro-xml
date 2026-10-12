// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/text-shape.ts; changes in PROVENANCE.md.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_style};
use super::prst_geom::apply_preset_geometry;
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
    if let Some(src) = el.image_src.as_deref() {
        set_style(ctx, id, "background-image", &format!("url({src})"));
        set_style(ctx, id, "background-size", "100% 100%");
        set_style(ctx, id, "background-repeat", "no-repeat");
    } else {
        match el.fill_mode.as_deref() {
            Some("none") => {}
            Some("gradient") => {
                if let Some(gradient) = &el.fill_gradient {
                    set_style(ctx, id, "background-image", gradient);
                }
            }
            _ => {
                if let Some(color) = &el.fill_color {
                    set_style(ctx, id, "background-color", color);
                }
            }
        }
    }
    if let Some(st) = &el.shape_type {
        set_attr(ctx, id, "data-shape-type", st);
        apply_preset_geometry(ctx, id, st, el.width, el.height);
    }
    if let Some(color) = &el.stroke_color {
        let width = el.stroke_width_px.unwrap_or(1.0).max(0.0);
        set_style(
            ctx,
            id,
            "border",
            &format!("{}px solid {color}", format_px(width)),
        );
    }
    if let Some(rot) = el.rotation {
        if rot.abs() > 0.01 {
            set_style(ctx, id, "transform", &format!("rotate({}deg)", rot));
        }
    }
    let text = el.text.as_deref().unwrap_or("");
    if !text.trim().is_empty() {
        paint_text_body(
            ctx,
            id,
            text,
            el.text_color.as_deref(),
            el.font_size_px,
            el.text_body_layout.as_ref(),
        );
    }
    ctx.view_ir
        .push_painted(if el.type_ == "text" { "text" } else { "shape" }, el, None);
}

fn format_px(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}
