// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/dist-core.ts,
// distributions-f.ts (fCdf and the inverse helpers) and the distribution bodies in
// distributions.ts / distributions-more.ts; changes listed in PROVENANCE.md.
//! Incomplete gamma and beta, normal/t/chi/F inverses, and discrete probabilities.
//! `erf` is also the Engineering `ERF` family.
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]

use super::stats_core::gamma_ln;
use super::value::{ErrorCode, Fx};

const EPS: f64 = 1e-15;

pub fn gamma_p(a: f64, x: f64) -> Fx<f64> {
    if x <= 0.0 {
        return Ok(0.0);
    }
    let ln_prefix = -x + a * x.ln() - gamma_ln(a)?;
    if x < a + 1.0 {
        let mut ap = a;
        let mut sum = 1.0 / a;
        let mut del = sum;
        for _ in 0..1000 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * EPS {
                break;
            }
        }
        return Ok(sum * ln_prefix.exp());
    }
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / 1e-300;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..1000 {
        let an = -(i as f64) * (i as f64 - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < 1e-300 {
            d = 1e-300;
        }
        c = b + an / c;
        if c.abs() < 1e-300 {
            c = 1e-300;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < EPS {
            break;
        }
    }
    Ok(1.0 - ln_prefix.exp() * h)
}

pub fn beta_cf(a: f64, b: f64, x: f64) -> f64 {
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - (qab * x) / qap;
    if d.abs() < 1e-300 {
        d = 1e-300;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..=1000 {
        let m2 = 2.0 * m as f64;
        let mut aa = (m as f64 * (b - m as f64) * x) / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < 1e-300 {
            d = 1e-300;
        }
        c = 1.0 + aa / c;
        if c.abs() < 1e-300 {
            c = 1e-300;
        }
        d = 1.0 / d;
        h *= d * c;
        aa = (-(a + m as f64) * (qab + m as f64) * x) / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < 1e-300 {
            d = 1e-300;
        }
        c = 1.0 + aa / c;
        if c.abs() < 1e-300 {
            c = 1e-300;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < EPS {
            break;
        }
    }
    h
}

pub fn beta_i(x: f64, a: f64, b: f64) -> Fx<f64> {
    if x <= 0.0 {
        return Ok(0.0);
    }
    if x >= 1.0 {
        return Ok(1.0);
    }
    let bt =
        (gamma_ln(a + b)? - gamma_ln(a)? - gamma_ln(b)? + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        Ok(bt * beta_cf(a, b, x) / a)
    } else {
        Ok(1.0 - bt * beta_cf(b, a, 1.0 - x) / b)
    }
}

pub fn erf(x: f64) -> Fx<f64> {
    if x < 0.0 {
        Ok(-gamma_p(0.5, x * x)?)
    } else {
        gamma_p(0.5, x * x)
    }
}

pub fn norm_cdf(z: f64) -> Fx<f64> {
    Ok(0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2)?))
}

pub fn norm_pdf(z: f64) -> f64 {
    (-z * z / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

pub fn norm_inv(p: f64) -> Fx<f64> {
    if p <= 0.0 || p >= 1.0 {
        return Err(ErrorCode::Num);
    }
    let a = [
        -39.69683028665376,
        220.9460984245205,
        -275.9285104469687,
        138.357751867269,
        -30.66479806614716,
        2.506628277459239,
    ];
    let b = [
        -54.47609879822406,
        161.5858368580409,
        -155.6989798598866,
        66.80131188771972,
        -13.28068155288572,
    ];
    let c = [
        -0.007784894002430293,
        -0.3223964580411365,
        -2.400758277161838,
        -2.549732539343734,
        4.374664141464968,
        2.938163982698783,
    ];
    let d = [
        0.007784695709041462,
        0.3224671290700398,
        2.445134137142996,
        3.754408661907416,
    ];
    let x = if p < 0.02425 {
        let q = (-2.0 * p.ln()).sqrt();
        (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    } else if p <= 1.0 - 0.02425 {
        let q = p - 0.5;
        let r = q * q;
        ((((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q)
            / (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    };
    let e = norm_cdf(x)? - p;
    let u = e * (2.0 * std::f64::consts::PI).sqrt() * (x * x / 2.0).exp();
    Ok(x - u / (1.0 + (x * u) / 2.0))
}

pub fn invert(mut cdf: impl FnMut(f64) -> Fx<f64>, p: f64, lo: f64, hi: f64) -> Fx<f64> {
    let mut a = lo;
    let mut b = hi;
    while cdf(b)? < p && b < 1e10 {
        b *= 2.0;
    }
    for _ in 0..200 {
        let mid = (a + b) / 2.0;
        if cdf(mid)? < p {
            a = mid;
        } else {
            b = mid;
        }
        if b - a < 1e-15 * 1.0f64.max(mid.abs()) {
            break;
        }
    }
    Ok((a + b) / 2.0)
}

pub fn t_cdf(t: f64, df: f64) -> Fx<f64> {
    let x = df / (df + t * t);
    let tail = 0.5 * beta_i(x, df / 2.0, 0.5)?;
    if t >= 0.0 {
        Ok(1.0 - tail)
    } else {
        Ok(tail)
    }
}

pub fn t_pdf(t: f64, df: f64) -> Fx<f64> {
    Ok((gamma_ln((df + 1.0) / 2.0)?
        - gamma_ln(df / 2.0)?
        - 0.5 * (df * std::f64::consts::PI).ln()
        - ((df + 1.0) / 2.0) * (1.0 + (t * t) / df).ln())
    .exp())
}

pub fn chi_cdf(x: f64, df: f64) -> Fx<f64> {
    gamma_p(df / 2.0, x / 2.0)
}

pub fn positive(values: &[f64]) -> Fx<()> {
    if values.iter().any(|value| *value <= 0.0) {
        Err(ErrorCode::Num)
    } else {
        Ok(())
    }
}

pub fn probability(p: f64) -> Fx<()> {
    if p <= 0.0 || p >= 1.0 {
        Err(ErrorCode::Num)
    } else {
        Ok(())
    }
}

pub fn binom_pmf(k: f64, n: f64, p: f64) -> Fx<f64> {
    Ok(super::math::combin(n, k)? * p.powf(k) * (1.0 - p).powf(n - k))
}

pub fn binom_dist(k: f64, n: f64, p: f64, cumulative: f64) -> Fx<f64> {
    let kk = k.trunc();
    let nn = n.trunc();
    if kk < 0.0 || kk > nn || !(0.0..=1.0).contains(&p) {
        return Err(ErrorCode::Num);
    }
    if !super::ops::flag(cumulative) {
        return binom_pmf(kk, nn, p);
    }
    let mut total = 0.0;
    let mut i = 0.0;
    while i <= kk {
        total += binom_pmf(i, nn, p)?;
        i += 1.0;
    }
    Ok(total.min(1.0))
}

pub fn poisson(k: f64, mean: f64, cumulative: f64) -> Fx<f64> {
    let kk = k.trunc();
    if kk < 0.0 || mean < 0.0 {
        return Err(ErrorCode::Num);
    }
    if super::ops::flag(cumulative) {
        return Ok(1.0 - gamma_p(kk + 1.0, mean)?);
    }
    Ok((kk * mean.ln() - mean - gamma_ln(kk + 1.0)?).exp())
}

pub fn norm_dist(x: f64, m: f64, s: f64, cumulative: f64) -> Fx<f64> {
    positive(&[s])?;
    let z = (x - m) / s;
    if super::ops::flag(cumulative) {
        norm_cdf(z)
    } else {
        Ok(norm_pdf(z) / s)
    }
}

pub fn f_cdf(x: f64, d1: f64, d2: f64) -> Fx<f64> {
    if x <= 0.0 {
        Ok(0.0)
    } else {
        beta_i((d1 * x) / (d1 * x + d2), d1 / 2.0, d2 / 2.0)
    }
}

pub fn f_pdf(x: f64, d1: f64, d2: f64) -> Fx<f64> {
    if x == 0.0 {
        return if d1 == 2.0 {
            Ok(1.0)
        } else if d1 > 2.0 {
            Ok(0.0)
        } else {
            Err(ErrorCode::Num)
        };
    }
    let ln_b = gamma_ln(d1 / 2.0)? + gamma_ln(d2 / 2.0)? - gamma_ln((d1 + d2) / 2.0)?;
    Ok(
        (d1 / 2.0 * d1.ln() + d2 / 2.0 * d2.ln() + (d1 / 2.0 - 1.0) * x.ln()
            - ((d1 + d2) / 2.0) * (d2 + d1 * x).ln()
            - ln_b)
            .exp(),
    )
}

pub fn f_inverse(p: f64, d1: f64, d2: f64) -> Fx<f64> {
    if p == 0.0 {
        Ok(0.0)
    } else {
        invert(|x| f_cdf(x, d1, d2), p, 0.0, 10.0)
    }
}

pub fn beta_scaled(x: f64, a: f64, b: f64, lo: f64, hi: f64) -> Fx<f64> {
    if x < lo || x > hi || lo == hi {
        return Err(ErrorCode::Num);
    }
    beta_i((x - lo) / (hi - lo), a, b)
}

pub fn beta_inverse(p: f64, a: f64, b: f64, lo: f64, hi: f64) -> Fx<f64> {
    positive(&[a, b])?;
    if p <= 0.0 || p > 1.0 || lo >= hi {
        return Err(ErrorCode::Num);
    }
    Ok(lo + (hi - lo) * invert(|t| beta_i(t, a, b), p, 0.0, 1.0)?)
}

pub fn gamma_inverse(p: f64, a: f64, b: f64) -> Fx<f64> {
    if p < 0.0 || p > 1.0 {
        return Err(ErrorCode::Num);
    }
    positive(&[a, b])?;
    if p == 0.0 {
        Ok(0.0)
    } else {
        invert(|x| gamma_p(a, x / b), p, 0.0, a * b + 1.0)
    }
}

pub fn binom_inverse(trials: f64, p: f64, alpha: f64) -> Fx<f64> {
    let n = trials.trunc();
    if n < 0.0 {
        return Err(ErrorCode::Num);
    }
    if !(0.0..=1.0).contains(&p) || !(0.0..=1.0).contains(&alpha) {
        return Err(ErrorCode::Num);
    }
    let mut total = 0.0;
    let mut k = 0.0;
    while k <= n {
        total += binom_pmf(k, n, p)?;
        if total >= alpha {
            return Ok(k);
        }
        k += 1.0;
    }
    Ok(n)
}

pub fn hypgeom_pmf(k: f64, n: f64, big_k: f64, big_n: f64) -> Fx<f64> {
    let kk = k.trunc();
    let nn = n.trunc();
    let kk_pop = big_k.trunc();
    let nn_pop = big_n.trunc();
    if kk < 0.0
        || nn > nn_pop
        || kk_pop > nn_pop
        || kk > nn.min(kk_pop)
        || kk < nn - (nn_pop - kk_pop)
    {
        return Err(ErrorCode::Num);
    }
    Ok(
        super::math::combin(kk_pop, kk)? * super::math::combin(nn_pop - kk_pop, nn - kk)?
            / super::math::combin(nn_pop, nn)?,
    )
}

pub fn negbinom_pmf(f: f64, s: f64, p: f64) -> Fx<f64> {
    let ff = f.trunc();
    let ss = s.trunc();
    if ff < 0.0 || ss < 1.0 || !(0.0..=1.0).contains(&p) {
        return Err(ErrorCode::Num);
    }
    Ok(super::math::combin(ff + ss - 1.0, ss - 1.0)? * p.powf(ss) * (1.0 - p).powf(ff))
}
