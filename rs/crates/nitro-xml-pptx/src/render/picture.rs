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

    // Apply container transform (rotate, flip) to frame
    if let Some(transform) = get_container_transform(el) {
        set_style(ctx, id, "transform", &transform);
    }

    // Picture shape geometry (custGeom pathData, or prstGeom) clips the frame —
    // never the `<img>` (upstream `resolveShapeGeometry` / `getResolvedShapeClipPath`).
    if let (Some(path_data), Some(path_width), Some(path_height)) =
        (el.path_data.as_deref(), el.path_width, el.path_height)
    {
        if let Some(clip_path) =
            build_custom_geometry_clip_path(path_data, path_width, path_height, el.width, el.height)
        {
            set_style(ctx, id, "clip-path", &clip_path);
            set_style(ctx, id, "overflow", "hidden");
        }
    } else if let Some(st) = el.shape_type.as_deref() {
        if !st.eq_ignore_ascii_case("rect") && !st.eq_ignore_ascii_case("custom") {
            super::prst_geom::apply_preset_geometry(ctx, id, st, el.width, el.height);
        }
    }

    let img = create_child(ctx, id, "img");
    set_attr(ctx, img, "alt", el.name.as_deref().unwrap_or(""));
    set_style(ctx, img, "width", "100%");
    set_style(ctx, img, "height", "100%");
    set_style(ctx, img, "display", "block");
    if let Some(shadow) = el.box_shadow.as_deref() {
        set_style(ctx, id, "box-shadow", shadow);
    }
    if let Some(src) = el.image_src.as_deref() {
        set_attr(ctx, img, "src", src);
        set_style(ctx, img, "object-fit", "fill");
        let has_fit = el.image_fit.is_some();
        if has_fit {
            // getImageFitStyle: the crop/placement is an `<img>` transform inside a clipping frame.
            set_style(ctx, id, "overflow", "hidden");
            set_style(ctx, img, "position", "absolute");
            set_style(ctx, img, "max-width", "none");
            set_style(ctx, img, "max-height", "none");
            set_style(ctx, img, "transform-origin", "top left");
            set_style(
                ctx,
                img,
                "transform",
                &image_fit_transform(&el.image_fit.unwrap()),
            );
        } else if let Some(counter) =
            super::image_fill_counter_transform::get_image_fill_counter_transform(
                el.fill_image_rot_with_shape,
                el.flip_horizontal,
                el.flip_vertical,
                el.rotation,
                false,
            )
        {
            // `a:blipFill/@rotWithShape="0"`: counter-transform on `<img>` only.
            // Do not also paint a flipped CSS background on the frame (upstream
            // uses the `<img>` path exclusively here).
            set_style(ctx, img, "transform", &counter);
            set_style(ctx, img, "transform-origin", "center");
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

/// Build container transform (rotate, scaleX, scaleY) from element's transforms.
fn get_container_transform(el: &DumpElement) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(rot) = el.rotation {
        parts.push(format!("rotate({}deg)", rot));
    }
    if el.flip_horizontal {
        parts.push("scaleX(-1)".to_string());
    }
    if el.flip_vertical {
        parts.push("scaleY(-1)".to_string());
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

/// Port of upstream `buildCustomGeometryClipPath` (shape-geometry.ts): rescale
/// path-space coords into the element border-box, then wrap as CSS `path('…')`.
fn build_custom_geometry_clip_path(
    path_data: &str,
    path_width: f64,
    path_height: f64,
    elem_width: f64,
    elem_height: f64,
) -> Option<String> {
    if path_data.is_empty()
        || !path_width.is_finite()
        || !path_height.is_finite()
        || path_width <= 0.0
        || path_height <= 0.0
        || !elem_width.is_finite()
        || !elem_height.is_finite()
        || elem_width <= 0.0
        || elem_height <= 0.0
    {
        return None;
    }
    let sx = elem_width / path_width;
    let sy = elem_height / path_height;
    let round = |n: f64| -> String {
        let r = (n * 100.0).round() / 100.0;
        if r == 0.0 {
            "0".to_owned()
        } else {
            let s = format!("{r}");
            s
        }
    };
    // Tokenize like upstream: command letter + following payload.
    let mut tokens = Vec::new();
    let mut cur = String::new();
    for ch in path_data.chars() {
        if "MLCQZAHVmlcqzahv".contains(ch) {
            if !cur.is_empty() {
                tokens.push(cur);
            }
            cur = ch.to_string();
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    let mut out = Vec::new();
    for token in tokens {
        let Some(cmd) = token.chars().next() else {
            continue;
        };
        let upper = cmd.to_ascii_uppercase();
        if upper == 'Z' {
            out.push("Z".to_owned());
            continue;
        }
        let nums: Vec<f64> = token[cmd.len_utf8()..]
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        if upper == 'A' {
            let mut parts = Vec::new();
            let mut i = 0;
            while i + 6 < nums.len() {
                parts.push(round(nums[i] * sx));
                parts.push(round(nums[i + 1] * sy));
                parts.push(format!("{}", nums[i + 2]));
                parts.push(format!("{}", nums[i + 3]));
                parts.push(format!("{}", nums[i + 4]));
                parts.push(round(nums[i + 5] * sx));
                parts.push(round(nums[i + 6] * sy));
                i += 7;
            }
            out.push(format!("A {}", parts.join(" ")));
            continue;
        }
        let scaled: Vec<String> = nums
            .iter()
            .enumerate()
            .map(|(i, n)| round(n * if i % 2 == 0 { sx } else { sy }))
            .collect();
        out.push(format!("{upper} {}", scaled.join(" ")));
    }
    if out.is_empty() {
        return None;
    }
    let joined = out.join(" ");
    let escaped = joined.replace('\\', "\\\\").replace('\'', "\\'");
    Some(format!("path('{escaped}')"))
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
    // Upstream mirrors a `flip` tile only for `data:` sources; the viewer hands the renderer
    // `blob:` URLs, so a flipped tile paints as a plain repeat there and the oracle does too.
    let size = match native {
        Some((w, h)) => format!("{}px {}px", w * sx / 100.0, h * sy / 100.0),
        None => format!("{sx}% {sy}%"),
    };
    let image = format!("url({src})");
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
    use super::{build_custom_geometry_clip_path, image_fit_transform};

    // Catches: `a:srcRect` ignored so a cropped placeholder picture shows the whole source stretched.
    #[test]
    fn src_rect_becomes_scale_translate_and_uncropped_is_empty() {
        let t = image_fit_transform(&[0.0, 0.23715, 0.0, 0.23715, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(t, "translate(0%, -45.11%) scale(1, 1.902226)");
        assert_eq!(image_fit_transform(&[0.0; 8]), "");
    }

    // Catches: custGeom path-space clip used raw (400×200) so trapezoid image-fills
    // paint as full rectangles instead of silhouettes (c55ed898215f).
    #[test]
    fn custom_geometry_clip_scales_path_space_into_element_box() {
        let clip = build_custom_geometry_clip_path(
            "M 100 0 L 300 0 L 400 200 L 0 200 Z",
            400.0,
            200.0,
            280.0,
            140.0,
        )
        .expect("clip");
        assert_eq!(clip, "path('M 70 0 L 210 0 L 280 140 L 0 140 Z')");
        assert!(build_custom_geometry_clip_path("M 0 0 Z", 0.0, 100.0, 10.0, 10.0).is_none());
    }
}
