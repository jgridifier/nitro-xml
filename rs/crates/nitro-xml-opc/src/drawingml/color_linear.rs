// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/color/color-linear.ts; changes listed in PROVENANCE.md.

use super::color_primitives::clamp_unit_interval;

pub fn srgb255_to_linear(channel: f64) -> f64 {
    let c = clamp_unit_interval(channel / 255.0);
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb255(linear: f64) -> f64 {
    let c = clamp_unit_interval(linear);
    255.0
        * if c <= 0.0031308 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
}
