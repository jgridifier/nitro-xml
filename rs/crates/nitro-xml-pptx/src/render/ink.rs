// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/ink.ts and
// ink-stroke-svg.ts; changes listed in PROVENANCE.md.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_inner_markup_stub, set_style};
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "ink");
    if let Some(rot) = el.rotation {
        if rot.abs() > 0.01 {
            set_style(ctx, id, "transform", &format!("rotate({}deg)", rot));
        }
    }

    let strokes = el
        .ink_data
        .as_ref()
        .map(|d| d.strokes.as_slice())
        .unwrap_or(&[]);
    if !strokes.is_empty() {
        let w = el.width.max(1.0);
        let h = el.height.max(1.0);
        let mut paths = String::new();
        for stroke in strokes {
            let opacity_attr = if stroke.opacity < 0.999 {
                format!(" stroke-opacity=\"{}\"", stroke.opacity)
            } else {
                String::new()
            };
            paths.push_str(&format!(
                "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"{} stroke-linecap=\"round\" stroke-linejoin=\"round\" vector-effect=\"non-scaling-stroke\"/>",
                xml_attr_escape(&stroke.path_d),
                xml_attr_escape(&stroke.color),
                stroke.width,
                opacity_attr
            ));
        }
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"pptxv-ink-svg\" viewBox=\"0 0 {} {}\" preserveAspectRatio=\"none\" style=\"width:100%;height:100%;pointer-events:none;display:block\">{}</svg>",
            w, h, paths
        );
        set_inner_markup_stub(ctx, id, &svg);
    }

    ctx.view_ir.push_painted("ink", el, None);
}

pub(crate) fn xml_attr_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}
