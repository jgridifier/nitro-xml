// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/text-warp-glyph-outline.ts; changes listed in PROVENANCE.md.
//! Per-point glyph-outline warping for WordArt envelope presets.
//!
//! Given traced glyph contours (from the host), flatten each contour into
//! segments short enough for accurate warping (no longer than `MAX_SEGMENT`),
//! then map each point through the envelope warp to produce the final SVG
//! path `d` string for rendering as a filled path.

use crate::render::wordart_envelope_map::EnvelopeWarp;

/// Sub-segments per Bezier curve when flattening (upstream CURVE_STEPS).
const CURVE_STEPS: usize = 8;

/// Maximum segment length (layout units) when flattening contours.
pub const MAX_SEGMENT: f64 = 0.1;

/// A 2D point in outline space.
#[derive(Clone, Copy, Debug)]
pub struct OutlinePoint {
    pub x: f64,
    pub y: f64,
}

/// Flatten contours into closed polylines, splitting segments so no piece
/// is longer than `max_segment`.
fn flatten_outline(contours: &[Vec<OutlinePoint>], max_segment: f64) -> Vec<Vec<OutlinePoint>> {
    let mut result = Vec::new();

    for contour in contours {
        if contour.is_empty() {
            continue;
        }

        let mut flattened = Vec::new();

        for window in contour.windows(2) {
            let a = window[0];
            let b = window[1];

            // Linear interpolation for straight segment
            let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            let steps = if max_segment > 0.0 {
                ((len / max_segment).ceil() as usize).max(1)
            } else {
                1
            };

            for i in 1..=steps {
                let f = i as f64 / steps as f64;
                flattened.push(OutlinePoint {
                    x: a.x + (b.x - a.x) * f,
                    y: a.y + (b.y - a.y) * f,
                });
            }
        }

        // Close the contour by returning to start if needed
        if !flattened.is_empty() && !contour.is_empty() {
            let start = contour[0];
            if (flattened.last().map(|p| p.x).unwrap_or(0.0) - start.x).abs() > 1e-9
                || (flattened.last().map(|p| p.y).unwrap_or(0.0) - start.y).abs() > 1e-9
            {
                flattened.push(start);
            }
        }

        if flattened.len() >= 3 {
            result.push(flattened);
        }
    }

    result
}

/// Format a coordinate to 2 decimal places for compact SVG path d strings.
fn format_coord(n: f64) -> String {
    if n.is_finite() {
        format!("{}", (n * 100.0).round() / 100.0)
    } else {
        "0".to_string()
    }
}

/// Build the warped SVG path `d` for one glyph's outline.
/// Every contour is flattened (pieces no longer than `MAX_SEGMENT`) and each
/// point mapped through `warp`. Returns `None` for an empty outline.
pub fn build_warped_glyph_outline_path_d(
    contours: &[Vec<OutlinePoint>],
    warp: &dyn EnvelopeWarp,
) -> Option<String> {
    let flattened = flatten_outline(contours, MAX_SEGMENT);

    if flattened.is_empty() {
        return None;
    }

    let mut parts = Vec::new();

    for contour in flattened {
        for (i, p) in contour.iter().enumerate() {
            let (qx, qy) = warp.map(p.x, p.y);
            let cmd = if i == 0 { "M" } else { "L" };
            parts.push(format!("{}{}  {}", cmd, format_coord(qx), format_coord(qy)));
        }
        parts.push("Z".to_string());
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(""))
    }
}
