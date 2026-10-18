// SPDX-License-Identifier: MIT
//! Port of upstream text-warp.ts: path generators for WordArt text warp presets.
//! Ported from: ChristopherVR/ooxml@e66aaec upstream/src/ui/src/pptx/render/text-warp.ts
//! Changes: Fixture-scoped path-based warps (priority 1–4); envelope preset path fallbacks.

#[allow(unused_imports)]
pub use super::wordart_envelope_affine::fit_glyph_envelope_affine;
#[allow(unused_imports)]
pub use super::wordart_envelope_curves::has_glyph_envelope;
#[allow(unused_imports)]
pub use super::wordart_envelope_map::create_envelope_warp;

/// Clamp adjustment factor to 0..4 range.
fn clamp4(n: f64) -> f64 {
    n.clamp(0.0, 4.0)
}

// ── Priority 1 ──────────────────────────────────────────────────────────

/// Concentric upward arcs. t=0 is the tallest arch.
fn arch_up_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let adj_norm = adj
        .map(|a| (a as f64 / 21600000.0).clamp(0.0, 1.0))
        .unwrap_or(0.5);
    let max_arch = (0.85 * adj_norm) / 0.5;
    let arch_h = h * (max_arch - t * 0.7).max(0.0);
    if arch_h < 1.0 {
        format!("M 0,{} L {},{}", h, w, h)
    } else {
        format!("M 0,{} A {},{} 0 0,1 {},{}", h, w / 2.0, arch_h, w, h)
    }
}

/// Concentric downward arcs. t=1 is the deepest.
fn arch_down_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let adj_norm = adj
        .map(|a| (a as f64 / 21600000.0).clamp(0.0, 1.0))
        .unwrap_or(0.5);
    let base_depth = (0.15 * adj_norm) / 0.5;
    let arch_h = h * (base_depth + t * 0.7);
    if arch_h < 1.0 {
        format!("M 0,0 L {},0", w)
    } else {
        format!("M 0,0 A {},{} 0 0,0 {},{}", w / 2.0, arch_h, w, 0)
    }
}

/// Full ellipse: concentric ellipses shrink towards centre.
fn circle_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let cx = w / 2.0;
    let cy = h / 2.0;
    let adj_norm = adj
        .map(|a| (a as f64 / 21600000.0).clamp(0.0, 1.0))
        .unwrap_or(0.5);
    let base_scale = 0.45 + adj_norm * 1.1;
    let scale = (base_scale.min(1.0) - t * 0.55).max(0.0);
    let rx = ((w / 2.0) * scale).max(1.0);
    let ry = ((h / 2.0) * scale).max(1.0);
    format!(
        "M {},{} A {},{} 0 1,1 {},{} A {},{} 0 1,1 {},{}",
        cx,
        cy - ry,
        rx,
        ry,
        cx,
        cy + ry,
        rx,
        ry,
        cx,
        cy - ry
    )
}

/// Inflate: the baseline bows up in the middle (balloon shape).
fn inflate_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let y_base = h * (0.15 + t * 0.7);
    let intensity = adj.map(|a| clamp4(a as f64 / 18750.0)).unwrap_or(1.0);
    let base = h * 0.22 * intensity;
    let extra = h * 0.1 * intensity * (0.5 - t);
    let bulge = base + extra;
    format!(
        "M 0,{} Q {},{} {},{}",
        y_base,
        w / 2.0,
        y_base - bulge,
        w,
        y_base
    )
}

/// Deflate: the baseline dips down in the middle (pinched shape).
fn deflate_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let y_base = h * (0.15 + t * 0.7);
    let intensity = adj.map(|a| clamp4(a as f64 / 18750.0)).unwrap_or(1.0);
    let base = h * 0.18 * intensity;
    let extra = h * 0.08 * intensity * (t - 0.5);
    let dip = base + extra;
    format!(
        "M 0,{} Q {},{} {},{}",
        y_base,
        w / 2.0,
        y_base + dip,
        w,
        y_base
    )
}

/// Curve up: gentle upward curve.
fn curve_up_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let y_base = h * (0.35 + t * 0.55);
    let adj_factor = adj.map(|a| a as f64 / 45977.0).unwrap_or(1.0);
    let curve = h * 0.4 * (1.0 - t * 0.3) * clamp4(adj_factor);
    format!(
        "M 0,{} Q {},{} {},{}",
        y_base,
        w / 2.0,
        y_base - curve,
        w,
        y_base
    )
}

/// Curve down: gentle downward curve.
fn curve_down_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let y_base = h * (0.1 + t * 0.55);
    let adj_factor = adj.map(|a| a as f64 / 45977.0).unwrap_or(1.0);
    let curve = h * 0.4 * (1.0 - (1.0 - t) * 0.3) * clamp4(adj_factor);
    format!(
        "M 0,{} Q {},{} {},{}",
        y_base,
        w / 2.0,
        y_base + curve,
        w,
        y_base
    )
}

// ── Priority 2 ──────────────────────────────────────────────────────────

/// Can up: cylindrical text, upward.
fn can_up_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let adj_factor = adj.map(|a| a as f64 / 18750.0).unwrap_or(1.0);
    let curvature = clamp4(adj_factor);
    let arch_h = h * (0.35 - t * 0.25) * curvature;
    if arch_h < 1.0 {
        format!("M 0,{} L {},{}", h, w, h)
    } else {
        format!("M 0,{} A {},{} 0 0,1 {},{}", h, w / 2.0, arch_h, w, h)
    }
}

/// Can down: cylindrical text, downward.
fn can_down_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let adj_factor = adj.map(|a| a as f64 / 18750.0).unwrap_or(1.0);
    let curvature = clamp4(adj_factor);
    let arch_h = h * (0.1 + t * 0.25) * curvature;
    if arch_h < 1.0 {
        format!("M 0,0 L {},0", w)
    } else {
        format!("M 0,0 A {},{} 0 0,0 {},{}", w / 2.0, arch_h, w, 0)
    }
}

// ── Priority 4 (slant, fade, cascade) ───────────────────────────────────

/// Slant up: baseline rises from left to right.
fn slant_up_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let adj_factor = adj.map(|a| a as f64 / 55000.0).unwrap_or(1.0);
    let slant = 0.25 * clamp4(adj_factor);
    let y_mid = h * (0.175 + t * 0.55);
    let y_start = y_mid + (h * slant) / 2.0;
    let y_end = y_mid - (h * slant) / 2.0;
    format!("M 0,{} L {},{}", y_start, w, y_end)
}

/// Slant down: baseline falls from left to right.
fn slant_down_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let adj_factor = adj.map(|a| a as f64 / 55000.0).unwrap_or(1.0);
    let slant = 0.25 * clamp4(adj_factor);
    let y_mid = h * (0.175 + t * 0.55);
    let y_start = y_mid - (h * slant) / 2.0;
    let y_end = y_mid + (h * slant) / 2.0;
    format!("M 0,{} L {},{}", y_start, w, y_end)
}

/// Fade right: baseline tilts down towards the right edge.
fn fade_right_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let intensity = adj.map(|a| clamp4(a as f64 / 50000.0)).unwrap_or(1.0);
    let y_left = h * (0.1 + t * 0.8);
    let squeeze = 0.32 * intensity;
    let y_right = h * (0.5 + squeeze * 0.4);
    format!("M 0,{} L {},{}", y_left, w, y_right)
}

/// Fade left: mirror of fade_right_path.
fn fade_left_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let intensity = adj.map(|a| clamp4(a as f64 / 50000.0)).unwrap_or(1.0);
    let squeeze = 0.32 * intensity;
    let y_left = h * (0.5 + squeeze * 0.4);
    let y_right = h * (0.1 + t * 0.8);
    format!("M 0,{} L {},{}", y_left, w, y_right)
}

/// Cascade up: lines tilt from lower-left to upper-right.
fn cascade_up_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let adj_factor = adj.map(|a| a as f64 / 44444.0).unwrap_or(1.0);
    let tilt = 0.2 * clamp4(adj_factor);
    let y_mid = h * (0.2 + t * 0.6);
    let y_start = y_mid + (h * tilt) / 2.0;
    let y_end = y_mid - (h * tilt) / 2.0;
    format!("M 0,{} L {},{}", y_start, w, y_end)
}

/// Cascade down: lines tilt from upper-left to lower-right.
fn cascade_down_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let adj_factor = adj.map(|a| a as f64 / 44444.0).unwrap_or(1.0);
    let tilt = 0.2 * clamp4(adj_factor);
    let y_mid = h * (0.2 + t * 0.6);
    let y_start = y_mid - (h * tilt) / 2.0;
    let y_end = y_mid + (h * tilt) / 2.0;
    format!("M 0,{} L {},{}", y_start, w, y_end)
}

/// Fade up: text narrows towards the top.
fn fade_up_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let adj_factor = adj.map(|a| a as f64 / 50000.0).unwrap_or(1.0);
    let taper_scale = clamp4(adj_factor);
    let narrow_fraction = 1.0 - 0.7 * taper_scale;
    let narrow_w = w * narrow_fraction.max(0.0);
    let line_w = narrow_w + t * (w - narrow_w);
    let x_start = (w - line_w) / 2.0;
    let y_base = h * (0.1 + t * 0.8);
    format!("M {},{} L {},{}", x_start, y_base, x_start + line_w, y_base)
}

/// Fade down: text narrows towards the bottom.
fn fade_down_path(w: f64, h: f64, t: f64, adj: Option<i32>) -> String {
    let adj_factor = adj.map(|a| a as f64 / 50000.0).unwrap_or(1.0);
    let taper_scale = clamp4(adj_factor);
    let narrow_fraction = 1.0 - 0.7 * taper_scale;
    let narrow_w = w * narrow_fraction.max(0.0);
    let line_w = w - t * (w - narrow_w);
    let x_start = (w - line_w) / 2.0;
    let y_base = h * (0.1 + t * 0.8);
    format!("M {},{} L {},{}", x_start, y_base, x_start + line_w, y_base)
}

// ── Public API ──────────────────────────────────────────────────────────

/// Build the SVG path `d` attribute for a warp preset at a given line position.
///
/// # Arguments
/// * `preset` - OOXML `prstTxWarp` preset name
/// * `width` - Box width in px
/// * `height` - Box height in px
/// * `line_index` - Zero-based index of the line/paragraph
/// * `line_count` - Total number of lines/paragraphs
/// * `adj` - Optional primary adjustment (raw OOXML 1/60000th units)
/// * `adj2` - Optional secondary adjustment (raw OOXML 1/60000th units)
pub fn build_warp_path(
    preset: &str,
    width: f64,
    height: f64,
    line_index: usize,
    line_count: usize,
    adj: Option<i32>,
    _adj2: Option<i32>,
) -> Option<String> {
    let t = if line_count <= 1 {
        0.5
    } else {
        line_index as f64 / (line_count - 1) as f64
    };

    match preset {
        "textArchUp" => Some(arch_up_path(width, height, t, adj)),
        "textArchDown" => Some(arch_down_path(width, height, t, adj)),
        "textCircle" => Some(circle_path(width, height, t, adj)),
        "textInflate" => Some(inflate_path(width, height, t, adj)),
        "textDeflate" => Some(deflate_path(width, height, t, adj)),
        "textCurveUp" => Some(curve_up_path(width, height, t, adj)),
        "textCurveDown" => Some(curve_down_path(width, height, t, adj)),
        "textCanUp" => Some(can_up_path(width, height, t, adj)),
        "textCanDown" => Some(can_down_path(width, height, t, adj)),
        "textSlantUp" => Some(slant_up_path(width, height, t, adj)),
        "textSlantDown" => Some(slant_down_path(width, height, t, adj)),
        "textFadeRight" => Some(fade_right_path(width, height, t, adj)),
        "textFadeLeft" => Some(fade_left_path(width, height, t, adj)),
        "textCascadeUp" => Some(cascade_up_path(width, height, t, adj)),
        "textCascadeDown" => Some(cascade_down_path(width, height, t, adj)),
        "textFadeUp" => Some(fade_up_path(width, height, t, adj)),
        "textFadeDown" => Some(fade_down_path(width, height, t, adj)),
        _ => None,
    }
}

/// Predicate: should a warped shape render through SVG textPath / envelope
/// instead of flat text? (upstream `shouldUseSvgWarp` + glyph-envelope set).
pub fn should_use_svg_warp(preset: &str) -> bool {
    matches!(
        preset,
        "textArchUp"
            | "textArchDown"
            | "textCircle"
            | "textInflate"
            | "textDeflate"
            | "textCurveUp"
            | "textCurveDown"
            | "textCanUp"
            | "textCanDown"
            | "textSlantUp"
            | "textSlantDown"
            | "textFadeRight"
            | "textFadeLeft"
            | "textCascadeUp"
            | "textCascadeDown"
            | "textFadeUp"
            | "textFadeDown"
            | "textInflateBottom"
            | "textInflateTop"
            | "textDeflateBottom"
            | "textDeflateTop"
            | "textDeflateInflate"
            | "textDeflateInflateDeflate"
    )
}

/// Upstream `getEnvelopeCssTransform` — CSS hint for envelope presets.
#[allow(dead_code)]
pub fn envelope_css_transform(
    preset: &str,
    adj1: Option<i32>,
    _adj2: Option<i32>,
) -> Option<(String, String)> {
    let a1 = adj1.unwrap_or(18_750) as f64;
    let intensity = (a1 / 18_750.0).clamp(0.0, 4.0);
    let (transform, origin) = match preset {
        "textInflate" => (
            format!(
                "scaleY({}) scaleX({})",
                1.0 + 0.15 * intensity,
                1.0 + 0.05 * intensity
            ),
            "center center",
        ),
        "textInflateBottom" => (
            format!(
                "perspective({}px) rotateX({}deg)",
                600.0 - 100.0 * intensity,
                -8.0 * intensity
            ),
            "center bottom",
        ),
        "textInflateTop" => (
            format!(
                "perspective({}px) rotateX({}deg)",
                600.0 - 100.0 * intensity,
                8.0 * intensity
            ),
            "center top",
        ),
        "textDeflate" => (
            format!(
                "scaleY({}) scaleX({})",
                1.0 - 0.12 * intensity,
                1.0 - 0.05 * intensity
            ),
            "center center",
        ),
        "textDeflateBottom" => (
            format!(
                "perspective({}px) rotateX({}deg)",
                600.0 - 100.0 * intensity,
                6.0 * intensity
            ),
            "center bottom",
        ),
        "textDeflateTop" => (
            format!(
                "perspective({}px) rotateX({}deg)",
                600.0 - 100.0 * intensity,
                -6.0 * intensity
            ),
            "center top",
        ),
        "textDeflateInflate" => (
            format!(
                "scaleY({}) scaleX({})",
                1.0 - 0.08 * intensity,
                1.0 + 0.04 * intensity
            ),
            "center center",
        ),
        "textDeflateInflateDeflate" => (
            format!(
                "scaleY({}) scaleX({})",
                1.0 - 0.15 * intensity,
                1.0 + 0.06 * intensity
            ),
            "center center",
        ),
        "textCanUp" => (
            format!(
                "perspective({}px) rotateX({}deg)",
                500.0 - 80.0 * intensity,
                -6.0 * intensity
            ),
            "center center",
        ),
        "textCanDown" => (
            format!(
                "perspective({}px) rotateX({}deg)",
                500.0 - 80.0 * intensity,
                6.0 * intensity
            ),
            "center center",
        ),
        _ => return None,
    };
    Some((transform, origin.to_owned()))
}
