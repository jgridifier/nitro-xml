// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/visual-3d-camera-parametric.ts; changes listed in PROVENANCE.md.

/// Parametric camera homography for an explicit `a:camera/a:rot` override
/// (framework-agnostic).
///
/// Closes the third of three "3-D shapes and scenes" limitations: the parametric
/// model builds the same kind of exact homography for explicit camera overrides
/// that a preset table stores for fixed presets, enabling both to use identical
/// matrix3d machinery.
///
/// 1. The shape's flat picture plane is a unit square in its own local XY plane
///    (`z=0`), corners at `(-0.5,-0.5) .. (0.5,0.5)`.
/// 2. Each corner is projected by `project_corner`: an ORTHOGRAPHIC (parallel, no
///    perspective divide) rotation-composition transform.
/// 3. `lat=lon=rev=0` reproduces an EXACT identity homography - the same trivial
///    case `orthographicFront` produces - by construction (`cos(0)=1`, every other
///    term vanishes).
/// 4. `rev` (roll about the view axis) commutes with the projection: rolling the
///    camera about its own aim axis is exactly a 2D rotation of the already-projected
///    image, applied as a post-projection step.
/// 5. The 4 projected corners feed `unit_square_to_quad_homography`, the same
///    closed-form solver used in the homography table.
///
/// COM validation (27-point lat x lon x rev grid, 2026-09) found the exact closed form:
/// ```
/// x = X * cos(lon)
/// y = Y * cos(lat) - X * sin(lat) * sin(lon)
/// ```
/// Across all 27 grid cells: average max-corner error 0.61%, median well under 1%,
/// worst at 2.10% (all `rev=25deg` angles, noise from antialiasing, not systematic).
use super::visual_3d_camera_homography_math::{
    unit_square_to_quad_homography, Homography3, Point2,
};

/// Explicit camera parameters (radians for angles, already zoom-adjusted FOV).
#[derive(Debug, Clone, Copy)]
pub struct ParametricCameraParams {
    /// Pitch (`a:camera/a:rot/@lat`), radians.
    pub lat_rad: f64,
    /// Yaw (`a:camera/a:rot/@lon`), radians.
    pub lon_rad: f64,
    /// Roll (`a:camera/a:rot/@rev`), radians.
    pub rev_rad: f64,
    /// Field of view, radians (zoom already folded in).
    /// UNUSED by `project_corner` (the parametric model is purely orthographic);
    /// kept for API stability.
    #[allow(dead_code)]
    pub fov_rad: f64,
}

fn rotate_2d(p: Point2, angle: f64) -> Point2 {
    if angle == 0.0 {
        return p;
    }
    let c = angle.cos();
    let s = angle.sin();
    Point2 {
        x: p.x * c - p.y * s,
        y: p.x * s + p.y * c,
    }
}

/// Project one local unit-square corner `(x, y)` (already centred, y-up)
/// through the camera.
///
/// `x` is a pure per-axis cosine foreshortening (`x = X*cos(lon)`), COM-confirmed
/// independent of `lat`. `y` gets the same cosine scale on its own axis
/// (`Y*cos(lat)`) plus a rotation-composition cross term, `-X*sin(lat)*sin(lon)`,
/// that is exactly 0 whenever EITHER axis is 0 (so both the identity and every
/// single-axis case reproduce their already-COM-validated result unchanged) and
/// otherwise fits the 27-point grid to within ~1% on average.
fn project_corner(local_x: f64, local_y: f64, params: ParametricCameraParams) -> Point2 {
    let x = local_x * params.lon_rad.cos();
    let y = local_y * params.lat_rad.cos() - local_x * params.lat_rad.sin() * params.lon_rad.sin();
    rotate_2d(Point2 { x, y }, params.rev_rad)
}

/// Compute the unit-square homography (same convention as the measured table:
/// destination expressed as fractions of the square's own side, relative to its
/// own un-rotated top-left corner) for an explicit camera pose. Reproduces the
/// exact identity for `lat_rad=lon_rad=rev_rad=0` by construction.
pub fn compute_parametric_camera_homography(params: ParametricCameraParams) -> Homography3 {
    // Source (u,v) in [0,1]^2 -> local (x, y-up) centred at the origin: v=0
    // (top row) is local y=+0.5, matching the destination convention where
    // (0,0) is the top-left corner.
    let to_local = |u: f64, v: f64| -> (f64, f64) { (u - 0.5, 0.5 - v) };
    let to_dest = |u: f64, v: f64| -> Point2 {
        let (lx, ly) = to_local(u, v);
        let projected = project_corner(lx, ly, params);
        Point2 {
            x: 0.5 + projected.x,
            y: 0.5 - projected.y,
        }
    };

    unit_square_to_quad_homography(
        to_dest(0.0, 0.0),
        to_dest(1.0, 0.0),
        to_dest(0.0, 1.0),
        to_dest(1.0, 1.0),
    )
}

/// `1/60000`th-degree camera rotation field -> radians, or 0 when unset.
pub fn sixty_thousandths_deg_to_rad(value: Option<i32>) -> f64 {
    match value {
        Some(v) => (v as f64 / 60000.0) * (std::f64::consts::PI / 180.0),
        None => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: empty a:rot still foreshortens the shape (non-identity matrix) under orthographicFront.
    #[test]
    fn test_parametric_identity() {
        let params = ParametricCameraParams {
            lat_rad: 0.0,
            lon_rad: 0.0,
            rev_rad: 0.0,
            fov_rad: 0.0,
        };
        let h = compute_parametric_camera_homography(params);
        let expected: Homography3 = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        assert!(h
            .iter()
            .zip(expected.iter())
            .all(|(a, b)| (a - b).abs() < 1e-10));
    }

    // Catches: a:rot/@lon treated as degrees instead of 1/60000ths so a 25° yaw barely moves.
    #[test]
    fn test_sixty_thousandths_deg_to_rad() {
        let rad = sixty_thousandths_deg_to_rad(Some(60000)); // 1 degree
        assert!((rad - std::f64::consts::PI / 180.0).abs() < 1e-10);

        let rad = sixty_thousandths_deg_to_rad(Some(0));
        assert_eq!(rad, 0.0);

        let rad = sixty_thousandths_deg_to_rad(None);
        assert_eq!(rad, 0.0);
    }

    // Catches: explicit yaw (lon only) left as a flat axis-aligned square (332cb tile).
    #[test]
    fn test_yaw_only_foreshortens_x() {
        let lon = sixty_thousandths_deg_to_rad(Some(1_500_000)); // 25°
        let h = compute_parametric_camera_homography(ParametricCameraParams {
            lat_rad: 0.0,
            lon_rad: lon,
            rev_rad: 0.0,
            fov_rad: 0.0,
        });
        // Top-right corner of the unit square must land near (cos(25°), 0).
        let p10 = crate::visual_3d_camera_homography_math::apply_homography(
            &h,
            crate::visual_3d_camera_homography_math::Point2 { x: 1.0, y: 0.0 },
        );
        let expected_x = lon.cos();
        // Destination is relative to top-left: x≈0.5+0.5*cos(lon) after local→dest.
        assert!(
            (p10.x - (0.5 + 0.5 * expected_x)).abs() < 1e-6,
            "yaw foreshortening missing: p10={p10:?}"
        );
        assert!((p10.y - 0.0).abs() < 1e-6, "yaw must not skew y: p10={p10:?}");
    }
}
