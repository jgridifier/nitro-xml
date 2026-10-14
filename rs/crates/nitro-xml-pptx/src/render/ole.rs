// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/ole.ts; changes in PROVENANCE.md.
//! P15 OLE previews — poster image when a blip or VML preview resolved, else `[OLE]`.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_style};
use super::placeholder;
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    // Upstream only paints a bitmap preview when `previewImageData` is a real
    // raster URL. EMF/WMF arrive as `nitro-metafile:` (loader S3) and TS leaves
    // them as the typed placeholder — matching that avoids a blank/garbled poster
    // (poi-bug64693 soft band).
    match el.image_src.as_deref() {
        Some(src) if !src.starts_with("nitro-metafile:") => paint_preview(ctx, el, parent),
        _ => placeholder::paint_typed_stub(ctx, "ole", el, parent, "[OLE]"),
    }
}

fn paint_preview<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "ole");
    let img = create_child(ctx, id, "img");
    set_attr(ctx, img, "alt", el.name.as_deref().unwrap_or(""));
    set_style(ctx, img, "width", "100%");
    set_style(ctx, img, "height", "100%");
    set_style(ctx, img, "object-fit", "contain");
    if let Some(src) = el.image_src.as_deref() {
        set_attr(ctx, img, "src", src);
        let url = format!("url({src})");
        set_style(ctx, id, "background-image", &url);
        set_style(ctx, id, "background-size", "contain");
        set_style(ctx, id, "background-repeat", "no-repeat");
        set_style(ctx, id, "background-position", "center");
    }
    ctx.view_ir.push_painted("ole", el, None);
}
