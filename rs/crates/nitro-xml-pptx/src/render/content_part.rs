// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/contentpart.ts; changes listed in PROVENANCE.md.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_inner_markup_stub, set_style};
use super::ink::xml_attr_escape;
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "contentPart");
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
    if strokes.is_empty() {
        set_attr(ctx, id, "class", "pptxv-placeholder");
        let label_id = create_child(ctx, id, "div");
        set_attr(ctx, label_id, "class", "pptxv-placeholder-label");
        set_inner_markup_stub(ctx, label_id, "Content Part");
        ctx.view_ir.push_painted("contentPart", el, None);
        return;
    }

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
        "<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"pptxv-contentpart-svg\" viewBox=\"0 0 {} {}\" preserveAspectRatio=\"none\" style=\"width:100%;height:100%;pointer-events:none;display:block\">{}</svg>",
        w, h, paths
    );
    set_inner_markup_stub(ctx, id, &svg);
    ctx.view_ir.push_painted("contentPart", el, None);
}
