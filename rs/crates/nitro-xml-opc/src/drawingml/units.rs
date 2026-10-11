// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/units/units.ts; changes listed in PROVENANCE.md.

/// EMU per pixel at 96 DPI (914400 EMU per inch / 96).
pub const EMU_PER_PIXEL: i64 = 9525;

/// Convert EMU to CSS pixels at 96 DPI.
pub fn emu_to_pixels(value: i64) -> f64 {
    value as f64 / EMU_PER_PIXEL as f64
}

/// Convert CSS pixels at 96 DPI to EMU.
pub fn pixels_to_emu(value: f64) -> i64 {
    (value * EMU_PER_PIXEL as f64).round() as i64
}
