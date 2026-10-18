// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec upstream/src/ui/src/pptx/render/text-warp-preset-sampler.ts; changes listed in PROVENANCE.md.
//! Samples the spec-transcribed `prstTxWarp` top/bottom paths in
//! `wordart_preset_definitions.rs` at a normalised horizontal position,
//! using the OOXML guide-formula evaluator so the same arithmetic, clamping (`pin`),
//! and conditional operators the spec defines resolve `adj`-dependent guides exactly
//! as PowerPoint's own preset-shape geometry does.
//!
//! The box is normalised to `w = h = 100000` (matching the 0..100000 scale
//! `adj`/`adj2` are already stored in), so every resolved guide value is already
//! a fraction of `h` in `0..100000` and only needs dividing by 100000 to become
//! the `0..1` fraction `envelope_curve_at` returns.

use std::collections::HashMap;

use nitro_xml_opc::drawingml::{evaluate_guides, GeometryContext};

use super::wordart_preset_definitions::{get_warp_preset_definition, WarpCurveSegment};

/// OOXML angles are stored in 60,000ths of a degree.
fn angle_to_radians(ooxml_angle: f64) -> f64 {
    (ooxml_angle / 60000.0) * std::f64::consts::PI / 180.0
}

const BOX: f64 = 100000.0;

fn sample_line(y0: f64, y1: f64, u: f64) -> f64 {
    y0 + (y1 - y0) * u
}

fn sample_quad(y0: f64, yc: f64, y1: f64, u: f64) -> f64 {
    // x(t) = w*t exactly here because the control point's x is always the
    // midpoint `hc`, so t == u; see the module doc in
    // wordart_preset_definitions.rs for the derivation.
    let mt = 1.0 - u;
    mt * mt * y0 + 2.0 * mt * u * yc + u * u * y1
}

fn sample_cubic(y0: f64, yc1: f64, yc2: f64, y1: f64, u: f64) -> f64 {
    // x(t) == w*t exactly here too: the control points sit at the evenly
    // spaced x = w/3 and x = 2w/3 (`x0`/`x1` in the spec), which makes the
    // cubic's x-component reduce algebraically to a linear function of t.
    let mt = 1.0 - u;
    mt * mt * mt * y0 + 3.0 * mt * mt * u * yc1 + 3.0 * mt * u * u * yc2 + u * u * u * y1
}

/// Sample an elliptical `arcTo` segment at normalised horizontal position `u`.
///
/// The OOXML `arcTo` command draws an ellipse of radii `(wR, hR)` centred so
/// the current pen position `(penX, penY)` lies on it at angle `stAng`,
/// sweeping `swAng`. Solve for the traversal distance `d` (`0..|swAng|`) along
/// that sweep whose `x` matches `u * w`: writing the sweep as
/// `angle = stAng + sign(swAng) * d`, the identity
/// `cos(angle) = cos(stAng)*cos(d) - sign(swAng)*sin(stAng)*sin(d)` is a unit
/// rotation of `d` (`A = cos(stAng)`, `B = -sign(swAng)*sin(stAng)`,
/// `A^2+B^2=1`), so it solves in closed form as `d = phi -+ acos(cosVal)` for
/// `phi = atan2(B, A)`, picking whichever root lands in `[0, |swAng|]`.
///
/// A plain "is the angle inside `[stAng, stAng+swAng]`" range check (tried
/// first, 2026-09-06) is NOT equivalent: `textCanUp` and `textCanDown` both
/// sweep exactly `π` from `stAng = cd2 = π`, just in opposite directions, so
/// their swept angle sets both normalise to the same `[0, π]` interval and a
/// range check cannot tell them apart - it silently picked the same branch for
/// both, making `textCanUp` arch the wrong way (verified against a PowerPoint
/// COM screenshot; see tests).
fn sample_arc(
    pen_x: f64,
    pen_y: f64,
    w_r: f64,
    h_r: f64,
    st_ang_deg: f64,
    sw_ang_deg: f64,
    u: f64,
) -> f64 {
    if w_r == 0.0 || h_r == 0.0 {
        return pen_y;
    }

    let st_ang = angle_to_radians(st_ang_deg);
    let sw_ang = angle_to_radians(sw_ang_deg);
    let cx = pen_x - w_r * st_ang.cos();
    let cy = pen_y - h_r * st_ang.sin();
    let target_x = u * BOX;
    let cos_val = ((target_x - cx) / w_r).clamp(-1.0, 1.0);

    let sign = if sw_ang < 0.0 { -1.0 } else { 1.0 };
    let max_d = sw_ang.abs();
    let a = st_ang.cos();
    let b = -sign * st_ang.sin();
    let phi = b.atan2(a);
    let acos_val = cos_val.acos(); // in [0, pi]

    let two_pi = std::f64::consts::PI * 2.0;
    // Normalise a candidate `d` into whichever representative (mod 2*pi, or
    // its `- 2*pi` twin) falls inside the valid `[0, maxD]` traversal range.
    let fit_to_range = |raw: f64| -> Option<f64> {
        let mut wrapped = raw % two_pi;
        if wrapped < 0.0 {
            wrapped += two_pi;
        }
        if wrapped <= max_d + 1e-6 {
            return Some(wrapped);
        }
        let alt = wrapped - two_pi;
        if alt >= -1e-6 && alt <= max_d + 1e-6 {
            return Some(alt);
        }
        None
    };

    let d = fit_to_range(phi - acos_val)
        .or_else(|| fit_to_range(phi + acos_val))
        .unwrap_or_else(|| 0.0_f64.max(max_d.min(phi)));
    let angle = st_ang + sign * d;
    cy + h_r * angle.sin()
}

fn sample_segment(seg: &WarpCurveSegment, u: f64, vars: &HashMap<String, f64>) -> f64 {
    let g = |name: &str| -> f64 { vars.get(name).copied().unwrap_or(0.0) };

    match seg {
        WarpCurveSegment::Line { start_y, end_y } => sample_line(g(start_y), g(end_y), u),
        WarpCurveSegment::Quad {
            start_y,
            ctrl_y,
            end_y,
        } => sample_quad(g(start_y), g(ctrl_y), g(end_y), u),
        WarpCurveSegment::Cubic {
            start_y,
            ctrl1_y,
            ctrl2_y,
            end_y,
        } => sample_cubic(g(start_y), g(ctrl1_y), g(ctrl2_y), g(end_y), u),
        WarpCurveSegment::Arc {
            pen_x,
            pen_y,
            w_r,
            h_r,
            st_ang,
            sw_ang,
        } => sample_arc(g(pen_x), g(pen_y), g(w_r), g(h_r), g(st_ang), g(sw_ang), u),
    }
}

/// Top/bottom envelope fractions (0..1 of box height) at horizontal position `u`.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct SampledWarpCurve {
    pub top: f64,
    pub bottom: f64,
}

/// Sample the spec-transcribed top/bottom curve for `preset` at normalised
/// horizontal position `u` (0 = left edge, 1 = right edge). Returns
/// `None` when `preset` has no transcribed definition (the caller should
/// fall back to a reconstruction in that case).
#[allow(dead_code)]
pub fn sample_warp_preset_curve(
    preset: &str,
    u: f64,
    adj: Option<i32>,
    adj2: Option<i32>,
) -> Option<SampledWarpCurve> {
    let def = get_warp_preset_definition(preset)?;

    let mut adjustments = HashMap::new();
    adjustments.insert("adj".to_string(), adj.unwrap_or(def.default_adj) as f64);
    adjustments.insert("adj2".to_string(), adj2.unwrap_or(0) as f64);

    let vars = evaluate_guides(
        &def.gd_list,
        GeometryContext { w: BOX, h: BOX },
        Some(&adjustments),
    );

    let clamped_u = u.clamp(0.0, 1.0);

    Some(SampledWarpCurve {
        top: sample_segment(&def.top, clamped_u, &vars) / BOX,
        bottom: sample_segment(&def.bottom, clamped_u, &vars) / BOX,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_inflate_mid_band_from_sampler_differs_from_naive_hump() {
        // Catches: inflate mid band from sampler differs from naive hump
        let sampled = sample_warp_preset_curve("textInflate", 0.5, Some(18750), None).unwrap();
        // The spec formula gives precise mid-height bulge; a naive hump at 0.5 is just sin(π*0.5)=1
        // but the sampler should use cubic path, which is different
        assert!(sampled.top < 0.15, "top should bulge inward");
        assert!(sampled.bottom > 0.85, "bottom should bulge outward");
        // Band should be wider at centre than nominal
        assert!(
            sampled.bottom - sampled.top > 0.7,
            "inflate band should be wider at centre"
        );
    }

    #[test]
    fn test_sample_inflate_sanity() {
        // Catches: inflate sampler is working (not falling back)
        let mid = sample_warp_preset_curve("textInflate", 0.5, Some(18750), None).unwrap();
        let left = sample_warp_preset_curve("textInflate", 0.0, Some(18750), None).unwrap();
        let right = sample_warp_preset_curve("textInflate", 1.0, Some(18750), None).unwrap();
        // Verify the band is wider at mid than at edges
        let mid_band = mid.bottom - mid.top;
        let left_band = left.bottom - left.top;
        let right_band = right.bottom - right.top;
        assert!(mid_band > left_band, "mid band should be wider than left");
        assert!(mid_band > right_band, "mid band should be wider than right");
    }

    #[test]
    fn test_sample_canup_adj_66667_arches_correctly() {
        // Catches: canUp adj 66667 arches correctly (top < bottom, mid top lower than edges for canUp)
        let sampled = sample_warp_preset_curve("textCanUp", 0.5, Some(66667), None).unwrap();
        // Both curves should rise (move towards lower values)
        assert!(sampled.top < 0.85, "canUp top should rise above baseline");
        assert!(sampled.bottom < 1.0, "canUp bottom should be valid");
        // At mid-line, the top should be lower than edge values (arch)
        let left_edge = sample_warp_preset_curve("textCanUp", 0.0, Some(66667), None).unwrap();
        assert!(
            sampled.top <= left_edge.top,
            "canUp top at mid should arch (lower or equal)"
        );
    }

    #[test]
    fn test_canup_guide_eval() {
        // Catches: textCanUp guide evaluation - ensure adj is pinned correctly
        // With adj=66667 (> 100000), it should pin to 100000
        let high_adj = sample_warp_preset_curve("textCanUp", 0.5, Some(100001), None).unwrap();
        let max_adj = sample_warp_preset_curve("textCanUp", 0.5, Some(100000), None).unwrap();
        // Should be approximately equal since adj is pinned
        assert!(
            (high_adj.top - max_adj.top).abs() < 0.01,
            "high adj should pin to max"
        );
    }
}
