// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/image.ts; changes in PROVENANCE.md.

use nitro_xml_render::measure::LayoutReadKind;

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_style};
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    match (el.is_media, el.media_type.as_deref()) {
        (true, Some("video")) => {
            paint_media_video(ctx, el, parent);
        }
        (true, Some("audio")) => {
            paint_media_audio(ctx, el, parent);
        }
        _ => paint_image(ctx, el, parent),
    }
}

fn paint_media_host<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    el: &DumpElement,
    parent: u32,
) -> u32 {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "class", "pptxv-element pptxv-media");
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "media");
    if let Some(shadow) = el.box_shadow.as_deref() {
        set_style(ctx, id, "box-shadow", shadow);
    }
    id
}

fn paint_media_video<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    el: &DumpElement,
    parent: u32,
) {
    let id = paint_media_host(ctx, el, parent);
    let video = create_child(ctx, id, "video");
    set_attr(ctx, video, "class", "pptxv-media-video");
    set_style(ctx, video, "width", "100%");
    set_style(ctx, video, "height", "100%");
    set_style(ctx, video, "object-fit", "contain");
    set_style(ctx, video, "display", "block");
    set_attr(ctx, video, "controls", "");
    set_attr(ctx, video, "preload", "metadata");
    set_attr(ctx, video, "playsinline", "");
    if let Some(src) = el.media_src.as_deref() {
        set_attr(ctx, video, "src", src);
    }
    if let Some(poster) = el.image_src.as_deref() {
        set_attr(ctx, video, "poster", poster);
    }
    set_attr(ctx, video, "alt", el.name.as_deref().unwrap_or(""));
    ctx.view_ir.push_painted("video", el, None);
}

/// Upstream `media.ts`: audio renders native `<audio controls>` (reading-stage transport).
fn paint_media_audio<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    el: &DumpElement,
    parent: u32,
) {
    let id = paint_media_host(ctx, el, parent);
    if let Some(src) = el.media_src.as_deref() {
        let audio = create_child(ctx, id, "audio");
        set_attr(ctx, audio, "class", "pptxv-media-audio");
        set_style(ctx, audio, "width", "100%");
        set_attr(ctx, audio, "controls", "");
        set_attr(ctx, audio, "preload", "metadata");
        set_attr(ctx, audio, "src", src);
    } else if let Some(poster) = el.image_src.as_deref() {
        // No playable source: poster still (upstream media fallback).
        let img = create_child(ctx, id, "img");
        set_attr(ctx, img, "class", "pptxv-media-poster");
        set_attr(ctx, img, "src", poster);
        set_attr(ctx, img, "alt", "");
        set_style(ctx, img, "width", "100%");
        set_style(ctx, img, "height", "100%");
        set_style(ctx, img, "object-fit", "contain");
        set_style(ctx, img, "display", "block");
    }
    ctx.view_ir.push_painted("audio", el, None);
}

fn paint_image<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "image");
    let img = create_child(ctx, id, "img");
    set_attr(ctx, img, "alt", el.name.as_deref().unwrap_or(""));
    set_style(ctx, img, "width", "100%");
    set_style(ctx, img, "height", "100%");
    if let Some(shadow) = el.box_shadow.as_deref() {
        set_style(ctx, id, "box-shadow", shadow);
    }
    if let Some(src) = el.image_src.as_deref() {
        set_attr(ctx, img, "src", src);
        set_style(ctx, img, "object-fit", "fill");
        if let Some(fit) = el.image_fit {
            // getImageFitStyle: the crop/placement is an `<img>` transform inside a clipping frame.
            set_style(ctx, id, "overflow", "hidden");
            set_style(ctx, img, "position", "absolute");
            set_style(ctx, img, "max-width", "none");
            set_style(ctx, img, "max-height", "none");
            set_style(ctx, img, "transform-origin", "top left");
            set_style(ctx, img, "transform", &image_fit_transform(&fit));
        } else {
            let url = format!("url({src})");
            set_style(ctx, id, "background-image", &url);
            set_style(ctx, id, "background-size", "100% 100%");
            set_style(ctx, id, "background-repeat", "no-repeat");
        }
        if let Some(filter) = el.effect_filter.as_deref() {
            set_style(ctx, img, "filter", filter);
        }
    } else {
        set_style(ctx, img, "object-fit", "contain");
        set_style(ctx, id, "background", "#e8e8e8");
    }
    // P5 a:tile sx/sy uses intrinsic pixels via NaturalWidth read-back (#127).
    ctx.push_layout_read(img, LayoutReadKind::NaturalWidth, 0, None);
    // EMF/WMF → TODO metafile S3 (`nitro_xml_render::metafile`).
    ctx.view_ir.push_painted("image", el, None);
}

/// Ported from `fitTransformsFromInsets` (element-style.ts): placement then crop.
fn image_fit_transform(f: &[f64; 8]) -> String {
    let r2 = |v: f64| (v * 100.0).round() / 100.0;
    let r6 = |v: f64| (v * 1e6).round() / 1e6;
    let clamp = |v: f64| {
        let m = v.abs().min(0.95);
        if v < 0.0 {
            -m
        } else {
            m
        }
    };
    let (fl, ft, fr, fb) = (f[4], f[5], f[6], f[7]);
    let mut parts = Vec::new();
    if fl.abs() + ft.abs() + fr.abs() + fb.abs() > 0.0001 {
        parts.push(format!(
            "translate({}%, {}%) scale({}, {})",
            r2(fl * 100.0),
            r2(ft * 100.0),
            r6((1.0 - fl - fr).max(0.01)),
            r6((1.0 - ft - fb).max(0.01))
        ));
    }
    let (mut cl, mut ct, mut cr, mut cb) = (clamp(f[0]), clamp(f[1]), clamp(f[2]), clamp(f[3]));
    if [cl, ct, cr, cb].iter().any(|v| v.abs() > 0.0001) {
        let hs = if cl + cr >= 0.99 {
            0.99 / (cl + cr)
        } else {
            1.0
        };
        let vs = if ct + cb >= 0.99 {
            0.99 / (ct + cb)
        } else {
            1.0
        };
        cl = clamp(cl * hs);
        cr = clamp(cr * hs);
        ct = clamp(ct * vs);
        cb = clamp(cb * vs);
        let rw = (1.0 - cl - cr).max(0.01);
        let rh = (1.0 - ct - cb).max(0.01);
        parts.push(format!(
            "translate({}%, {}%) scale({}, {})",
            ((-cl / rw) * 10000.0).round() / 100.0 + 0.0,
            ((-ct / rh) * 10000.0).round() / 100.0 + 0.0,
            ((1.0 / rw) * 1e6).round() / 1e6,
            ((1.0 / rh) * 1e6).round() / 1e6
        ));
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::image_fit_transform;

    // Catches: `a:srcRect` ignored so a cropped placeholder picture shows the whole source stretched.
    #[test]
    fn src_rect_becomes_scale_translate_and_uncropped_is_empty() {
        let t = image_fit_transform(&[0.0, 0.23715, 0.0, 0.23715, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(t, "translate(0%, -45.11%) scale(1, 1.902226)");
        assert_eq!(image_fit_transform(&[0.0; 8]), "");
    }
}
