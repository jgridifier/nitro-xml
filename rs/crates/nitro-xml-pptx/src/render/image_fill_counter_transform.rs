// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/image-fill-counter-transform.ts; changes in PROVENANCE.md.

/// The exact algebraic inverse of the frame's rotate/flip transform,
/// restricted to rotation + flip (no skew).
///
/// Derivation: frame transform emits CSS `rotate(θ) scaleX(fx) scaleY(fy)`,
/// i.e. matrix product `R * Sx * Sy` (CSS applies the RIGHTMOST function first).
/// Its inverse is `Sy⁻¹ * Sx⁻¹ * R⁻¹ = Sy * Sx * R(-θ)` (flips are self-inverse),
/// which as CSS list is `scaleY(fy) scaleX(fx) rotate(-θ deg)` — order matters,
/// and differs from the forward transform's order.
pub fn get_frame_inverse_transform(
    flip_h: bool,
    flip_v: bool,
    rotation: Option<f64>,
) -> Option<String> {
    let mut transforms = Vec::new();
    if flip_v {
        transforms.push("scaleY(-1)".to_string());
    }
    if flip_h {
        transforms.push("scaleX(-1)".to_string());
    }
    if let Some(rot) = rotation {
        transforms.push(format!("rotate({}deg)", -rot));
    }
    if transforms.is_empty() {
        None
    } else {
        Some(transforms.join(" "))
    }
}

/// CSS `transform` to apply to a picture's / image fill's CONTENT layer
/// (the `<img>` or `background-image` layer) so it renders upright/unmirrored,
/// fixed to the page frame, when the owning element's image fill explicitly
/// disables `rotWithShape` (`fill_image_rot_with_shape === false`).
///
/// Returns `None` (apply nothing; keep today's behaviour) when:
///  - `fill_image_rot_with_shape` is not explicitly `false`, or
///  - the shape has no rotation/flip to counteract in the first place.
///
/// Callers MUST keep the shape's own geometry mask (clip-path/border-radius)
/// on the STATIONARY frame container (never on this content layer): the
/// container inherits the shape's rotate/flip so the visible clipped silhouette
/// still matches PowerPoint, while the content layer's counter-transform
/// only affects its pixels, not the ancestor's clip region.
///
/// Not applied when the content layer already carries a competing transform
/// (an `a:srcRect` source-crop translate/scale): the two transforms' origins
/// conflict, so combining them is left as a follow-up; the far more common
/// crop takes priority. Pass `has_competing_transform: true` to skip in that case.
pub fn get_image_fill_counter_transform(
    fill_image_rot_with_shape: Option<bool>,
    flip_h: bool,
    flip_v: bool,
    rotation: Option<f64>,
    has_competing_transform: bool,
) -> Option<String> {
    if has_competing_transform {
        return None;
    }
    if fill_image_rot_with_shape != Some(false) {
        return None;
    }
    get_frame_inverse_transform(flip_h, flip_v, rotation)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: flipV + rotWithShape false should emit scaleY(-1).
    #[test]
    fn counter_transform_flipv_rot_with_shape_false() {
        let result = get_image_fill_counter_transform(Some(false), false, true, None, false);
        assert_eq!(result, Some("scaleY(-1)".to_string()));
    }

    // Catches: flipV + flipH + rotation + rotWithShape false should emit all three transforms.
    #[test]
    fn counter_transform_all_flips_and_rotation() {
        let result = get_image_fill_counter_transform(Some(false), true, true, Some(45.5), false);
        assert_eq!(
            result,
            Some("scaleY(-1) scaleX(-1) rotate(-45.5deg)".to_string())
        );
    }

    // Catches: rotWithShape default (None) should not emit counter-transform.
    #[test]
    fn counter_transform_rot_with_shape_default_none() {
        let result = get_image_fill_counter_transform(None, true, true, Some(90.0), false);
        assert_eq!(result, None);
    }

    // Catches: rotWithShape true (explicit, Some(true)) should not emit counter-transform.
    #[test]
    fn counter_transform_rot_with_shape_true() {
        let result = get_image_fill_counter_transform(Some(true), true, true, Some(90.0), false);
        assert_eq!(result, None);
    }

    // Catches: competing transform (crop) takes priority, counter suppressed.
    #[test]
    fn counter_transform_suppressed_by_competing_transform() {
        let result = get_image_fill_counter_transform(Some(false), true, true, Some(45.0), true);
        assert_eq!(result, None);
    }

    // Catches: no flip/rotation should return None even when rotWithShape false.
    #[test]
    fn counter_transform_no_flips_no_rotation() {
        let result = get_image_fill_counter_transform(Some(false), false, false, None, false);
        assert_eq!(result, None);
    }
}
