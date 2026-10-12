// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/axis-nice.ts; changes listed in PROVENANCE.md.

use super::chart_scale::{clean, AxisScale};

const NICE_STEPS: [f64; 5] = [1.0, 2.0, 2.5, 5.0, 10.0];
const HEADROOM: f64 = 0.05;
const POINTS_PER_GRIDLINE: f64 = 20.0;
const MIN_TARGET_INTERVALS: i32 = 1;
const MAX_TARGET_INTERVALS: i32 = 10;
const PX_PER_POINT: f64 = 4.0 / 3.0;
const DEFAULT_TARGET_INTERVALS: i32 = 4;
const ZERO_ANCHOR_RATIO: f64 = 5.0 / 6.0;

#[derive(Clone, Copy, Debug)]
pub struct NiceAxisBounds {
    pub min: f64,
    pub max: f64,
    pub major_unit: f64,
}

pub fn axis_target_intervals(plot_height_px: f64) -> i32 {
    if !plot_height_px.is_finite() || plot_height_px <= 0.0 {
        return DEFAULT_TARGET_INTERVALS;
    }
    let plot_height_pt = plot_height_px / PX_PER_POINT;
    let raw = (plot_height_pt / POINTS_PER_GRIDLINE).round() as i32;
    raw.clamp(MIN_TARGET_INTERVALS, MAX_TARGET_INTERVALS)
}

pub fn nice_axis_step(value: f64) -> f64 {
    if !value.is_finite() || value <= 0.0 {
        return 1.0;
    }
    let exponent = value.log10().floor() as i32;
    let power = 10f64.powi(exponent);
    let mantissa = value / power;
    let step = NICE_STEPS
        .iter()
        .find(|candidate| mantissa <= *candidate + 1e-12)
        .copied()
        .unwrap_or(10.0);
    step * power
}

fn snap(value: f64, unit: f64, up: bool) -> f64 {
    let scaled = value / unit;
    let rounded = if up {
        scaled.ceil() - 1e-9
    } else {
        scaled.floor() + 1e-9
    };
    rounded * unit
}

pub fn nice_value_axis_bounds(
    data_min: f64,
    data_max: f64,
    target_intervals: i32,
    headroom: f64,
) -> NiceAxisBounds {
    if !data_min.is_finite() || !data_max.is_finite() {
        return NiceAxisBounds {
            min: 0.0,
            max: 1.0,
            major_unit: 0.5,
        };
    }
    let low = data_min.min(data_max);
    let high = data_min.max(data_max);
    let intervals = target_intervals.max(1);

    if low == high {
        if low == 0.0 {
            return NiceAxisBounds {
                min: 0.0,
                max: 1.0,
                major_unit: 1.0 / intervals as f64,
            };
        }
        let unit = nice_axis_step(low.abs() / intervals as f64);
        if low > 0.0 {
            return NiceAxisBounds {
                min: 0.0,
                max: snap(low, unit, true).max(unit),
                major_unit: unit,
            };
        }
        return NiceAxisBounds {
            min: snap(low, unit, false),
            max: 0.0,
            major_unit: unit,
        };
    }

    let data_span = high - low;
    let padding = data_span * headroom;
    let (padded_min, padded_max) = if low >= 0.0 {
        let padded_min = if low < high * ZERO_ANCHOR_RATIO {
            0.0
        } else {
            low - padding
        };
        (padded_min, high + padding)
    } else if high <= 0.0 {
        let padded_max = if high > low * ZERO_ANCHOR_RATIO {
            0.0
        } else {
            high + padding
        };
        (low - padding, padded_max)
    } else {
        (low - padding, high + padding)
    };

    let unit = nice_axis_step((padded_max - padded_min) / intervals as f64);
    let min = if padded_min == 0.0 {
        0.0
    } else {
        snap(padded_min, unit, false)
    };
    let mut max = if padded_max == 0.0 {
        0.0
    } else {
        snap(padded_max, unit, true)
    };
    if max <= min {
        max = min + unit;
    }
    NiceAxisBounds {
        min: clean(min),
        max: clean(max),
        major_unit: unit,
    }
}

pub fn axis_scale_from_nice_bounds(bounds: NiceAxisBounds) -> AxisScale {
    let min = bounds.min;
    let max = bounds.max;
    let unit = bounds.major_unit;
    let intervals = ((max - min) / unit).round().max(1.0) as usize;
    let ticks: Vec<f64> = (0..=intervals)
        .map(|i| clean(min + i as f64 * unit))
        .collect();
    AxisScale {
        min,
        max,
        major_unit: unit,
        ticks,
    }
}
