// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/drawingml/ordered-color-transforms.ts; changes listed in PROVENANCE.md.

use super::color_linear::{linear_to_srgb255, srgb255_to_linear};
use super::color_primitives::{
    clamp_unit_interval, hsl_to_rgb, parse_drawing_fraction, parse_drawing_hue_degrees, rgb_to_hsl,
};
use super::types::ColorTransform;

pub struct TransformedChannels {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub alpha: f64,
}

pub fn ordered_color_transforms(
    channels: (f64, f64, f64),
    transforms: &[ColorTransform],
) -> TransformedChannels {
    let (mut r, mut g, mut b) = channels;
    let mut alpha = 1.0;
    let wrap = |value: f64| ((value % 360.0) + 360.0) % 360.0;
    for transform in transforms {
        let name = transform.name.as_str();
        let value = transform.value.as_str();
        let fraction = parse_drawing_fraction(value);
        match name {
            "alpha" if let Some(frac) = fraction => {
                alpha = clamp_unit_interval(frac);
            }
            "alphaMod" if let Some(frac) = fraction => {
                alpha = clamp_unit_interval(alpha * frac);
            }
            "alphaOff" if let Some(frac) = fraction => {
                alpha = clamp_unit_interval(alpha + frac);
            }
            "inv" => {
                r = 255.0 - r;
                g = 255.0 - g;
                b = 255.0 - b;
            }
            "gray" => {
                let gray = 0.299 * r + 0.587 * g + 0.114 * b;
                r = gray;
                g = gray;
                b = gray;
            }
            "shade" | "tint" if let Some(frac) = fraction => {
                let mix = |channel: f64| {
                    let linear = if name == "shade" {
                        srgb255_to_linear(channel) * frac
                    } else {
                        1.0 - (1.0 - srgb255_to_linear(channel)) * frac
                    };
                    linear_to_srgb255(linear)
                };
                r = mix(r);
                g = mix(g);
                b = mix(b);
            }
            _ => {
                let hsl = rgb_to_hsl(r, g, b);
                let mut h = hsl.h;
                let mut s = hsl.s;
                let mut l = hsl.l;
                let mut applied = true;
                match name {
                    "comp" => h = wrap(h + 180.0),
                    "hue" => {
                        if let Some(hue) = parse_drawing_hue_degrees(value) {
                            h = wrap(hue);
                        }
                    }
                    "hueOff" => {
                        if let Some(hue) = parse_drawing_hue_degrees(value) {
                            h = wrap(h + hue);
                        }
                    }
                    "hueMod" if let Some(frac) = fraction => h = wrap(h * frac),
                    "sat" if let Some(frac) = fraction => s = clamp_unit_interval(frac),
                    "satMod" if let Some(frac) = fraction => s *= frac,
                    "satOff" if let Some(frac) = fraction => s = clamp_unit_interval(s + frac),
                    "lum" if let Some(frac) = fraction => l = clamp_unit_interval(frac),
                    "lumMod" if let Some(frac) = fraction => l = clamp_unit_interval(l * frac),
                    "lumOff" if let Some(frac) = fraction => l = clamp_unit_interval(l + frac),
                    _ => applied = false,
                }
                if !applied {
                    continue;
                }
                let rgb = hsl_to_rgb(h, s, l, |c| c, false);
                r = rgb.0;
                g = rgb.1;
                b = rgb.2;
            }
        }
    }
    TransformedChannels { r, g, b, alpha }
}
