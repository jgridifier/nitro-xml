// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/units.ts; changes listed in PROVENANCE.md.

pub fn twips_to_px(value: i64) -> f64 {
    (value as f64) * 96.0 / 1440.0
}

pub fn twips_f64_to_px(value: f64) -> f64 {
    value * 96.0 / 1440.0
}

pub const NO_TWIPS: i64 = 0;

pub fn px_to_twips(px: f64) -> i64 {
    ((px * 1440.0) / 96.0).round() as i64
}

pub fn pt_to_px(pt: f64) -> f64 {
    pt * 96.0 / 72.0
}

pub const DEFAULT_TAB_STOP_PX: f64 = 48.0;
pub const DEFAULT_FONT_FAMILY: &str = "Calibri";
pub const DEFAULT_FONT_SIZE_PT: f64 = 11.0;
pub const SINGLE_LINE_SPACING_UNITS: i64 = 240;
