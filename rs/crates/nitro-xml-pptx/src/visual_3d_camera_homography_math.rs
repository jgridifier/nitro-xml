// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/visual-3d-camera-homography-math.ts; changes listed in PROVENANCE.md.

/// Generic "unit square -> quadrilateral" projective homography solver
/// (framework-agnostic, no OOXML-specific knowledge).
///
/// Split out so the camera model math is not tangled with this general-purpose closed
/// form, and so it can be unit-tested in isolation against hand-computed cases. This is
/// the standard "map a square to an arbitrary convex quadrilateral" projective transform
/// (P. Heckbert, "Fundamentals of Texture Mapping and Image Warping", UC Berkeley MS
/// thesis, 1989, §3): given the four destination points the unit square's corners
/// `(0,0) (1,0) (0,1) (1,1)` map to, it returns the unique 3x3 homography (`h33` normalised
/// to 1) reproducing exactly those four correspondences.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

/// A row-major 3x3 homography, `h33` normalised to 1: `[h11,h12,h13,h21,h22,h23,h31,h32,h33]`.
pub type Homography3 = [f64; 9];

/// Solve the projective homography mapping the unit square's corners
/// `(0,0)->d00`, `(1,0)->d10`, `(0,1)->d01`, `(1,1)->d11`. Degenerates
/// gracefully to the identity when all four destinations equal their source
/// (avoids a division by zero the affine special case would otherwise need to
/// guard separately).
pub fn unit_square_to_quad_homography(
    d00: Point2,
    d10: Point2,
    d01: Point2,
    d11: Point2,
) -> Homography3 {
    let dx1 = d10.x - d11.x;
    let dx2 = d01.x - d11.x;
    let dx3 = d00.x - d10.x + d11.x - d01.x;
    let dy1 = d10.y - d11.y;
    let dy2 = d01.y - d11.y;
    let dy3 = d00.y - d10.y + d11.y - d01.y;

    let denom = dx1 * dy2 - dx2 * dy1;
    let (mut g, mut h) = (0.0, 0.0);
    if denom != 0.0 {
        // Fold a `-0` result to `+0` (e.g. the trivial identity case),
        // which matters only for exact-equality assertions in tests.
        let g_calc = (dx3 * dy2 - dx2 * dy3) / denom;
        let h_calc = (dx1 * dy3 - dx3 * dy1) / denom;
        g = if g_calc == -0.0 { 0.0 } else { g_calc };
        h = if h_calc == -0.0 { 0.0 } else { h_calc };
    }

    let a = d10.x - d00.x + g * d10.x;
    let b = d01.x - d00.x + h * d01.x;
    let c = d00.x;
    let d = d10.y - d00.y + g * d10.y;
    let e = d01.y - d00.y + h * d01.y;
    let f = d00.y;

    [a, b, c, d, e, f, g, h, 1.0]
}

/// Apply a {@link Homography3} to a single point (for tests / sanity checks).
#[allow(dead_code)]
pub fn apply_homography(h: &Homography3, p: Point2) -> Point2 {
    let [a, b, c, d, e, f, g, h_val, i] = h;
    let w = g * p.x + h_val * p.y + i;
    Point2 {
        x: (a * p.x + b * p.y + c) / w,
        y: (d * p.x + e * p.y + f) / w,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: flat orthographicFront (no rot) still emits a skewed matrix3d from a bad solver.
    #[test]
    fn test_identity_homography() {
        let h = unit_square_to_quad_homography(
            Point2 { x: 0.0, y: 0.0 },
            Point2 { x: 1.0, y: 0.0 },
            Point2 { x: 0.0, y: 1.0 },
            Point2 { x: 1.0, y: 1.0 },
        );
        let expected: Homography3 = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        assert!(h
            .iter()
            .zip(expected.iter())
            .all(|(a, b)| (a - b).abs() < 1e-10));
    }

    // Catches: unit-square→quad solver mapping the wrong corner so yaw/combined poses shear the wrong way.
    #[test]
    fn test_homography_corners() {
        let d00 = Point2 { x: 0.1, y: 0.2 };
        let d10 = Point2 { x: 1.1, y: 0.3 };
        let d01 = Point2 { x: 0.2, y: 1.3 };
        let d11 = Point2 { x: 1.2, y: 1.4 };

        let h = unit_square_to_quad_homography(d00, d10, d01, d11);

        let p00 = apply_homography(&h, Point2 { x: 0.0, y: 0.0 });
        let p10 = apply_homography(&h, Point2 { x: 1.0, y: 0.0 });
        let p01 = apply_homography(&h, Point2 { x: 0.0, y: 1.0 });
        let p11 = apply_homography(&h, Point2 { x: 1.0, y: 1.0 });

        assert!((p00.x - d00.x).abs() < 1e-10 && (p00.y - d00.y).abs() < 1e-10);
        assert!((p10.x - d10.x).abs() < 1e-10 && (p10.y - d10.y).abs() < 1e-10);
        assert!((p01.x - d01.x).abs() < 1e-10 && (p01.y - d01.y).abs() < 1e-10);
        assert!((p11.x - d11.x).abs() < 1e-10 && (p11.y - d11.y).abs() < 1e-10);
    }
}
