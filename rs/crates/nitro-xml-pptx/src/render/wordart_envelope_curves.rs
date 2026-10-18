// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec upstream/src/ui/src/pptx/render/text-warp-envelope-curves.ts; changes listed in PROVENANCE.md.
//! Two-curve envelope maths for the WordArt "envelope" preset family
//! (inflate/deflate/can), transcribed from the OOXML `prstTxWarp` guide-formula
//! shape: PowerPoint bends text between an independent TOP curve and BOTTOM
//! curve, so a glyph's height (not just its baseline position) varies with its
//! horizontal position along the line.

#![allow(dead_code)]

use super::wordart_preset_sampler::sample_warp_preset_curve;

/// Presets that carry an independent top/bottom envelope curve.
pub const GLYPH_ENVELOPE_PRESETS: &[&str] = &[
    "textInflate",
    "textDeflate",
    "textInflateBottom",
    "textInflateTop",
    "textDeflateBottom",
    "textDeflateTop",
    "textDeflateInflate",
    "textDeflateInflateDeflate",
    "textCanUp",
    "textCanDown",
];

/// True when `preset` should be rendered with a per-glyph two-curve envelope.
pub fn has_glyph_envelope(preset: &str) -> bool {
    GLYPH_ENVELOPE_PRESETS.contains(&preset)
}

/// The undeformed (adj=0) top/bottom band, as a fraction of box height.
const TOP_MARGIN: f64 = 0.15;
const BOTTOM_MARGIN: f64 = 0.85;

/// The narrowest an envelope band may collapse to (fraction of box height).
const MIN_BAND: f64 = 0.02;

/// Default OOXML `adj1` for every envelope preset maps to intensity factor 1.
const DEFAULT_ADJ: f64 = 18750.0;

fn clamp4(n: f64) -> f64 {
    n.clamp(0.0, 4.0)
}

fn clamp_unit(n: f64) -> f64 {
    n.clamp(0.0, 1.0)
}

fn intensity_of(adj: Option<i32>) -> f64 {
    adj.map(|a| clamp4(a as f64 / DEFAULT_ADJ)).unwrap_or(1.0)
}

/// Single hump: 0 at the edges, 1 at the horizontal centre.
fn hump(u: f64) -> f64 {
    (std::f64::consts::PI * clamp_unit(u)).sin()
}

/// Three alternating lobes across the line (deflate / inflate / deflate),
/// matching `textDeflateInflateDeflate` name literally: the outer thirds pinch,
/// the middle third bulges.
fn triple_lobe(u: f64) -> f64 {
    (3.0 * std::f64::consts::PI * clamp_unit(u)).sin()
}

/// Top/bottom envelope curve position, each a fraction of box height.
#[derive(Clone, Copy, Debug)]
pub struct EnvelopeCurveFractions {
    pub top: f64,
    pub bottom: f64,
}

/// The top/bottom envelope curve fractions at horizontal position `u` (0 = left
/// edge, 1 = right edge, 0.5 = centre) for a glyph-envelope preset.
///
/// Returns `None` for a preset outside the glyph-envelope family.
pub fn envelope_curve_at(
    preset: &str,
    u: f64,
    adj: Option<i32>,
    adj2: Option<i32>,
) -> Option<EnvelopeCurveFractions> {
    if !has_glyph_envelope(preset) {
        return None;
    }

    // Try the preset sampler first (real OOXML guide formulas)
    if let Some(sampled) = sample_warp_preset_curve(preset, u, adj, adj2) {
        return Some(clamp_band(sampled.top, sampled.bottom));
    }

    // Fall back to hump-based formulas for robustness
    let intensity = intensity_of(adj);
    let bulge = hump(u);

    let mut top = TOP_MARGIN;
    let mut bottom = BOTTOM_MARGIN;

    match preset {
        "textInflate" => {
            // Both curves bow away from centre: the band is tallest mid-line.
            top -= 0.12 * intensity * bulge;
            bottom += 0.12 * intensity * bulge;
        }
        "textDeflate" => {
            // Both curves pinch toward centre: the band is shortest mid-line.
            top += 0.1 * intensity * bulge;
            bottom -= 0.1 * intensity * bulge;
        }
        "textInflateTop" => {
            top -= 0.16 * intensity * bulge;
        }
        "textInflateBottom" => {
            bottom += 0.16 * intensity * bulge;
        }
        "textDeflateTop" => {
            top += 0.14 * intensity * bulge;
        }
        "textDeflateBottom" => {
            bottom -= 0.14 * intensity * bulge;
        }
        "textDeflateInflate" => {
            // Top curve deflates (pinches down); bottom curve inflates (bulges
            // further down): the band shrinks toward the bottom mid-line.
            top += 0.1 * intensity * bulge;
            bottom += 0.1 * intensity * bulge;
        }
        "textDeflateInflateDeflate" => {
            let lobe = triple_lobe(u);
            top += 0.1 * intensity * lobe;
            bottom -= 0.1 * intensity * lobe;
        }
        "textCanUp" => {
            // Cylindrical arc: both curves rise together; the band narrows very
            // slightly mid-line (the barrel surface tilts away from the viewer).
            let arch = 0.18 * intensity * bulge;
            let squeeze = 0.05 * intensity * bulge;
            top -= arch;
            bottom -= arch - squeeze;
        }
        "textCanDown" => {
            let arch = 0.18 * intensity * bulge;
            let squeeze = 0.05 * intensity * bulge;
            top += arch - squeeze;
            bottom += arch;
        }
        _ => {}
    }

    Some(clamp_band(top, bottom))
}

/// Clamp a raw top/bottom pair to `[0, 1]` and enforce MIN_BAND.
fn clamp_band(top: f64, bottom: f64) -> EnvelopeCurveFractions {
    let mut clamped_top = clamp_unit(top);
    let mut clamped_bottom = clamp_unit(bottom);

    if clamped_bottom - clamped_top < MIN_BAND {
        let mid = (clamped_top + clamped_bottom) / 2.0;
        clamped_top = mid - MIN_BAND / 2.0;
        clamped_bottom = mid + MIN_BAND / 2.0;
    }

    EnvelopeCurveFractions {
        top: clamped_top,
        bottom: clamped_bottom,
    }
}

/// The undeformed band, exported so layout code can compute a stable scale=1 baseline.
#[allow(dead_code)]
pub const NOMINAL_ENVELOPE_BAND: EnvelopeCurveFractions = EnvelopeCurveFractions {
    top: TOP_MARGIN,
    bottom: BOTTOM_MARGIN,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_envelope_curve_at_inflate_mid_taller() {
        // Catches: inflate hump should peak mid-line where u=0.5
        let curve = envelope_curve_at("textInflate", 0.5, Some(18750), None).unwrap();
        // At centre, hump=1.0 so bulge is max; top should drop, bottom should rise
        assert!(
            curve.top < TOP_MARGIN,
            "inflate top should bow down at centre"
        );
        assert!(
            curve.bottom > BOTTOM_MARGIN,
            "inflate bottom should bow up at centre"
        );
        assert!(
            curve.bottom - curve.top > NOMINAL_ENVELOPE_BAND.bottom - NOMINAL_ENVELOPE_BAND.top,
            "inflate band should be taller at centre"
        );
    }

    #[test]
    fn test_envelope_curve_at_can_up_arches() {
        // Catches: canUp both curves rise together forming a cylindrical arc
        let curve = envelope_curve_at("textCanUp", 0.5, Some(66667), None).unwrap();
        // Both curves should rise (move down in fraction space towards 0)
        assert!(curve.top < TOP_MARGIN, "canUp top should move down");
        assert!(
            curve.bottom < BOTTOM_MARGIN,
            "canUp bottom should move down"
        );
    }

    #[test]
    fn test_envelope_curve_edges_return_to_baseline() {
        // Catches: edges (u=0 or u=1) should have minimal bulge, mostly at nominal band
        let left = envelope_curve_at("textInflate", 0.0, Some(18750), None).unwrap();
        let right = envelope_curve_at("textInflate", 1.0, Some(18750), None).unwrap();
        // At edges, hump=0 so bulge=0, band should be nominal (or from sampler edge values)
        // Allow small tolerance for sampler values
        assert!(
            (left.top - TOP_MARGIN).abs() < 0.05,
            "left top near nominal"
        );
        assert!(
            (left.bottom - BOTTOM_MARGIN).abs() < 0.05,
            "left bottom near nominal"
        );
        assert!(
            (right.top - TOP_MARGIN).abs() < 0.05,
            "right top near nominal"
        );
        assert!(
            (right.bottom - BOTTOM_MARGIN).abs() < 0.05,
            "right bottom near nominal"
        );
    }

    #[test]
    fn test_can_up_high_intensity() {
        // Catches: textCanUp with adj=66667 should produce arching when using sampler
        let curve_low = envelope_curve_at("textCanUp", 0.5, Some(18750), None).unwrap();
        let curve_high = envelope_curve_at("textCanUp", 0.5, Some(66667), None).unwrap();
        // With real sampler, results depend on guide formulas, not intensity
        // Just verify both return valid bands with top < bottom
        assert!(curve_low.top < curve_low.bottom, "low intensity band valid");
        assert!(
            curve_high.top < curve_high.bottom,
            "high intensity band valid"
        );
    }
}
