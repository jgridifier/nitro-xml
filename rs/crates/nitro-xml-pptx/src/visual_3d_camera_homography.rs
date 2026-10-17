// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/visual-3d-camera-homography.ts; changes listed in PROVENANCE.md.

/// COM-measured exact off-axis camera homographies.
///
/// The rotateX/rotateY + centred perspective() model cannot reproduce a genuine
/// off-axis vanishing point: a real camera translated to the side while re-aimed
/// at the shape projects a flat plane through a full 2D projective transform
/// (an 8-DOF homography), not a 3-axis Euler rotation viewed through a centred lens.
use super::visual_3d_camera_homography_math::Homography3;

/// Re-scale a unit-square homography to an element's actual on-screen
/// width/height and embed it as a CSS `matrix3d(...)` string.
///
/// The homography is stored normalised to the unit square on BOTH the source
/// and destination sides (COM-verified size-invariant); for an element of
/// size (W, H) the general rescale is applied entry-wise:
/// `H' = diag(W,H,1) . H_unit . diag(1/W,1/H,1)`.
///
/// The 3x3 -> CSS 4x4 embedding (z-row/column identity, everything else zero)
/// reproduces the exact 2D projective divide on the element's z=0 plane:
/// `matrix3d(h11,h21,0,h31, h12,h22,0,h32, 0,0,1,0, h13,h23,0,h33)`. Must be
/// paired with `transform-origin: 0 0` (the homography already encodes
/// whatever translation the measured camera produces relative to the
/// element's own un-rotated top-left; a non-zero transform-origin would
/// double-apply it).
pub fn homography_to_matrix3d(h_unit: &Homography3, width: f64, height: f64) -> String {
    let w = if width > 0.0 { width } else { 1.0 };
    let h = if height > 0.0 { height } else { 1.0 };
    let [h11, h12, h13, h21, h22, h23, h31, h32, h33] = *h_unit;

    let a11 = h11;
    let a12 = h12 * (w / h);
    let a13 = h13 * w;
    let a21 = h21 * (h / w);
    let a22 = h22;
    let a23 = h23 * h;
    let a31 = h31 / w;
    let a32 = h32 / h;
    let a33 = h33;

    let m = [
        a11, a21, 0.0, a31, a12, a22, 0.0, a32, 0.0, 0.0, 1.0, 0.0, a13, a23, 0.0, a33,
    ];
    let formatted: Vec<String> = m.iter().map(|n| format!("{:.6}", n)).collect();
    format!("matrix3d({})", formatted.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // Catches: identity homography produces identity matrix3d
    fn test_identity_matrix3d() {
        let h_identity: Homography3 = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let result = homography_to_matrix3d(&h_identity, 100.0, 100.0);
        assert!(result.contains("matrix3d("));
        // For identity, should have 1.000000 on diagonal and 0.000000 elsewhere
        assert!(result.contains("1.000000"));
        assert!(result.contains("0.000000"));
    }
}
