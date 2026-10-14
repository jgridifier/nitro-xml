// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/shape-geometry-cascade.ts,
// src/core/pptx/core/geometry/shape-geometry.ts, src/core/geometry/preset-clip-paths-core.ts;
// changes listed in PROVENANCE.md.

use nitro_xml_opc::drawingml::evaluate_preset_shape;

use super::context::{PaintContext, StringIntern};
use super::dom::set_style;

/// OOXML default `roundRect` adjustment (16667 / 50000).
const ROUND_RECT_DEFAULT_ADJ: f64 = 16667.0 / 50000.0;

const MIN_ROUND_RECT_RADIUS_PX: f64 = 0.01;

/// Apply CSS geometry for `a:prstGeom` (border-radius or clip-path), matching upstream cascade order.
pub fn apply_preset_geometry<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    id: u32,
    shape_type: &str,
    width: f64,
    height: f64,
) {
    let normalized = shape_type.to_ascii_lowercase();
    if normalized == "rect" {
        return;
    }

    if is_round_rect_family(&normalized) {
        let radius = round_rect_radius_px(width, height, None);
        if radius > MIN_ROUND_RECT_RADIUS_PX {
            set_style(ctx, id, "border-radius", &format_radius_px(radius));
        }
        return;
    }

    if normalized == "ellipse" || normalized == "oval" {
        set_style(ctx, id, "border-radius", "50%");
        return;
    }

    if normalized == "cylinder" || normalized == "can" {
        set_style(ctx, id, "border-radius", "48% / 12%");
        return;
    }

    if let Some(clip) = preset_clip_path(&normalized) {
        set_style(ctx, id, "clip-path", clip);
        set_style(ctx, id, "overflow", "hidden");
    }
}

fn is_round_rect_family(normalized: &str) -> bool {
    matches!(
        normalized,
        "roundrect"
            | "round1rect"
            | "round2samerect"
            | "round2diagrect"
            | "sniproundrect"
            | "snip1rect"
            | "snip2diagrect"
    )
}

fn round_rect_radius_px(width: f64, height: f64, adj: Option<i64>) -> f64 {
    let normalized_adj = adj
        .map(|a| (a.clamp(0, 50_000) as f64) / 50_000.0)
        .unwrap_or(ROUND_RECT_DEFAULT_ADJ);
    let short_side = width.max(1.0).min(height.max(1.0));
    short_side * 0.5 * normalized_adj
}

fn format_radius_px(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}px", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        format!("{}px", s.trim_end_matches('0').trim_end_matches('.'))
    }
}

/// Centred stroke overlay path from the OOXML preset evaluator (upstream `outlinePathData`).
pub fn preset_stroke_outline_path(shape_type: &str, width: f64, height: f64) -> Option<String> {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let normalized = shape_type.to_ascii_lowercase();
    if normalized == "rect" || normalized == "line" {
        return None;
    }
    evaluate_preset_shape(shape_type, width, height, None)
        .map(|eval| eval.svg_path)
        .filter(|path| !path.is_empty())
}

/// Static percentage polygons from upstream `preset-clip-paths-core.ts` (subset for poi-keyframes + common nav).
fn preset_clip_path(normalized: &str) -> Option<&'static str> {
    match normalized {
        "rightarrow" | "rtarrow" => {
            Some("polygon(0% 20%, 60% 20%, 60% 0%, 100% 50%, 60% 100%, 60% 80%, 0% 80%)")
        }
        "stripedrightarrow" => Some(
            "polygon(0% 25%, 8% 25%, 12% 25%, 12% 20%, 60% 20%, 60% 0%, 100% 50%, 60% 100%, 60% 80%, 12% 80%, 12% 75%, 8% 75%, 0% 75%)",
        ),
        "leftarrow" => {
            Some("polygon(40% 0%, 40% 20%, 100% 20%, 100% 80%, 40% 80%, 40% 100%, 0% 50%)")
        }
        "uparrow" => {
            Some("polygon(50% 0%, 100% 40%, 80% 40%, 80% 100%, 20% 100%, 20% 40%, 0% 40%)")
        }
        "downarrow" => {
            Some("polygon(20% 0%, 80% 0%, 80% 60%, 100% 60%, 50% 100%, 0% 60%, 20% 60%)")
        }
        "notchedrightarrow" => Some(
            "polygon(0% 20%, 60% 20%, 60% 0%, 100% 50%, 60% 100%, 60% 80%, 0% 80%, 15% 50%)",
        ),
        "chevron" => Some("polygon(0% 0%, 75% 0%, 100% 50%, 75% 100%, 0% 100%, 25% 50%)"),
        "homeplate" => Some("polygon(0% 0%, 80% 0%, 100% 50%, 80% 100%, 0% 100%)"),
        "triangle" => Some("polygon(50% 0%, 100% 100%, 0% 100%)"),
        "diamond" => Some("polygon(50% 0%, 100% 50%, 50% 100%, 0% 50%)"),
        "pentagon" => Some("polygon(50% 0%, 100% 38%, 82% 100%, 18% 100%, 0% 38%)"),
        "hexagon" => {
            Some("polygon(25% 0%, 75% 0%, 100% 50%, 75% 100%, 25% 100%, 0% 50%)")
        }
        "octagon" => Some(
            "polygon(30% 0%, 70% 0%, 100% 30%, 100% 70%, 70% 100%, 30% 100%, 0% 70%, 0% 30%)",
        ),
        "parallelogram" => Some("polygon(20% 0%, 100% 0%, 80% 100%, 0% 100%)"),
        "trapezoid" => Some("polygon(20% 0%, 80% 0%, 100% 100%, 0% 100%)"),
        // Upstream preset-clip-paths-core / -extended (be4aaa soft band).
        "pie" => Some("polygon(50% 50%, 50% 0%, 100% 0%, 100% 100%, 50% 100%)"),
        "piewedge" => Some("polygon(100% 0%, 100% 100%, 0% 100%, 0% 55%, 55% 0%)"),
        "star5" => Some(
            "polygon(50% 0%, 61% 35%, 98% 35%, 68% 57%, 79% 91%, 50% 70%, 21% 91%, 32% 57%, 2% 35%, 39% 35%)",
        ),
        "flowchartdecision" => Some("polygon(50% 0%, 100% 50%, 50% 100%, 0% 50%)"),
        "actionbuttonhome" => Some("inset(0 round 6px)"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{preset_clip_path, round_rect_radius_px};

    // Catches: roundRect menu buttons painted square when default OOXML adj radius is skipped.
    #[test]
    fn round_rect_default_radius_uses_ooxml_adj() {
        let r = round_rect_radius_px(207.0, 57.0, None);
        assert!(r > 8.0 && r < 12.0);
    }

    // Catches: stripedRightArrow nav control clipped to a filled axis-aligned rectangle.
    #[test]
    fn striped_right_arrow_has_clip_polygon() {
        assert!(preset_clip_path("stripedrightarrow").is_some());
    }

    // Catches: be4aaa pie/star5/decision/actionHome fill as axis-aligned rects (no CSS clip).
    #[test]
    fn be4aaa_presets_have_clip_polygons() {
        for name in [
            "pie",
            "star5",
            "flowchartdecision",
            "actionbuttonhome",
        ] {
            assert!(
                preset_clip_path(name).is_some(),
                "missing clip for {name}"
            );
        }
    }
}

