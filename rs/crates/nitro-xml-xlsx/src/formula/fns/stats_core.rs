// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/stats-core.ts; changes listed in PROVENANCE.md.
//! Pure statistics on number lists, shared by the statistical functions, SUBTOTAL and AGGREGATE.
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::excessive_precision)]

use super::value::{ErrorCode, Fx};

pub fn sum(values: &[f64]) -> f64 {
    let mut total = 0.0;
    let mut largest = 0.0f64;
    for value in values {
        total += value;
        largest = largest.max(value.abs());
    }
    if total != 0.0 && total.abs() <= largest * 1e-15 {
        0.0
    } else {
        total
    }
}

pub fn mean(values: &[f64]) -> Fx<f64> {
    if values.is_empty() {
        Err(ErrorCode::Div0)
    } else {
        Ok(sum(values) / values.len() as f64)
    }
}

pub fn devsq(values: &[f64]) -> Fx<f64> {
    let m = mean(values)?;
    let mut total = 0.0;
    for value in values {
        total += (value - m) * (value - m);
    }
    Ok(total)
}

pub fn variance(values: &[f64], sample: bool) -> Fx<f64> {
    let n = values.len();
    if n == 0 || (sample && n < 2) {
        return Err(ErrorCode::Div0);
    }
    Ok(devsq(values)? / if sample { n as f64 - 1.0 } else { n as f64 })
}

pub fn stdev(values: &[f64], sample: bool) -> Fx<f64> {
    Ok(variance(values, sample)?.sqrt())
}

pub fn sorted(values: &[f64]) -> Vec<f64> {
    let mut out = values.to_vec();
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    out
}

pub fn median(values: &[f64]) -> Fx<f64> {
    if values.is_empty() {
        return Err(ErrorCode::Num);
    }
    let s = sorted(values);
    let mid = s.len() >> 1;
    if s.len() % 2 == 1 {
        Ok(s[mid])
    } else {
        Ok((s[mid - 1] + s[mid]) / 2.0)
    }
}

/// Most frequent values in order of first appearance. `#N/A` when nothing repeats.
/// `0` and `-0` share a key, matching JavaScript's SameValueZero `Map`.
pub fn modes(values: &[f64]) -> Fx<Vec<f64>> {
    let mut order: Vec<(f64, usize)> = Vec::new();
    let mut best = 1usize;
    for value in values {
        if let Some(slot) = order.iter_mut().find(|(key, _)| *key == *value) {
            slot.1 += 1;
            best = best.max(slot.1);
        } else {
            order.push((*value, 1));
        }
    }
    if best < 2 {
        return Err(ErrorCode::Na);
    }
    Ok(order
        .into_iter()
        .filter(|(_, count)| *count == best)
        .map(|(value, _)| value)
        .collect())
}

#[allow(clippy::neg_cmp_op_on_partial_ord)] // NaN is not `>= 1`, and Excel rejects it.
pub fn kth(values: &[f64], k: f64, largest: bool) -> Fx<f64> {
    let n = values.len();
    if n == 0 || !(k >= 1.0) || k > n as f64 {
        return Err(ErrorCode::Num);
    }
    let index = (if largest { n as f64 + 1.0 - k } else { k }).floor() - 1.0;
    let s = sorted(values);
    let at = index as usize;
    s.get(at).copied().ok_or(ErrorCode::Num)
}

pub fn extreme(values: &[f64], max: bool) -> f64 {
    let Some(first) = values.first().copied() else {
        return 0.0;
    };
    let mut best = first;
    for value in values {
        if max {
            if *value > best {
                best = *value;
            }
        } else if *value < best {
            best = *value;
        }
    }
    best
}

pub fn percentile_inc(values: &[f64], p: f64) -> Fx<f64> {
    if values.is_empty() || !(0.0..=1.0).contains(&p) {
        return Err(ErrorCode::Num);
    }
    let s = sorted(values);
    let h = (s.len() - 1) as f64 * p;
    let lo = h.floor() as usize;
    let a = s[lo];
    let b = s[lo.saturating_add(1).min(s.len() - 1)];
    Ok(a + (h - lo as f64) * (b - a))
}

pub fn percentile_exc(values: &[f64], p: f64) -> Fx<f64> {
    let n = values.len();
    if n == 0 || p <= 0.0 || p >= 1.0 {
        return Err(ErrorCode::Num);
    }
    let h = (n as f64 + 1.0) * p;
    if h < 1.0 || h > n as f64 {
        return Err(ErrorCode::Num);
    }
    let s = sorted(values);
    let lo = h.floor() as usize;
    let a = s[lo - 1];
    let b = s[lo.min(n - 1)];
    Ok(a + (h - lo as f64) * (b - a))
}

pub fn quartile(values: &[f64], q: f64, exclusive: bool) -> Fx<f64> {
    let quart = q.trunc();
    if exclusive {
        if quart <= 0.0 || quart >= 4.0 {
            return Err(ErrorCode::Num);
        }
        percentile_exc(values, quart / 4.0)
    } else {
        if quart < 0.0 || quart > 4.0 {
            return Err(ErrorCode::Num);
        }
        percentile_inc(values, quart / 4.0)
    }
}

pub fn percent_rank(values: &[f64], x: f64, digits: f64, exclusive: bool) -> Fx<f64> {
    let s = sorted(values);
    let n = s.len();
    if n == 0 || digits < 1.0 {
        return Err(ErrorCode::Num);
    }
    let first = s[0];
    let last = s[n - 1];
    if x < first || x > last {
        return Err(ErrorCode::Na);
    }
    let lower = s.iter().filter(|value| **value < x).count();
    let exact = s.contains(&x);
    let rank = if exact {
        if exclusive {
            (lower as f64 + 1.0) / (n as f64 + 1.0)
        } else if n == 1 {
            1.0
        } else {
            lower as f64 / (n as f64 - 1.0)
        }
    } else {
        let below = s[lower - 1];
        let above = s[lower];
        let frac = (x - below) / (above - below);
        if exclusive {
            (lower as f64 + frac) / (n as f64 + 1.0)
        } else {
            (lower as f64 - 1.0 + frac) / (n as f64 - 1.0)
        }
    };
    let factor = 10f64.powf(digits.trunc());
    Ok((rank * factor + 1e-9).floor() / factor)
}

pub fn covariance(xs: &[f64], ys: &[f64], sample: bool) -> Fx<f64> {
    if xs.len() != ys.len() {
        return Err(ErrorCode::Na);
    }
    let n = xs.len();
    if n == 0 || (sample && n < 2) {
        return Err(ErrorCode::Div0);
    }
    let mx = mean(xs)?;
    let my = mean(ys)?;
    let mut total = 0.0;
    for i in 0..n {
        total += (xs[i] - mx) * (ys[i] - my);
    }
    Ok(total / if sample { n as f64 - 1.0 } else { n as f64 })
}

pub fn correlation(xs: &[f64], ys: &[f64]) -> Fx<f64> {
    let sx = devsq(xs)?.sqrt();
    let sy = devsq(ys)?.sqrt();
    if sx == 0.0 || sy == 0.0 {
        return Err(ErrorCode::Div0);
    }
    Ok(covariance(xs, ys, false)? * xs.len() as f64 / (sx * sy))
}

pub struct LinearFit {
    pub slope: f64,
    pub intercept: f64,
}

pub fn linear_fit(xs: &[f64], ys: &[f64]) -> Fx<LinearFit> {
    if xs.len() != ys.len() {
        return Err(ErrorCode::Na);
    }
    if xs.is_empty() {
        return Err(ErrorCode::Div0);
    }
    let mx = mean(xs)?;
    let my = mean(ys)?;
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    for i in 0..xs.len() {
        let dx = xs[i] - mx;
        sxy += dx * (ys[i] - my);
        sxx += dx * dx;
    }
    if sxx == 0.0 {
        return Err(ErrorCode::Div0);
    }
    let slope = sxy / sxx;
    Ok(LinearFit {
        slope,
        intercept: my - slope * mx,
    })
}

pub fn rank(values: &[f64], x: f64, ascending: bool, average: bool) -> Fx<f64> {
    let mut better = 0.0;
    let mut ties = 0.0;
    for value in values {
        if *value == x {
            ties += 1.0;
        } else if if ascending { *value < x } else { *value > x } {
            better += 1.0;
        }
    }
    if ties == 0.0 {
        return Err(ErrorCode::Na);
    }
    if average {
        Ok(better + (ties + 1.0) / 2.0)
    } else {
        Ok(better + 1.0)
    }
}

const LANCZOS: [f64; 8] = [
    676.5203681218851,
    -1259.1392167224028,
    771.32342877765313,
    -176.61502916214059,
    12.507343278686905,
    -0.13857109526572012,
    9.9843695780195716e-6,
    1.5056327351493116e-7,
];

pub fn gamma(z: f64) -> f64 {
    if z < 0.5 {
        return std::f64::consts::PI / ((std::f64::consts::PI * z).sin() * gamma(1.0 - z));
    }
    let x = z - 1.0;
    let mut a = 0.99999999999980993;
    let t = x + 7.5;
    for (i, coeff) in LANCZOS.iter().enumerate() {
        a += coeff / (x + i as f64 + 1.0);
    }
    (2.0 * std::f64::consts::PI).sqrt() * t.powf(x + 0.5) * (-t).exp() * a
}

pub fn gamma_ln(z: f64) -> Fx<f64> {
    if z <= 0.0 {
        return Err(ErrorCode::Num);
    }
    if z < 0.5 {
        return Ok(
            (std::f64::consts::PI / (std::f64::consts::PI * z).sin().abs()).ln()
                - gamma_ln(1.0 - z)?,
        );
    }
    let x = z - 1.0;
    let mut a = 0.99999999999980993;
    let t = x + 7.5;
    for (i, coeff) in LANCZOS.iter().enumerate() {
        a += coeff / (x + i as f64 + 1.0);
    }
    Ok(0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln())
}
