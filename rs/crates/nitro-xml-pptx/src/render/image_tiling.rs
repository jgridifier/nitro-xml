// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/image-tiling.ts; changes in PROVENANCE.md.

use std::collections::HashMap;

/// Tile configuration parsed from `a:blipFill/a:tile`.
#[derive(Clone, Debug)]
pub struct TileConfig {
    pub scale_x: Option<f64>,
    pub scale_y: Option<f64>,
    pub offset_x: Option<f64>,
    pub offset_y: Option<f64>,
    pub flip: Option<String>,
    pub alignment: Option<String>,
    pub native_width: Option<f64>,
    pub native_height: Option<f64>,
}

/// Whether this element paints its bitmap as a repeating tile.
///
/// Any `a:tile` attribute that survived parsing is taken as proof of the tile
/// element itself: `<a:tile flip="xy"/>` and `<a:tile algn="tl"/>` are legal and
/// carry no scale, and gating on `@sx`/`@sy` alone (as React did) rendered those
/// as a single stretched copy.
pub fn is_image_tiled(
    tile_scale_x: Option<f64>,
    tile_scale_y: Option<f64>,
    tile_offset_x: Option<f64>,
    tile_offset_y: Option<f64>,
    tile_flip: Option<&str>,
    tile_alignment: Option<&str>,
) -> bool {
    tile_scale_x.is_some()
        || tile_scale_y.is_some()
        || tile_offset_x.is_some()
        || tile_offset_y.is_some()
        || (tile_flip.is_some() && tile_flip != Some("none"))
        || tile_alignment.is_some()
}

/// `a:tile/@algn` as the percentage pair CSS anchors a background at.
///
/// A percentage `background-position` aligns the same percentage point of the
/// TILE with that percentage point of the box, which is exactly the OOXML
/// anchoring rule, and unlike the `left`/`center` keywords it composes with the
/// `@tx`/`@ty` offset inside a `calc()`.
fn tile_alignment_percentages(alignment: Option<&str>) -> (f64, f64) {
    match alignment {
        Some("tl") => (0.0, 0.0),
        Some("t") => (50.0, 0.0),
        Some("tr") => (100.0, 0.0),
        Some("l") => (0.0, 50.0),
        Some("ctr") => (50.0, 50.0),
        Some("r") => (100.0, 50.0),
        Some("bl") => (0.0, 100.0),
        Some("b") => (50.0, 100.0),
        Some("br") => (100.0, 100.0),
        _ => (0.0, 0.0), // Default to top-left
    }
}

/// One axis of the tile origin: the alignment anchor plus the `@tx`/`@ty` offset.
fn tile_axis_position(percent: f64, offset_px: f64) -> String {
    if (offset_px.abs()) < 0.001 {
        format!("{}%", percent)
    } else if (percent.abs()) < 0.001 {
        format!("{}px", offset_px)
    } else {
        // calc() lets the anchor and the offset coexist
        if offset_px >= 0.0 {
            format!("calc({}% + {}px)", percent, offset_px)
        } else {
            format!("calc({}% - {}px)", percent, -offset_px)
        }
    }
}

/// Bake `a:tile/@flip` into a single composite tile.
///
/// CSS `background-repeat` cannot mirror alternate tiles, so the mirror is baked
/// into the tile itself: an inline SVG lays out 2 (`x` / `y`) or 4 (`xy`)
/// mirrored copies of the source, and that composite then repeats seamlessly —
/// each neighbour is the mirror of the last, exactly as PowerPoint tiles it.
///
/// The source must be an embeddable `data:` URI: an SVG loaded as a `data:`
/// background has an opaque origin and cannot reference `blob:`/`http:` hrefs, so
/// for those sources this returns `None` and the caller keeps plain
/// (non-mirrored) repetition.
///
/// # Arguments
///
/// * `src` - The tile image source (must start with `data:`).
/// * `flip` - Tile flip mode (`x` / `y` / `xy`).
/// * `scale_x` - Per-tile horizontal size as a percentage (e.g. 100 = 100%).
/// * `scale_y` - Per-tile vertical size as a percentage.
/// * `native_width` - The source's native pixel width, when known.
/// * `native_height` - The source's native pixel height, when known.
///
/// # Returns
///
/// A pair of (backgroundImage, backgroundSize) CSS values, or `None` when no
/// mirror applies or the source is not embeddable.
pub fn build_mirror_tiled_background(
    src: &str,
    flip: &str,
    scale_x: f64,
    scale_y: f64,
    native_width: Option<f64>,
    native_height: Option<f64>,
) -> Option<(String, String)> {
    if !src.starts_with("data:") {
        return None;
    }

    let flip_x = flip == "x" || flip == "xy";
    let flip_y = flip == "y" || flip == "xy";
    let cols = if flip_x { 2 } else { 1 };
    let rows = if flip_y { 2 } else { 1 };

    // Escape SVG attribute values: src is a data URI with `<`, `&`, `"` that need encoding.
    let href = escape_svg_attr(src);

    let mut images = Vec::new();
    for cy in 0..rows {
        for cx in 0..cols {
            let mirror_x = flip_x && cx % 2 == 1;
            let mirror_y = flip_y && cy % 2 == 1;
            let sx = if mirror_x { -1 } else { 1 };
            let sy = if mirror_y { -1 } else { 1 };

            // After scale(-1) the unit cell spans [-1,0]; shift it back by +1 so it
            // lands in [0,1], then translate to the target grid cell (cx, cy).
            let tx = cx as f64 + if mirror_x { 1.0 } else { 0.0 };
            let ty = cy as f64 + if mirror_y { 1.0 } else { 0.0 };

            images.push(format!(
                r#"<image href="{}" x="0" y="0" width="1" height="1" preserveAspectRatio="none" transform="translate({},{}),scale({},{})"/>"#,
                href, tx, ty, sx, sy
            ));
        }
    }

    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">{}</svg>"#,
        cols,
        rows,
        cols,
        rows,
        images.join("")
    );

    let background_size = if let (Some(w), Some(h)) = (native_width, native_height) {
        // Use absolute pixels when native size is known (per ECMA-376 §20.1.8.58)
        format!(
            "{}px {}px",
            (w * scale_x * cols as f64) / 100.0,
            (h * scale_y * rows as f64) / 100.0
        )
    } else {
        // Fall back to percentage of container
        format!("{}% {}%", scale_x * cols as f64, scale_y * rows as f64)
    };

    let background_image = format!("url(\"data:image/svg+xml,{}\")", encode_svg_data(&svg));

    Some((background_image, background_size))
}

/// Escape SVG attribute values for use in `data:` URIs.
fn escape_svg_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Encode SVG data for a `data:` URI (upstream `encodeURIComponent`).
fn encode_svg_data(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len() * 3);
    for b in svg.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Sync native pixel size from an embeddable `data:` image URL (PNG IHDR /
/// JPEG SOF). Used so `a:tile/@sx|@sy` can be absolute pixels per ECMA-376
/// without waiting for a NaturalWidth layout round-trip.
pub fn native_size_from_data_url(src: &str) -> Option<(f64, f64)> {
    let payload = src.strip_prefix("data:")?;
    let (meta, data) = payload.split_once(',')?;
    let bytes = if meta.contains(";base64") {
        base64_decode(data)?
    } else {
        // Rare non-base64 data URLs — skip.
        return None;
    };
    if meta.starts_with("image/png") {
        return png_size(&bytes);
    }
    if meta.starts_with("image/jpeg") || meta.starts_with("image/jpg") {
        return jpeg_size(&bytes);
    }
    None
}

fn png_size(bytes: &[u8]) -> Option<(f64, f64)> {
    // Signature (8) + IHDR length (4) + type (4) + width/height (8).
    if bytes.len() < 24 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    if &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    if w == 0 || h == 0 {
        return None;
    }
    Some((f64::from(w), f64::from(h)))
}

fn jpeg_size(bytes: &[u8]) -> Option<(f64, f64)> {
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return None;
    }
    let mut i = 2usize;
    while i + 9 < bytes.len() {
        if bytes[i] != 0xff {
            return None;
        }
        while i < bytes.len() && bytes[i] == 0xff {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        let marker = bytes[i];
        i += 1;
        if marker == 0xd9 || marker == 0xda {
            return None;
        }
        if i + 1 >= bytes.len() {
            return None;
        }
        let seglen = u16::from_be_bytes([bytes[i], bytes[i + 1]]) as usize;
        if seglen < 2 || i + seglen > bytes.len() {
            return None;
        }
        // SOF0..SOF3 (baseline/progressive) carry height/width.
        if (0xc0..=0xc3).contains(&marker) && seglen >= 7 {
            let h = u16::from_be_bytes([bytes[i + 3], bytes[i + 4]]);
            let w = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]);
            if w > 0 && h > 0 {
                return Some((f64::from(w), f64::from(h)));
            }
        }
        i += seglen;
    }
    None
}

fn base64_decode(data: &str) -> Option<Vec<u8>> {
    // Minimal decoder for data-URL payloads (no whitespace required in our fixtures).
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let clean: Vec<u8> = data
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    if clean.is_empty() {
        return Some(Vec::new());
    }
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    let mut i = 0;
    while i + 3 < clean.len() {
        let a = val(clean[i])?;
        let b = val(clean[i + 1])?;
        let c = val(clean[i + 2])?;
        let d = val(clean[i + 3])?;
        out.push((a << 2) | (b >> 4));
        out.push((b << 4) | (c >> 2));
        out.push((c << 6) | d);
        i += 4;
    }
    let rem = clean.len() - i;
    if rem == 2 {
        let a = val(clean[i])?;
        let b = val(clean[i + 1])?;
        out.push((a << 2) | (b >> 4));
    } else if rem == 3 {
        let a = val(clean[i])?;
        let b = val(clean[i + 1])?;
        let c = val(clean[i + 2])?;
        out.push((a << 2) | (b >> 4));
        out.push((b << 4) | (c >> 2));
    } else if rem != 0 {
        return None;
    }
    Some(out)
}

/// CSS styles for a tiled image, or `None` when the element is not tiled.
///
/// Returned as a full-size background layer, because a repeating fill cannot be
/// expressed on an `<img>`: the binding renders a `<div>` carrying this style in
/// place of the `<img>` it would otherwise emit.
pub fn get_image_tiling_style(
    image_src: Option<&str>,
    config: &TileConfig,
) -> Option<HashMap<&'static str, String>> {
    if !is_image_tiled(
        config.scale_x,
        config.scale_y,
        config.offset_x,
        config.offset_y,
        config.flip.as_deref(),
        config.alignment.as_deref(),
    ) {
        return None;
    }

    let src = image_src?;

    // @sx/@sy default to 100% when the attribute is absent
    let scale_x = config.scale_x.unwrap_or(100.0);
    let scale_y = config.scale_y.unwrap_or(100.0);
    let offset_x = config.offset_x.unwrap_or(0.0);
    let offset_y = config.offset_y.unwrap_or(0.0);

    // @algn anchors the tile grid and @tx/@ty shift it from that anchor: they COMPOSE
    let (anchor_x, anchor_y) = tile_alignment_percentages(config.alignment.as_deref());
    let background_position = format!(
        "{} {}",
        tile_axis_position(anchor_x, offset_x),
        tile_axis_position(anchor_y, offset_y)
    );

    let flip = config.flip.as_deref().unwrap_or("none");
    let mirror = if flip != "none" {
        build_mirror_tiled_background(
            src,
            flip,
            scale_x,
            scale_y,
            config.native_width,
            config.native_height,
        )
    } else {
        None
    };

    let background_size = if let (Some(w), Some(h)) = (config.native_width, config.native_height) {
        format!("{}px {}px", (w * scale_x) / 100.0, (h * scale_y) / 100.0)
    } else {
        format!("{}% {}%", scale_x, scale_y)
    };

    let mut style = HashMap::new();
    style.insert("background-repeat" as &'static str, "repeat".to_string());
    style.insert(
        "background-size" as &'static str,
        mirror
            .as_ref()
            .map(|(_, size)| size.clone())
            .unwrap_or(background_size),
    );
    style.insert("background-position" as &'static str, background_position);
    style.insert("width" as &'static str, "100%".to_string());
    style.insert("height" as &'static str, "100%".to_string());

    if let Some((bg_image, _)) = mirror {
        style.insert("background-image" as &'static str, bg_image);
    } else {
        style.insert("background-image" as &'static str, format!("url({})", src));
    }

    Some(style)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: is_image_tiled rejects perfectly valid tile-only elements like <a:tile algn="tl"/>
    #[test]
    fn tile_with_alignment_only_is_tiled() {
        assert!(is_image_tiled(None, None, None, None, None, Some("tl")));
    }

    // Catches: tile_axis_position fails to compose offset with percentage anchor
    #[test]
    fn tile_axis_position_composes_anchor_and_offset() {
        assert_eq!(tile_axis_position(50.0, 0.0), "50%");
        assert_eq!(tile_axis_position(0.0, 10.0), "10px");
        assert_eq!(
            tile_axis_position(50.0, 10.0),
            "calc(50% + 10px)",
            "positive offset"
        );
        assert_eq!(
            tile_axis_position(50.0, -10.0),
            "calc(50% - 10px)",
            "negative offset"
        );
    }

    // Catches: tile_alignment_percentages defaults to top-left instead of ctr
    #[test]
    fn tile_alignment_maps_to_percentages() {
        assert_eq!(tile_alignment_percentages(Some("tl")), (0.0, 0.0));
        assert_eq!(tile_alignment_percentages(Some("ctr")), (50.0, 50.0));
        assert_eq!(tile_alignment_percentages(Some("br")), (100.0, 100.0));
        assert_eq!(tile_alignment_percentages(None), (0.0, 0.0)); // default
    }

    // Catches: get_image_tiling_style returns Some when all tile props are None
    #[test]
    fn get_image_tiling_style_returns_none_when_not_tiled() {
        let config = TileConfig {
            scale_x: None,
            scale_y: None,
            offset_x: None,
            offset_y: None,
            flip: None,
            alignment: None,
            native_width: None,
            native_height: None,
        };
        assert_eq!(
            get_image_tiling_style(Some("data:image/png;base64,ABC"), &config),
            None
        );
    }

    // Catches: build_mirror_tiled_background accepts non-data: URIs
    #[test]
    fn build_mirror_tiled_background_rejects_non_data_uris() {
        assert_eq!(
            build_mirror_tiled_background(
                "blob:http://example.com/abc123",
                "xy",
                100.0,
                100.0,
                None,
                None
            ),
            None
        );
    }

    // Catches: tile @sx/@sy sized against the container because PNG IHDR was
    // never read from the data URL (b332 tiles paint at 25% of the box ≈ 25px
    // instead of 25% of 16px = 4px).
    #[test]
    fn native_size_from_16x16_png_data_url() {
        // 1×1 PNG is too small a signal; use a real 16×16 IHDR prefix via a
        // minimal valid PNG (IHDR only + IEND).
        let png: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // sig
            0x00, 0x00, 0x00, 0x0d, // IHDR len
            0x49, 0x48, 0x44, 0x52, // IHDR
            0x00, 0x00, 0x00, 0x10, // w=16
            0x00, 0x00, 0x00, 0x10, // h=16
            0x08, 0x02, 0x00, 0x00, 0x00, // bit/color/etc
            0x00, 0x00, 0x00, 0x00, // crc (ignored)
        ];
        let b64 = {
            const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut s = String::new();
            let mut i = 0;
            while i + 2 < png.len() {
                let n = ((png[i] as u32) << 16) | ((png[i + 1] as u32) << 8) | png[i + 2] as u32;
                s.push(T[((n >> 18) & 63) as usize] as char);
                s.push(T[((n >> 12) & 63) as usize] as char);
                s.push(T[((n >> 6) & 63) as usize] as char);
                s.push(T[(n & 63) as usize] as char);
                i += 3;
            }
            s
        };
        let url = format!("data:image/png;base64,{b64}");
        assert_eq!(native_size_from_data_url(&url), Some((16.0, 16.0)));
    }
}
