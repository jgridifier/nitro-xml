// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/connector.ts; changes in PROVENANCE.md.

use super::connector_geometry::connector_stroke_geometry;
use super::connector_markers::{attach_markers, build_marker_def};
use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_inner_markup_stub, set_style};
use super::text::{paint_text_body, TextBodyPaint};
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    // Upstream connector.ts: zero-height `cxnSp` wrappers stay overflow-visible so
    // the absolute SVG stroke (max(dim,1)) is not clipped away.
    set_style(ctx, id, "overflow", "visible");
    set_style(ctx, id, "pointer-events", "none");
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
    let w_view = el.width.max(1.0).round() as i64;
    let h_view = el.height.max(1.0).round() as i64;
    let geom = connector_stroke_geometry(
        el.shape_type.as_deref(),
        el.width,
        el.height,
        el.flip_horizontal,
        el.flip_vertical,
        stroke,
        stroke_width,
        el.stroke_dash.as_deref(),
    );
    let marker_seed = el
        .id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let start_id = el
        .connector_start_arrow
        .as_deref()
        .map(|_| format!("{}-start", marker_seed));
    let end_id = el
        .connector_end_arrow
        .as_deref()
        .map(|_| format!("{}-end", marker_seed));
    let mut inner = String::new();
    if start_id.is_some() || end_id.is_some() {
        inner.push_str("<defs>");
        if let (Some(id), Some(ty)) = (start_id.as_deref(), el.connector_start_arrow.as_deref()) {
            inner.push_str(&build_marker_def(id, ty, stroke));
        }
        if let (Some(id), Some(ty)) = (end_id.as_deref(), el.connector_end_arrow.as_deref()) {
            inner.push_str(&build_marker_def(id, ty, stroke));
        }
        inner.push_str("</defs>");
    }
    let stroke_markup = attach_markers(&geom.markup, start_id.as_deref(), end_id.as_deref());
    inner.push_str(&stroke_markup);
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w_view}\" height=\"{h_view}\" viewBox=\"0 0 {w_view} {h_view}\" style=\"overflow:visible;display:block\">{}</svg>",
        inner
    );
    set_inner_markup_stub(ctx, id, &svg);
    let text = el.text.as_deref().unwrap_or("");
    if !text.trim().is_empty() {
        paint_text_body(ctx, id, text, TextBodyPaint::from_element(el));
    }
    ctx.view_ir.push_painted("connector", el, None);
}
