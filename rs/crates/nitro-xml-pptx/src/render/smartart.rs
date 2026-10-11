// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/smartart-svg.ts;
// changes in PROVENANCE.md.
//! P11 SmartArt — S2 cached-drawing bootstrap at the element box.
//!
//! Full layout fallback and P12 3D remain follow-ups (`smartart3d.rs`).

use nitro_xml_render::smartart::smartart_svg;

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_inner_markup_stub};
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "smartArt");
    let svg = smartart_svg(el.smartart_drawing_xml.as_deref(), el.width, el.height);
    set_inner_markup_stub(ctx, id, &svg);
    ctx.view_ir.push_painted("smartArt", el, None);
}
