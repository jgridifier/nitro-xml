// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-scale.ts

#[derive(Clone, Debug)]
pub struct AxisScale {
    pub min: f64,
    pub max: f64,
    pub major_unit: f64,
    pub ticks: Vec<f64>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NiceScaleOptions {
    pub max_intervals: Option<usize>,
    pub no_zero: bool,
}

fn clean(n: f64) -> f64 {
    let s = format!("{:.12}", n);
    s.parse().unwrap_or(n)
}

fn units(raw: f64) -> impl Iterator<Item = f64> {
    let mut exp = (raw.log10().floor() as i32) - 1;
    std::iter::from_fn(move || {
        let base = 10f64.powi(exp);
        exp += 1;
        Some([clean(base), clean(2.0 * base), clean(5.0 * base)])
    })
    .flatten()
}

pub fn nice_scale(data_min: f64, data_max: f64, options: NiceScaleOptions) -> AxisScale {
    let max_intervals = options.max_intervals.unwrap_or(10).max(1);
    let mut lo = if data_min.is_finite() { data_min } else { 0.0 };
    let mut hi = if data_max.is_finite() { data_max } else { 0.0 };
    if lo > hi {
        std::mem::swap(&mut lo, &mut hi);
    }
    if lo == hi {
        if lo == 0.0 {
            hi = 1.0;
        } else if options.no_zero {
            let pad = lo.abs() * 0.1.max(1.0);
            lo -= pad;
            hi += pad;
        } else if lo > 0.0 {
            lo = 0.0;
        } else {
            hi = 0.0;
        }
    }
    let mut fixed_lo = false;
    let mut fixed_hi = false;
    if !options.no_zero {
        if lo >= 0.0 && hi - lo >= hi / 6.0 {
            lo = 0.0;
            fixed_lo = true;
        } else if hi <= 0.0 && hi - lo >= -lo / 6.0 {
            hi = 0.0;
            fixed_hi = true;
        }
    }
    let range = hi - lo;
    let top = if fixed_hi { hi } else { hi + range * 0.05 };
    let bottom = if fixed_lo { lo } else { lo - range * 0.05 };
    for unit in units(range / max_intervals as f64) {
        let min = if fixed_lo {
            lo
        } else {
            (clean(bottom / unit)).floor() * unit
        };
        let max = if fixed_hi {
            hi
        } else {
            (clean(top / unit)).ceil() * unit
        };
        let intervals = ((max - min) / unit).round() as usize;
        if intervals <= max_intervals && intervals > 0 {
            let ticks: Vec<f64> = (0..=intervals)
                .map(|i| clean(min + i as f64 * unit))
                .collect();
            return AxisScale {
                min: clean(min),
                max: clean(max),
                major_unit: unit,
                ticks,
            };
        }
    }
    AxisScale {
        min: lo,
        max: hi,
        major_unit: range,
        ticks: vec![lo, hi],
    }
}

pub fn format_axis_value(value: f64, percent: bool) -> String {
    if percent {
        return format!("{}%", clean(value * 100.0));
    }
    let v = clean(value);
    if v.abs() >= 1e15 || (v != 0.0 && v.abs() < 1e-6) {
        let s = format!("{:.2e}", v);
        return s.replace("e+", "E+").replace("e-", "E-");
    }
    let abs = v.abs();
    let s = if abs.fract() == 0.0 {
        format!("{}", abs as i64)
    } else {
        format!("{}", abs)
    };
    let parts: Vec<&str> = s.split('.').collect();
    let int = parts[0];
    let frac = parts.get(1);
    let mut grouped = String::new();
    for (i, ch) in int.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            grouped.insert(0, ',');
        }
        grouped.insert(0, ch);
    }
    let sign = if v < 0.0 { "-" } else { "" };
    if let Some(frac) = frac {
        format!("{}{}.{}", sign, grouped, frac)
    } else {
        format!("{}{}", sign, grouped)
    }
}
