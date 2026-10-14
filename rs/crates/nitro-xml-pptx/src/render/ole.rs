// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/ole.ts; changes in PROVENANCE.md.
//! P15 OLE previews — poster image when a blip / VML / metafile preview resolved, else `[OLE]`.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_inner_markup_stub, set_style};
use super::placeholder;
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    // Loader `scheduleMetafileAttr` rasterizes `nitro-metafile:` img src (bug64693 WMF).
    if el.image_src.is_some() {
        paint_preview(ctx, el, parent);
    } else {
        placeholder::paint_typed_stub(ctx, "ole", el, parent, "[OLE]");
    }
}

fn paint_preview<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "ole");
    set_style(ctx, id, "position", "relative");
    let img = create_child(ctx, id, "img");
    set_attr(ctx, img, "alt", el.name.as_deref().unwrap_or(""));
    set_style(ctx, img, "width", "100%");
    set_style(ctx, img, "height", "100%");
    set_style(ctx, img, "object-fit", "contain");
    set_style(ctx, img, "display", "block");
    if let Some(src) = el.image_src.as_deref() {
        // Prefer <img src> so the loader can resolve nitro-metafile: → PNG.
        set_attr(ctx, img, "src", src);
        if !src.starts_with("nitro-metafile:") {
            let url = format!("url({src})");
            set_style(ctx, id, "background-image", &url);
            set_style(ctx, id, "background-size", "contain");
            set_style(ctx, id, "background-repeat", "no-repeat");
            set_style(ctx, id, "background-position", "center");
        }
    }
    // Upstream `buildPreview` type badge (bottom-right).
    let badge = create_child(ctx, id, "div");
    set_attr(ctx, badge, "aria-hidden", "true");
    set_style(ctx, badge, "position", "absolute");
    set_style(ctx, badge, "bottom", "4px");
    set_style(ctx, badge, "right", "4px");
    set_style(ctx, badge, "z-index", "10");
    set_style(ctx, badge, "pointer-events", "none");
    set_inner_markup_stub(
        ctx,
        badge,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\" class=\"pptxv-ole-badge\">\
<rect x=\"2\" y=\"2\" width=\"20\" height=\"20\" rx=\"3\" fill=\"#4b5563\"/>\
<text x=\"12\" y=\"16\" text-anchor=\"middle\" fill=\"white\" font-size=\"10\" font-weight=\"bold\" font-family=\"Liberation Sans, Arial, sans-serif\">OLE</text>\
</svg>",
    );
    ctx.view_ir.push_painted("ole", el, None);
}
