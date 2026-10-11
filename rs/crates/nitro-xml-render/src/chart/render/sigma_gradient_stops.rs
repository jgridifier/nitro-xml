// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/color/sigma-gradient-stops.ts

use nitro_xml_opc::drawingml::{hex_to_rgb_channels, to_hex};

use super::gradient_coincident_stops::GradientStop;

mod sigma_factors {
    include!("sigma_factors.rs");
}
use sigma_factors::OFFICE_SIGMA_FACTORS;

pub fn sigma_gradient_stops(stops: &[GradientStop]) -> Option<Vec<GradientStop>> {
    if stops.len() != 2 || stops[0].offset != 0.0 || stops[1].offset != 1.0 {
        return None;
    }
    if stops
        .iter()
        .any(|s| (s.opacity.unwrap_or(1.0) - 1.0).abs() > 1e-9)
    {
        return None;
    }
    let front = hex_to_rgb_channels(&stops[0].color)?;
    let back = hex_to_rgb_channels(&stops[1].color)?;
    let mix = |a: f64, b: f64, factor: f64| {
        let v = ((a / 255.0).powf(2.2) * (1.0 - factor) + (b / 255.0).powf(2.2) * factor)
            .powf(1.0 / 2.2);
        to_hex(v * 255.0)
    };
    Some(
        OFFICE_SIGMA_FACTORS
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let factor = *value as f64 / 65536.0;
                GradientStop {
                    offset: (index as f64 / 255.0) as f32 as f64,
                    color: format!(
                        "#{}{}{}",
                        mix(front.0, back.0, factor),
                        mix(front.1, back.1, factor),
                        mix(front.2, back.2, factor)
                    ),
                    opacity: Some(1.0),
                }
            })
            .collect(),
    )
}
