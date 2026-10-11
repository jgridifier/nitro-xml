// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/gradient-coincident-stops.ts

pub const NATIVE_COINCIDENT_STOP_RAMP: f64 = 1.0 / 256.0;

#[derive(Clone, Debug)]
pub struct GradientStop {
    pub offset: f64,
    pub color: String,
    pub opacity: Option<f64>,
}

pub fn spread_coincident_stops(stops: &[GradientStop]) -> Vec<GradientStop> {
    let mut out: Vec<GradientStop> = Vec::new();
    for (index, stop) in stops.iter().enumerate() {
        let source = stops.get(index - 1);
        let previous = out.last();
        let next = stops.get(index + 1);
        if source.is_none()
            || previous.is_none()
            || stop.offset != source.unwrap().offset
            || (stop.color == source.unwrap().color
                && stop.opacity.unwrap_or(1.0) == source.unwrap().opacity.unwrap_or(1.0))
        {
            out.push(stop.clone());
            continue;
        }
        let limit = next.map(|n| n.offset).unwrap_or(1.0).min(1.0);
        let new_offset = stop
            .offset
            .max(previous.unwrap().offset + NATIVE_COINCIDENT_STOP_RAMP)
            .min(limit);
        out.push(GradientStop {
            offset: new_offset,
            color: stop.color.clone(),
            opacity: stop.opacity,
        });
    }
    out
}
