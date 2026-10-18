// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec upstream/src/ui/src/pptx/render/text-warp-envelope-map.ts; changes listed in PROVENANCE.md.
//! PowerPoint's point mapping for the WordArt two-curve envelope presets,
//! using arc-length parameterization of the top and bottom paths.

#![allow(dead_code)]

use crate::render::wordart_envelope_curves::{envelope_curve_at, has_glyph_envelope};

/// The unwarped text block's bounding box, in layout coordinates.
#[derive(Clone, Copy, Debug)]
pub struct EnvelopeBlockBox {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

/// Maps an unwarped layout point onto the warped shape box.
pub trait EnvelopeWarp {
    fn map(&self, x: f64, y: f64) -> (f64, f64);
}

/// Samples per curve; the arc-length inverse is linear between them.
const CURVE_SAMPLES: usize = 512;

struct ArcTable {
    /// Curve x (box units) at each sample.
    xs: Vec<f64>,
    /// Curve y (box units) at each sample.
    ys: Vec<f64>,
    /// Cumulative arc length at each sample, normalised to `[0, 1]`.
    s: Vec<f64>,
}

fn build_arc_table(ys: &[f64], width: f64) -> ArcTable {
    let n = ys.len();
    let mut xs = vec![0.0; n];
    let mut s = vec![0.0; n];
    let mut total = 0.0;

    for i in 0..n {
        xs[i] = (i as f64 / (n as f64 - 1.0)) * width;
        if i > 0 {
            let dx = xs[i] - xs[i - 1];
            let dy = ys[i] - ys[i - 1];
            total += (dx * dx + dy * dy).sqrt();
        }
        s[i] = total;
    }

    if total > 0.0 {
        for s_i in s.iter_mut() {
            *s_i /= total;
        }
    } else {
        for (i, s_i) in s.iter_mut().enumerate() {
            *s_i = i as f64 / (n as f64 - 1.0);
        }
    }

    ArcTable {
        xs,
        ys: ys.to_vec(),
        s,
    }
}

/// The point at arc-length fraction `t` (clamped to `[0, 1]`) along `table`.
fn point_at_arc_fraction(table: &ArcTable, t: f64) -> (f64, f64) {
    let clamped = t.clamp(0.0, 1.0);
    let mut lo = 0;
    let mut hi = table.s.len() - 1;

    while hi - lo > 1 {
        let mid = (lo + hi) >> 1;
        if table.s[mid] <= clamped {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    let span = table.s[hi] - table.s[lo];
    let f = if span > 0.0 {
        (clamped - table.s[lo]) / span
    } else {
        0.0
    };

    let x = table.xs[lo] + (table.xs[hi] - table.xs[lo]) * f;
    let y = table.ys[lo] + (table.ys[hi] - table.ys[lo]) * f;

    (x, y)
}

/// Concrete EnvelopeWarp implementation using arc-length tables
struct ConcreteEnvelopeWarp {
    top: ArcTable,
    bottom: ArcTable,
    block_w: f64,
    block_h: f64,
    block_left: f64,
    block_top: f64,
}

impl EnvelopeWarp for ConcreteEnvelopeWarp {
    fn map(&self, x: f64, y: f64) -> (f64, f64) {
        let s = (x - self.block_left) / self.block_w;
        let v = (y - self.block_top) / self.block_h;

        let (t_x, t_y) = point_at_arc_fraction(&self.top, s);
        let (b_x, b_y) = point_at_arc_fraction(&self.bottom, s);

        let x_mapped = t_x + (b_x - t_x) * v;
        let y_mapped = t_y + (b_y - t_y) * v;

        (x_mapped, y_mapped)
    }
}

/// Build the envelope mapping for `preset` in a `width` x `height` shape box,
/// normalising layout points by `block`. Returns `None` for a preset
/// outside the envelope family, a degenerate box, or an empty block.
pub fn create_envelope_warp(
    preset: &str,
    width: f64,
    height: f64,
    adj: Option<i32>,
    adj2: Option<i32>,
    block: EnvelopeBlockBox,
) -> Option<Box<dyn EnvelopeWarp>> {
    if !has_glyph_envelope(preset) {
        return None;
    }

    let block_w = block.right - block.left;
    let block_h = block.bottom - block.top;

    if width <= 0.0 || height <= 0.0 || block_w <= 0.0 || block_h <= 0.0 {
        return None;
    }

    let mut top_ys = vec![0.0; CURVE_SAMPLES + 1];
    let mut bottom_ys = vec![0.0; CURVE_SAMPLES + 1];

    for i in 0..=CURVE_SAMPLES {
        let band = envelope_curve_at(preset, i as f64 / CURVE_SAMPLES as f64, adj, adj2)?;
        top_ys[i] = band.top * height;
        bottom_ys[i] = band.bottom * height;
    }

    let top = build_arc_table(&top_ys, width);
    let bottom = build_arc_table(&bottom_ys, width);

    Some(Box::new(ConcreteEnvelopeWarp {
        top,
        bottom,
        block_w,
        block_h,
        block_left: block.left,
        block_top: block.top,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_envelope_warp_inflate() {
        let block = EnvelopeBlockBox {
            left: 10.0,
            top: 10.0,
            right: 110.0,
            bottom: 60.0,
        };
        let warp = create_envelope_warp("textInflate", 100.0, 50.0, Some(18750), None, block);
        assert!(warp.is_some(), "inflate should create a valid warp");

        let warp = warp.unwrap();
        // Centre point should map somewhere
        let (x, y) = warp.map(60.0, 35.0);
        assert!(
            x.is_finite() && y.is_finite(),
            "mapped point should be finite"
        );
    }

    #[test]
    fn test_create_envelope_warp_invalid_block() {
        let block = EnvelopeBlockBox {
            left: 10.0,
            top: 10.0,
            right: 10.0, // zero width
            bottom: 60.0,
        };
        let warp = create_envelope_warp("textInflate", 100.0, 50.0, Some(18750), None, block);
        assert!(warp.is_none(), "degenerate block should return None");
    }
}
