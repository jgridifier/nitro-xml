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
        let fill_bg = match el.fill_mode.as_deref() {
            Some("none") => None,
            Some("gradient") => el.fill_gradient.clone(),
            _ => None,
        };
        if let Some(bg) =
            compose_background_image(el.material_overlay.as_deref(), fill_bg.as_deref())
        {
            set_style(ctx, id, "background-image", &bg);
        }
        if fill_bg.is_none() {
            if let Some(color) = &el.fill_color {
                if !matches!(el.fill_mode.as_deref(), Some("none")) {
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
    if let Some(shadow) =
        compose_box_shadow(el.material_box_shadow.as_deref(), el.box_shadow.as_deref())
    {
        set_style(ctx, id, "box-shadow", &shadow);
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
            el.text_shadow.as_deref(),
        );
    }
    ctx.view_ir
        .push_painted(if el.type_ == "text" { "text" } else { "shape" }, el, None);
}

/// Material inset shadows first, then outer/drop shadows (matches upstream layering).
fn compose_box_shadow(material: Option<&str>, outer: Option<&str>) -> Option<String> {
    match (material, outer) {
        (Some(m), Some(o)) => Some(format!("{m}, {o}")),
        (Some(m), None) => Some(m.to_owned()),
        (None, Some(o)) => Some(o.to_owned()),
        (None, None) => None,
    }
}

/// Material gradient on top of fill gradient (TS `visual-3d-materials` order).
fn compose_background_image(overlay: Option<&str>, fill: Option<&str>) -> Option<String> {
    match (overlay, fill) {
        (Some(o), Some(f)) => Some(format!("{o}, {f}")),
        (Some(o), None) => Some(o.to_owned()),
        (None, Some(f)) => Some(f.to_owned()),
        (None, None) => None,
    }
}

fn format_px(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}
