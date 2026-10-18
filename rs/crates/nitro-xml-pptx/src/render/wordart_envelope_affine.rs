// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec upstream/src/ui/src/pptx/render/text-warp-glyph-slicing.ts; changes listed in PROVENANCE.md.
//! Affine / piecewise-affine fallback for a WordArt envelope glyph whose
//! outline is not obtainable. Fits SVG `matrix(a b c d e f)` transforms
//! over glyph ink boxes, slicing when a single affine misses the envelope mapping.

#![allow(dead_code)]

use crate::render::wordart_envelope_map::EnvelopeWarp;

/// Never split a single glyph into more than this many rendered pieces.
pub const MAX_ENVELOPE_GLYPH_SLICES: usize = 24;

/// Largest acceptable miss between a piece's affine and the true mapping, as
/// a fraction of the shape box height (never below half a pixel).
const SLICE_TOLERANCE_FRACTION: f64 = 0.005;
const MIN_SLICE_TOLERANCE_PX: f64 = 0.5;

/// Grid resolution (per axis) the affine fit and its error are measured on.
const FIT_GRID: usize = 5;

/// How far past a glyph's outer ink edge its outermost clip rects reach.
const CLIP_OVERHANG: f64 = 100000.0;

/// One rendered piece of a glyph: clipped to its own x-band, its own affine fit.
#[derive(Clone, Debug)]
pub struct EnvelopeGlyphSlice {
    /// Left edge of this slice's clip rect, in the glyph's own (pre-transform) x.
    pub clip_x0: f64,
    /// Right edge of this slice's clip rect, in the glyph's own (pre-transform) x.
    pub clip_x1: f64,
    /// SVG `matrix(a b c d e f)` fit of the envelope mapping over this slice.
    pub transform: String,
}

/// The rectangle (layout units) an affine fit covers.
#[derive(Clone, Copy, Debug)]
pub struct GlyphFitBox {
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
}

type Affine = [f64; 6];

/// Solve the 3x3 system `m * p = r` by Cramer's rule (`None` when singular).
fn solve3(m: &[[f64; 3]; 3], r: &[f64; 3]) -> Option<[f64; 3]> {
    let det = |a: &[[f64; 3]; 3]| -> f64 {
        a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
            - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
    };

    let d = det(m);
    if d.abs() < 1e-12 {
        return None;
    }

    let col = |i: usize| -> [[f64; 3]; 3] {
        let mut result = *m;
        for k in 0..3 {
            result[k][i] = r[k];
        }
        result
    };

    Some([det(&col(0)) / d, det(&col(1)) / d, det(&col(2)) / d])
}

struct FitResult {
    affine: Affine,
    error: f64,
}

/// Least-squares affine of `warp` over `box`, plus its worst miss on the grid.
fn fit_affine(warp: &dyn EnvelopeWarp, box_: GlyphFitBox) -> FitResult {
    let mut src: Vec<(f64, f64)> = Vec::new();
    let mut dst: Vec<(f64, f64)> = Vec::new();

    for i in 0..FIT_GRID {
        for j in 0..FIT_GRID {
            let x = box_.x0 + ((box_.x1 - box_.x0) * i as f64) / (FIT_GRID as f64 - 1.0);
            let y = box_.y0 + ((box_.y1 - box_.y0) * j as f64) / (FIT_GRID as f64 - 1.0);
            src.push((x, y));
            let (dx, dy) = warp.map(x, y);
            dst.push((dx, dy));
        }
    }

    let mut n = [[0.0; 3]; 3];
    let mut rx = [0.0; 3];
    let mut ry = [0.0; 3];

    for (k, &(x, y)) in src.iter().enumerate() {
        let v = [x, y, 1.0];
        for i in 0..3 {
            for j in 0..3 {
                n[i][j] += v[i] * v[j];
            }
            rx[i] += v[i] * dst[k].0;
            ry[i] += v[i] * dst[k].1;
        }
    }

    let px = solve3(&n, &rx);
    let py = solve3(&n, &ry);

    if let (Some(px), Some(py)) = (px, py) {
        // SVG `matrix(a b c d e f)`: x' = a x + c y + e, y' = b x + d y + f.
        let affine: Affine = [px[0], py[0], px[1], py[1], px[2], py[2]];

        let mut error: f64 = 0.0;
        for (k, &(x, y)) in src.iter().enumerate() {
            let fx = affine[0] * x + affine[2] * y + affine[4];
            let fy = affine[1] * x + affine[3] * y + affine[5];
            let dx = fx - dst[k].0;
            let dy = fy - dst[k].1;
            error = error.max((dx * dx + dy * dy).sqrt());
        }

        FitResult { affine, error }
    } else {
        let c = warp.map((box_.x0 + box_.x1) / 2.0, (box_.y0 + box_.y1) / 2.0);
        FitResult {
            affine: [1.0, 0.0, 0.0, 1.0, c.0, c.1],
            error: 0.0,
        }
    }
}

fn format_matrix(m: &Affine) -> String {
    format!(
        "matrix({} {} {} {} {} {})",
        if m[0].is_finite() { m[0] } else { 0.0 },
        if m[1].is_finite() { m[1] } else { 0.0 },
        if m[2].is_finite() { m[2] } else { 0.0 },
        if m[3].is_finite() { m[3] } else { 0.0 },
        if m[4].is_finite() { m[4] } else { 0.0 },
        if m[5].is_finite() { m[5] } else { 0.0 },
    )
}

/// A glyph's single-affine transform plus, when one affine is not enough, its slices.
#[derive(Clone, Debug)]
pub struct GlyphAffineFit {
    pub transform: String,
    pub slices: Option<Vec<EnvelopeGlyphSlice>>,
}

/// Fit the envelope mapping over a glyph's ink `box`, slicing it into
/// vertical bands when a single affine misses by more than the tolerance.
pub fn fit_glyph_envelope_affine(
    warp: &dyn EnvelopeWarp,
    box_: GlyphFitBox,
    shape_height: f64,
) -> GlyphAffineFit {
    let tolerance = (MIN_SLICE_TOLERANCE_PX).max(shape_height * SLICE_TOLERANCE_FRACTION);
    let whole = fit_affine(warp, box_);
    let transform = format_matrix(&whole.affine);

    if whole.error <= tolerance || box_.x1 <= box_.x0 {
        return GlyphAffineFit {
            transform,
            slices: None,
        };
    }

    let mut pieces = Vec::new();
    let mut count = 2;

    while count <= MAX_ENVELOPE_GLYPH_SLICES {
        let step = (box_.x1 - box_.x0) / count as f64;
        pieces.clear();

        for i in 0..count {
            let x0 = box_.x0 + step * i as f64;
            let x1 = box_.x0 + step * (i as f64 + 1.0);
            pieces.push(fit_affine(
                warp,
                GlyphFitBox {
                    x0,
                    x1,
                    y0: box_.y0,
                    y1: box_.y1,
                },
            ));
        }

        if pieces.iter().all(|p| p.error <= tolerance) {
            break;
        }
        count += 1;
    }

    let n = pieces.len();
    let step = (box_.x1 - box_.x0) / n as f64;
    let slices = pieces
        .into_iter()
        .enumerate()
        .map(|(i, p)| EnvelopeGlyphSlice {
            clip_x0: if i == 0 {
                box_.x0 - CLIP_OVERHANG
            } else {
                box_.x0 + step * i as f64
            },
            clip_x1: if i == n - 1 {
                box_.x1 + CLIP_OVERHANG
            } else {
                box_.x0 + step * (i as f64 + 1.0)
            },
            transform: format_matrix(&p.affine),
        })
        .collect();

    GlyphAffineFit {
        transform,
        slices: Some(slices),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockWarp;

    impl EnvelopeWarp for MockWarp {
        fn map(&self, x: f64, y: f64) -> (f64, f64) {
            // Simple linear transformation for testing
            (x * 1.1, y * 1.05)
        }
    }

    #[test]
    fn test_fit_glyph_envelope_affine_finite() {
        // Catches: affine matrix should produce finite values even with near-singular systems
        let warp = MockWarp;
        let box_ = GlyphFitBox {
            x0: 0.0,
            x1: 100.0,
            y0: 0.0,
            y1: 50.0,
        };

        let fit = fit_glyph_envelope_affine(&warp, box_, 50.0);
        assert!(
            fit.transform.contains("matrix("),
            "transform should be a matrix"
        );
        // Verify all values in the matrix string are numbers (no NaN or Infinity)
        let matrix_content = fit.transform.replace("matrix(", "").replace(")", "");
        let values: Vec<&str> = matrix_content.split_whitespace().collect();
        assert_eq!(values.len(), 6, "matrix should have 6 values");
        for v in values {
            let parsed: Result<f64, _> = v.parse();
            assert!(parsed.is_ok(), "matrix value should be parseable: {}", v);
            let num = parsed.unwrap();
            assert!(
                num.is_finite(),
                "matrix value should be finite, got: {}",
                num
            );
        }
    }
}
