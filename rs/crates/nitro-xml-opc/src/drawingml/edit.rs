// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/utils/xfrm-emu-resolution.ts;
// fill/line patch helpers are original; changes listed in PROVENANCE.md.

/// Whether `stored_emu` still round-trips to the element's current pixel value.
///
/// `Math.round(storedEmu / emuPerPx) === Math.round(px)` in the upstream helper.
/// A missing stored EMU (SDK-created shape) is always "changed".
pub fn is_xfrm_emu_unchanged(px: f64, stored_emu: Option<i64>, emu_per_px: f64) -> bool {
    let Some(stored) = stored_emu else {
        return false;
    };
    if emu_per_px == 0.0 {
        return false;
    }
    (stored as f64 / emu_per_px).round() == px.round()
}

/// EMU integer to write for one transform axis.
///
/// Re-emits `stored_emu` when it still agrees with `px`. Otherwise re-quantizes
/// with `Math.round(px * emuPerPx)`.
pub fn resolve_xfrm_emu(px: f64, stored_emu: Option<i64>, emu_per_px: f64) -> i64 {
    if is_xfrm_emu_unchanged(px, stored_emu, emu_per_px) {
        return stored_emu.unwrap_or(0);
    }
    (px * emu_per_px).round() as i64
}

/// Merge a solid / none fill onto an editor element.
///
/// `color`: `None` leaves the colour, `Some(None)` clears it, `Some(Some(hex))` sets it.
/// `mode`: `None` leaves fill mode; `"solid"` / `"none"` replace it.
pub fn patch_shape_fill(
    fill_color: &mut Option<String>,
    fill_mode: &mut Option<String>,
    color: Option<Option<String>>,
    mode: Option<&str>,
) {
    if let Some(next) = color {
        *fill_color = next;
    }
    if let Some(mode) = mode {
        *fill_mode = Some(mode.to_owned());
    }
}

/// Merge stroke colour and width onto an editor element.
///
/// `color`: `None` leaves the colour, `Some(None)` clears it, `Some(Some(hex))` sets it.
/// `width`: `None` leaves the width.
pub fn patch_shape_line(
    stroke_color: &mut Option<String>,
    stroke_width: &mut Option<f64>,
    color: Option<Option<String>>,
    width: Option<f64>,
) {
    if let Some(next) = color {
        *stroke_color = next;
    }
    if let Some(width) = width {
        *stroke_width = Some(width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_unchanged_non_multiple_emu_reemitted_verbatim() {
        // Catches: saving an unmoved shape re-quantizes a non-multiple EMU and drifts a:off.
        let emu_per_px = 9525.0;
        let stored = 10_000_i64;
        let px = (stored as f64 / emu_per_px).round();
        assert!(is_xfrm_emu_unchanged(px, Some(stored), emu_per_px));
        assert_eq!(resolve_xfrm_emu(px, Some(stored), emu_per_px), stored);
        let moved = px + 1.0;
        assert!(!is_xfrm_emu_unchanged(moved, Some(stored), emu_per_px));
        assert_eq!(
            resolve_xfrm_emu(moved, Some(stored), emu_per_px),
            (moved * emu_per_px).round() as i64
        );
        assert_eq!(
            resolve_xfrm_emu(120.0, None, emu_per_px),
            (120.0 * emu_per_px).round() as i64
        );
    }
}
