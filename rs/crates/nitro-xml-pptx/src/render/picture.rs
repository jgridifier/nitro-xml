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
    if let (Some(tile), Some(src)) = (el.image_tile.as_ref(), el.image_src.as_deref()) {
        paint_tile(ctx, el, tile, src, id);
        return;
    }
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

/// `renderImageElement` tiling branch: a repeating background layer (`getImageTilingStyle`).
fn paint_tile<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    el: &DumpElement,
    tile: &crate::slide::ImageTile,
    src: &str,
    host: u32,
) {
    if let Some(shadow) = el.box_shadow.as_deref() {
        set_style(ctx, host, "box-shadow", shadow);
    }
    set_style(ctx, host, "overflow", "hidden");
    let sx = tile.scale_x.map_or(100.0, |v| v * 100.0);
    let sy = tile.scale_y.map_or(100.0, |v| v * 100.0);
    let native = native_image_size(src);
    let flip = tile.flip.as_deref().filter(|f| *f != "none");
    let (image, size) = if let (Some(flip), true) = (flip, src.starts_with("data:")) {
        let (cols, rows) = (
            if flip.contains('x') { 2.0 } else { 1.0 },
            if flip.contains('y') { 2.0 } else { 1.0 },
        );
        let (fx, fy) = (cols > 1.0, rows > 1.0);
        let mut images = String::new();
        for cy in 0..rows as u32 {
            for cx in 0..cols as u32 {
                let (mx, my) = (fx && cx % 2 == 1, fy && cy % 2 == 1);
                images.push_str(&format!(
                    "<image href=\"{}\" x=\"0\" y=\"0\" width=\"1\" height=\"1\" preserveAspectRatio=\"none\" transform=\"translate({},{}) scale({},{})\"/>",
                    src.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;"),
                    cx + u32::from(mx),
                    cy + u32::from(my),
                    if mx { -1 } else { 1 },
                    if my { -1 } else { 1 }
                ));
            }
        }
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {cols} {rows}\" width=\"{cols}\" height=\"{rows}\">{images}</svg>"
        );
        let size = match native {
            Some((w, h)) => format!("{}px {}px", w * sx * cols / 100.0, h * sy * rows / 100.0),
            None => format!("{}% {}%", sx * cols, sy * rows),
        };
        (
            format!("url(\"data:image/svg+xml,{}\")", uri_encode(&svg)),
            size,
        )
    } else {
        let size = match native {
            Some((w, h)) => format!("{}px {}px", w * sx / 100.0, h * sy / 100.0),
            None => format!("{sx}% {sy}%"),
        };
        (format!("url({src})"), size)
    };
    let (ax, ay) = match tile.align.as_deref() {
        Some("t") => (50.0, 0.0),
        Some("tr") => (100.0, 0.0),
        Some("l") => (0.0, 50.0),
        Some("ctr") => (50.0, 50.0),
        Some("r") => (100.0, 50.0),
        Some("bl") => (0.0, 100.0),
        Some("b") => (50.0, 100.0),
        Some("br") => (100.0, 100.0),
        _ => (0.0, 0.0),
    };
    let axis = |pct: f64, off: f64| {
        if off == 0.0 {
            format!("{pct}%")
        } else if pct == 0.0 {
            format!("{off}px")
        } else {
            format!("calc({pct}% + {off}px)")
        }
    };
    let layer = create_child(ctx, host, "div");
    set_attr(ctx, layer, "class", "pptxv-image-tile");
    set_style(ctx, layer, "background-repeat", "repeat");
    set_style(ctx, layer, "background-size", &size);
    set_style(
        ctx,
        layer,
        "background-position",
        &format!(
            "{} {}",
            axis(ax, tile.offset_x.unwrap_or(0.0)),
            axis(ay, tile.offset_y.unwrap_or(0.0))
        ),
    );
    set_style(ctx, layer, "width", "100%");
    set_style(ctx, layer, "height", "100%");
    set_style(ctx, layer, "background-image", &image);
    if let Some(filter) = el.effect_filter.as_deref() {
        set_style(ctx, layer, "filter", filter);
    }
    ctx.view_ir.push_painted("image", el, None);
}

/// `encodeURIComponent`.
fn uri_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Pixel size of a base64 PNG / GIF / JPEG `data:` URL (what `probeNativeImageSize` reads).
fn native_image_size(src: &str) -> Option<(f64, f64)> {
    let b64 = src.strip_prefix("data:")?.split_once(";base64,")?.1;
    let mut bytes = Vec::with_capacity(b64.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0);
    for c in b64.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    let be16 = |i: usize| u16::from_be_bytes([bytes[i], bytes[i + 1]]) as f64;
    if bytes.len() > 24 && bytes.starts_with(b"\x89PNG") {
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        return Some((w as f64, h as f64));
    }
    if bytes.len() > 10 && bytes.starts_with(b"GIF8") {
        let w = u16::from_le_bytes([bytes[6], bytes[7]]) as f64;
        let h = u16::from_le_bytes([bytes[8], bytes[9]]) as f64;
        return Some((w, h));
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xFF {
                i += 1;
                continue;
            }
            let m = bytes[i + 1];
            if (0xC0..=0xCF).contains(&m) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
                return Some((be16(i + 7), be16(i + 5)));
            }
            i += 2 + if m == 0xFF || m == 0xD8 || m == 0x01 {
                0
            } else {
                be16(i + 2) as usize
            };
        }
    }
    None
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
