// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/ole.ts; changes in PROVENANCE.md.
//! P15 OLE previews — poster image when a blip or VML preview resolved, else `[OLE]`.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_style, set_text_content};
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    // Upstream only paints a bitmap preview when `previewImageData` is a real
    // raster URL. EMF/WMF arrive as `nitro-metafile:` (loader S3) and TS leaves
    // them as the typed placeholder — matching that avoids a blank/garbled poster
    // (poi-bug64693 soft band).
    match el.image_src.as_deref() {
        Some(src) if !src.starts_with("nitro-metafile:") => paint_preview(ctx, el, parent),
        _ => paint_typed_placeholder(ctx, el, parent),
    }
}

fn paint_typed_placeholder<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    el: &DumpElement,
    parent: u32,
) {
    // Compact type badge approximating upstream `pptxv-ole-badge` / placeholder
    // chrome so metafile OLE (bug64693) does not leave a blank hole vs TS.
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "ole");
    set_style(ctx, id, "display", "flex");
    set_style(ctx, id, "align-items", "center");
    set_style(ctx, id, "justify-content", "center");
    set_style(ctx, id, "background", "#f3f4f6");
    set_style(ctx, id, "border", "1px solid #d1d5db");
    let badge = create_child(ctx, id, "div");
    set_style(ctx, badge, "background", "#4b5563");
    set_style(ctx, badge, "color", "#ffffff");
    set_style(ctx, badge, "font", "bold 10px \"Liberation Sans\", Arial, sans-serif");
    set_style(ctx, badge, "padding", "2px 6px");
    set_style(ctx, badge, "border-radius", "3px");
    set_text_content(ctx, badge, "OLE");
    ctx.view_ir.push_painted("ole", el, None);
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
