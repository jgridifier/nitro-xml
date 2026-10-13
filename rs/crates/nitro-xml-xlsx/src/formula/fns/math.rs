// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/{math,math-sum,math-mod,math-combin,math-more,matrix}.ts
// and ROMAN/ARABIC/BASE/DECIMAL in engineering.ts; changes listed in PROVENANCE.md.
//! Math & Trig functions.
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::needless_range_loop)]

use std::sync::OnceLock;

use regex::Regex;

use super::criteria::{criteria_pairs, lift_criteria, matching_values, CriteriaPair};
use super::helpers::{collect_numbers, int_of, num, numbers_of, str_of};
use super::host::CallCtx;
use super::ops::{flag, js_round, js_sign, power};
use super::spec::{gen_spec, num_spec, FunctionSpec, ParamKind};
use super::stats_core::{self as stats};
use super::text_num::round15;
use super::value::{ErrorCode, Fx, Matrix, Scalar, Value};

const C: &str = "Math & Trig";
const ANY: &[ParamKind] = &[ParamKind::Any];
const VALUE_ANY: &[ParamKind] = &[ParamKind::Value, ParamKind::Any];
const SUMIF_PARAMS: &[ParamKind] = &[ParamKind::Any, ParamKind::Value, ParamKind::Any];
const SERIES_PARAMS: &[ParamKind] = &[
    ParamKind::Value,
    ParamKind::Value,
    ParamKind::Value,
    ParamKind::Any,
];

const SPLIT: f64 = 134_217_729.0;
const MOD_LIMIT: f64 = 1.1259e12;
const HALF: f64 = 0.5 - 1.0 / ((1u64 << 48) as f64);

fn product_error(a: f64, b: f64, p: f64) -> f64 {
    let ca = SPLIT * a;
    let ah = ca - (ca - a);
    let al = a - ah;
    let cb = SPLIT * b;
    let bh = cb - (cb - b);
    let bl = b - bh;
    ah * bh - p + ah * bl + al * bh + al * bl
}

fn remainder(n: f64, d: f64, q: f64) -> f64 {
    let p = q * d;
    n - p - product_error(q, d, p)
}

/// Excel `MOD`: the remainder takes the divisor's sign and huge quotients are `#NUM!`.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // NaN must fail the limit test, matching `!(abs < limit)`.
pub(crate) fn excel_mod(n: f64, d: f64) -> Fx<f64> {
    if d == 0.0 {
        return Err(ErrorCode::Div0);
    }
    let ratio = n / d;
    if !(ratio.abs() < MOD_LIMIT) {
        return Err(ErrorCode::Num);
    }
    let mut q = ratio.floor();
    let mut r = remainder(n, d, q);
    if r != 0.0 && (r < 0.0) != (d < 0.0) {
        q -= 1.0;
        r = remainder(n, d, q);
    } else if r.abs() >= d.abs() {
        q += 1.0;
        r = remainder(n, d, q);
    }
    Ok(if r == 0.0 { 0.0 } else { r })
}

pub(crate) fn mround(x: f64, m: f64) -> Fx<f64> {
    if m == 0.0 || x == 0.0 {
        return Ok(0.0);
    }
    if (x < 0.0) != (m < 0.0) {
        return Err(ErrorCode::Num);
    }
    let q = x / m;
    let whole = q.floor();
    let n = if q - whole >= HALF {
        whole + 1.0
    } else {
        whole
    };
    let result = n * m;
    Ok(if result == 0.0 { 0.0 } else { result })
}

enum RoundMode {
    Half,
    Up,
    Down,
}

pub(crate) fn round_half_digits(x: f64, digits: f64) -> f64 {
    round_to(x, digits, RoundMode::Half)
}

fn round_to(x: f64, digits: f64, mode: RoundMode) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let d = digits.trunc();
    if d > 15.0 {
        return x;
    }
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let factor = 10f64.powf(d.abs());
    let scaled = round15(if d >= 0.0 {
        x.abs() * factor
    } else {
        x.abs() / factor
    });
    let rounded = match mode {
        RoundMode::Half => (scaled + 0.5).floor(),
        RoundMode::Up => scaled.ceil(),
        RoundMode::Down => scaled.floor(),
    };
    let result = if d >= 0.0 {
        rounded / factor
    } else {
        rounded * factor
    };
    if result == 0.0 {
        0.0
    } else {
        sign * result
    }
}

fn quotient_multiple(x: f64, sig: f64, op: fn(f64) -> f64) -> f64 {
    if sig == 0.0 {
        0.0
    } else {
        op(round15(x / sig)) * sig
    }
}

/// Factorial. Arguments above 170 overflow to infinity (then `#NUM!`) without a long loop.
pub(crate) fn fact(n: f64) -> Fx<f64> {
    if n < 0.0 {
        return Err(ErrorCode::Num);
    }
    let t = n.trunc();
    if t > 170.0 {
        return Ok(f64::INFINITY);
    }
    let mut out = 1.0;
    let mut i = 2.0;
    while i <= t {
        out *= i;
        i += 1.0;
    }
    Ok(out)
}

pub(crate) fn combin(n: f64, k: f64) -> Fx<f64> {
    let nn = n.trunc();
    let kk = k.trunc();
    if nn < 0.0 || kk < 0.0 || kk > nn {
        return Err(ErrorCode::Num);
    }
    let mut out = 1.0;
    let lim = kk.min(nn - kk);
    let mut i = 1.0;
    while i <= lim {
        out = (out * (nn - lim + i)) / i;
        if !out.is_finite() {
            break;
        }
        i += 1.0;
    }
    Ok(js_round(out))
}

fn gcd2(mut a: f64, mut b: f64) -> f64 {
    while b != 0.0 {
        let next = a % b;
        a = b;
        b = next;
    }
    a
}

fn integers(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Vec<f64>> {
    let mut out = Vec::new();
    for n in collect_numbers(ctx, args, false, false)? {
        if n < 0.0 {
            return Err(ErrorCode::Num);
        }
        out.push(n.trunc());
    }
    Ok(out)
}

pub(crate) struct Elimination {
    pub det: f64,
    pub inverse: Option<Vec<Vec<f64>>>,
}

pub(crate) fn eliminate(input: &[Vec<f64>], invert: bool) -> Fx<Elimination> {
    let n = input.len();
    if n == 0 || input.iter().any(|row| row.len() != n) {
        return Err(ErrorCode::Value);
    }
    let mut a = input.to_vec();
    let mut inv = vec![vec![0.0; n]; n];
    for i in 0..n {
        inv[i][i] = 1.0;
    }
    let mut det = 1.0;
    for col in 0..n {
        let mut pivot = col;
        for r in (col + 1)..n {
            if a[r][col].abs() > a[pivot][col].abs() {
                pivot = r;
            }
        }
        let p = a[pivot][col];
        if p.abs() < 1e-300 {
            return Ok(Elimination {
                det: 0.0,
                inverse: None,
            });
        }
        if pivot != col {
            a.swap(pivot, col);
            inv.swap(pivot, col);
            det = -det;
        }
        det *= p;
        for j in 0..n {
            a[col][j] /= p;
            inv[col][j] /= p;
        }
        for r in 0..n {
            if r == col {
                continue;
            }
            let factor = a[r][col];
            if factor == 0.0 {
                continue;
            }
            for j in 0..n {
                a[r][j] -= factor * a[col][j];
                inv[r][j] -= factor * inv[col][j];
            }
        }
    }
    Ok(Elimination {
        det,
        inverse: invert.then_some(inv),
    })
}

fn numeric_rows(ctx: &CallCtx<'_>, value: &Value) -> Fx<Vec<Vec<f64>>> {
    let matrix = ctx.to_matrix(value)?;
    let data = matrix.data()?;
    let mut rows = Vec::with_capacity(data.len());
    for line in data {
        let mut nums = Vec::with_capacity(line.len());
        for cell in line {
            match cell {
                Scalar::Error(code) => return Err(code),
                Scalar::Number(n) => nums.push(n),
                _ => return Err(ErrorCode::Value),
            }
        }
        rows.push(nums);
    }
    Ok(rows)
}

fn fn_abs(a: &[f64]) -> Fx<f64> {
    Ok(a[0].abs())
}
fn fn_sign(a: &[f64]) -> Fx<f64> {
    Ok(js_sign(a[0]))
}
fn fn_int(a: &[f64]) -> Fx<f64> {
    Ok(a[0].floor())
}
fn fn_trunc(a: &[f64]) -> Fx<f64> {
    Ok(round_to(
        a[0],
        a.get(1).copied().unwrap_or(0.0),
        RoundMode::Down,
    ))
}
fn fn_round(a: &[f64]) -> Fx<f64> {
    Ok(round_to(
        a[0],
        a.get(1).copied().unwrap_or(0.0),
        RoundMode::Half,
    ))
}
fn fn_roundup(a: &[f64]) -> Fx<f64> {
    Ok(round_to(
        a[0],
        a.get(1).copied().unwrap_or(0.0),
        RoundMode::Up,
    ))
}
fn fn_mround(a: &[f64]) -> Fx<f64> {
    mround(a[0], a.get(1).copied().unwrap_or(0.0))
}
fn fn_ceiling(a: &[f64]) -> Fx<f64> {
    let s = a.get(1).copied().unwrap_or(0.0);
    if a[0] > 0.0 && s < 0.0 {
        return Err(ErrorCode::Num);
    }
    Ok(quotient_multiple(a[0], s, f64::ceil))
}
fn fn_floor(a: &[f64]) -> Fx<f64> {
    let s = *a.get(1).unwrap_or(&0.0);
    if s == 0.0 {
        return Err(ErrorCode::Div0);
    }
    if a[0] > 0.0 && s < 0.0 {
        return Err(ErrorCode::Num);
    }
    Ok(quotient_multiple(a[0], s, f64::floor))
}
fn fn_ceiling_math(a: &[f64]) -> Fx<f64> {
    let sig = a.get(1).copied().unwrap_or(1.0).abs();
    let mode = a.get(2).copied().unwrap_or(0.0);
    if a[0] < 0.0 && flag(mode) {
        Ok(-quotient_multiple(-a[0], sig, f64::ceil))
    } else {
        Ok(quotient_multiple(a[0], sig, f64::ceil))
    }
}
fn fn_floor_math(a: &[f64]) -> Fx<f64> {
    let sig = a.get(1).copied().unwrap_or(1.0).abs();
    let mode = a.get(2).copied().unwrap_or(0.0);
    if a[0] < 0.0 && flag(mode) {
        Ok(-quotient_multiple(-a[0], sig, f64::floor))
    } else {
        Ok(quotient_multiple(a[0], sig, f64::floor))
    }
}
fn fn_ceiling_precise(a: &[f64]) -> Fx<f64> {
    Ok(quotient_multiple(
        a[0],
        a.get(1).copied().unwrap_or(1.0).abs(),
        f64::ceil,
    ))
}
fn fn_floor_precise(a: &[f64]) -> Fx<f64> {
    Ok(quotient_multiple(
        a[0],
        a.get(1).copied().unwrap_or(1.0).abs(),
        f64::floor,
    ))
}
fn fn_even(a: &[f64]) -> Fx<f64> {
    let x = a[0];
    Ok(js_sign(x) * (round15(x.abs()) / 2.0).ceil() * 2.0)
}
fn fn_odd(a: &[f64]) -> Fx<f64> {
    let x = a[0];
    let n = round15(x.abs()).ceil();
    let mag = if n % 2.0 == 0.0 { n + 1.0 } else { n };
    Ok((if x < 0.0 { -1.0 } else { 1.0 }) * mag)
}
fn fn_mod(a: &[f64]) -> Fx<f64> {
    excel_mod(a[0], a.get(1).copied().unwrap_or(0.0))
}
fn fn_quotient(a: &[f64]) -> Fx<f64> {
    let d = a.get(1).copied().unwrap_or(1.0);
    if d == 0.0 {
        return Err(ErrorCode::Div0);
    }
    Ok((a[0] / d).trunc())
}
fn fn_sqrt(a: &[f64]) -> Fx<f64> {
    if a[0] < 0.0 {
        Err(ErrorCode::Num)
    } else {
        Ok(a[0].sqrt())
    }
}
fn fn_sqrtpi(a: &[f64]) -> Fx<f64> {
    if a[0] < 0.0 {
        Err(ErrorCode::Num)
    } else {
        Ok((a[0] * std::f64::consts::PI).sqrt())
    }
}
fn fn_power(a: &[f64]) -> Fx<f64> {
    power(a[0], a.get(1).copied().unwrap_or(1.0))
}
fn fn_exp(a: &[f64]) -> Fx<f64> {
    Ok(a[0].exp())
}
fn fn_ln(a: &[f64]) -> Fx<f64> {
    if a[0] > 0.0 {
        Ok(a[0].ln())
    } else {
        Err(ErrorCode::Num)
    }
}
fn fn_log10(a: &[f64]) -> Fx<f64> {
    if a[0] > 0.0 {
        Ok(a[0].log10())
    } else {
        Err(ErrorCode::Num)
    }
}
fn fn_log(a: &[f64]) -> Fx<f64> {
    let base = a.get(1).copied().unwrap_or(10.0);
    if a[0] <= 0.0 || base <= 0.0 {
        return Err(ErrorCode::Num);
    }
    if base == 1.0 {
        return Err(ErrorCode::Div0);
    }
    if base == 10.0 {
        Ok(a[0].log10())
    } else {
        Ok(a[0].ln() / base.ln())
    }
}
fn fn_pi(_: &[f64]) -> Fx<f64> {
    Ok(std::f64::consts::PI)
}
fn fn_fact(a: &[f64]) -> Fx<f64> {
    fact(a[0])
}
fn fn_factdouble(a: &[f64]) -> Fx<f64> {
    if a[0] < -1.0 {
        return Err(ErrorCode::Num);
    }
    let mut out = 1.0;
    let mut i = a[0].trunc();
    while i > 1.0 {
        out *= i;
        i -= 2.0;
    }
    Ok(out)
}
fn fn_combin(a: &[f64]) -> Fx<f64> {
    combin(a[0], a.get(1).copied().unwrap_or(0.0))
}
fn fn_combina(a: &[f64]) -> Fx<f64> {
    let k = a.get(1).copied().unwrap_or(0.0);
    if a[0] < 0.0 || k < 0.0 {
        return Err(ErrorCode::Num);
    }
    if a[0].trunc() == 0.0 && k.trunc() == 0.0 {
        Ok(1.0)
    } else {
        combin(a[0] + k - 1.0, k)
    }
}

fn fn_sin(a: &[f64]) -> Fx<f64> {
    Ok(a[0].sin())
}
fn fn_cos(a: &[f64]) -> Fx<f64> {
    Ok(a[0].cos())
}
fn fn_tan(a: &[f64]) -> Fx<f64> {
    Ok(a[0].tan())
}
fn fn_asin(a: &[f64]) -> Fx<f64> {
    if a[0].abs() <= 1.0 {
        Ok(a[0].asin())
    } else {
        Err(ErrorCode::Num)
    }
}
fn fn_acos(a: &[f64]) -> Fx<f64> {
    if a[0].abs() <= 1.0 {
        Ok(a[0].acos())
    } else {
        Err(ErrorCode::Num)
    }
}
fn fn_atan(a: &[f64]) -> Fx<f64> {
    Ok(a[0].atan())
}
fn fn_atan2(a: &[f64]) -> Fx<f64> {
    let y = a.get(1).copied().unwrap_or(0.0);
    if a[0] == 0.0 && y == 0.0 {
        return Err(ErrorCode::Div0);
    }
    Ok(y.atan2(a[0]))
}
fn fn_sinh(a: &[f64]) -> Fx<f64> {
    Ok(a[0].sinh())
}
fn fn_cosh(a: &[f64]) -> Fx<f64> {
    Ok(a[0].cosh())
}
fn fn_tanh(a: &[f64]) -> Fx<f64> {
    Ok(a[0].tanh())
}
fn fn_asinh(a: &[f64]) -> Fx<f64> {
    Ok(a[0].asinh())
}
fn fn_acosh(a: &[f64]) -> Fx<f64> {
    if a[0] >= 1.0 {
        Ok(a[0].acosh())
    } else {
        Err(ErrorCode::Num)
    }
}
fn fn_atanh(a: &[f64]) -> Fx<f64> {
    if a[0].abs() < 1.0 {
        Ok(a[0].atanh())
    } else {
        Err(ErrorCode::Num)
    }
}
fn fn_cot(a: &[f64]) -> Fx<f64> {
    if a[0] == 0.0 {
        Err(ErrorCode::Div0)
    } else {
        Ok(1.0 / a[0].tan())
    }
}
fn fn_coth(a: &[f64]) -> Fx<f64> {
    if a[0] == 0.0 {
        Err(ErrorCode::Div0)
    } else {
        Ok(1.0 / a[0].tanh())
    }
}
fn fn_csc(a: &[f64]) -> Fx<f64> {
    if a[0] == 0.0 {
        Err(ErrorCode::Div0)
    } else {
        Ok(1.0 / a[0].sin())
    }
}
fn fn_csch(a: &[f64]) -> Fx<f64> {
    if a[0] == 0.0 {
        Err(ErrorCode::Div0)
    } else {
        Ok(1.0 / a[0].sinh())
    }
}
fn fn_sec(a: &[f64]) -> Fx<f64> {
    Ok(1.0 / a[0].cos())
}
fn fn_sech(a: &[f64]) -> Fx<f64> {
    Ok(1.0 / a[0].cosh())
}
fn fn_acot(a: &[f64]) -> Fx<f64> {
    Ok(std::f64::consts::FRAC_PI_2 - a[0].atan())
}
fn fn_acoth(a: &[f64]) -> Fx<f64> {
    if a[0].abs() > 1.0 {
        Ok(0.5 * ((a[0] + 1.0) / (a[0] - 1.0)).ln())
    } else {
        Err(ErrorCode::Num)
    }
}
fn fn_degrees(a: &[f64]) -> Fx<f64> {
    Ok(a[0] * 180.0 / std::f64::consts::PI)
}
fn fn_radians(a: &[f64]) -> Fx<f64> {
    Ok(a[0] * std::f64::consts::PI / 180.0)
}

fn fn_multinomial(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = integers(ctx, args)?;
    let total: f64 = values.iter().sum();
    let mut acc = fact(total)?;
    for value in values {
        acc /= fact(value)?;
    }
    Ok(Value::number(acc))
}
fn fn_gcd(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = integers(ctx, args)?;
    Ok(Value::number(values.into_iter().fold(0.0, gcd2)))
}
fn fn_lcm(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = integers(ctx, args)?;
    Ok(Value::number(values.into_iter().fold(1.0, |a, b| {
        if a == 0.0 || b == 0.0 {
            0.0
        } else {
            (a / gcd2(a, b)) * b
        }
    })))
}

fn fn_rand(ctx: &CallCtx<'_>, _args: &[Value]) -> Fx<Value> {
    Ok(Value::number(ctx.random()))
}
fn fn_randbetween(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let lo = num(ctx, &args[0])?.ceil();
    let hi = num(ctx, args.get(1).unwrap_or(&Value::blank()))?.floor();
    if lo > hi {
        return Err(ErrorCode::Num);
    }
    Ok(Value::number(lo + (ctx.random() * (hi - lo + 1.0)).floor()))
}
fn fn_series(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let x = num(ctx, &args[0])?;
    let n = num(ctx, args.get(1).unwrap_or(&Value::blank()))?;
    let m = num(ctx, args.get(2).unwrap_or(&Value::blank()))?;
    let coeffs = collect_numbers(
        ctx,
        &[args.get(3).cloned().unwrap_or_else(Value::blank)],
        false,
        false,
    )?;
    let mut total = 0.0;
    for (i, a) in coeffs.iter().enumerate() {
        total += a * x.powf(n + i as f64 * m);
    }
    Ok(Value::number(total))
}

fn fn_mmult(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let a = numeric_rows(ctx, &args[0])?;
    let b = numeric_rows(ctx, args.get(1).unwrap_or(&Value::blank()))?;
    let inner = a.first().map(|row| row.len()).unwrap_or(0);
    if inner != b.len() {
        return Err(ErrorCode::Value);
    }
    let cols = b.first().map(|row| row.len()).unwrap_or(0);
    let matrix = Matrix::build(a.len(), cols, |r, c| {
        let mut total = 0.0;
        for k in 0..inner {
            total += a[r].get(k).copied().unwrap_or(0.0) * b[k].get(c).copied().unwrap_or(0.0);
        }
        Ok(Scalar::Number(total))
    })?;
    Ok(Value::Matrix(matrix))
}
fn fn_mdeterm(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(
        eliminate(&numeric_rows(ctx, &args[0])?, false)?.det,
    ))
}
fn fn_minverse(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let solved = eliminate(&numeric_rows(ctx, &args[0])?, true)?;
    let Some(inverse) = solved.inverse else {
        return Err(ErrorCode::Num);
    };
    if solved.det == 0.0 {
        return Err(ErrorCode::Num);
    }
    let rows = inverse
        .into_iter()
        .map(|line| line.into_iter().map(Scalar::Number).collect())
        .collect();
    Ok(Value::Matrix(Matrix::from_rows(rows)))
}
fn fn_munit(_ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let n = int_of(_ctx, &args[0])?;
    if n < 1.0 {
        return Err(ErrorCode::Value);
    }
    let n = n as usize;
    Ok(Value::Matrix(Matrix::build(n, n, |r, c| {
        Ok(Scalar::Number(if r == c { 1.0 } else { 0.0 }))
    })?))
}

fn sum_product(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<f64> {
    let mut matrices = Vec::new();
    for arg in args {
        matrices.push(ctx.to_matrix(arg)?);
    }
    let Some(first) = matrices.first() else {
        return Err(ErrorCode::Value);
    };
    let rows = first.rows();
    let cols = first.cols();
    for matrix in &matrices {
        if matrix.rows() != rows || matrix.cols() != cols {
            return Err(ErrorCode::Value);
        }
    }
    let mut total = 0.0;
    for r in 0..rows {
        for c in 0..cols {
            let mut product = 1.0;
            for matrix in &matrices {
                match matrix.get(r, c) {
                    Scalar::Error(code) => return Err(code),
                    Scalar::Number(n) => product *= n,
                    _ => product *= 0.0,
                }
            }
            total += product;
        }
    }
    Ok(total)
}

fn pairwise(ctx: &CallCtx<'_>, args: &[Value], func: fn(f64, f64) -> f64) -> Fx<f64> {
    let xs = ctx
        .to_matrix(args.first().unwrap_or(&Value::blank()))?
        .flat()?;
    let ys = ctx
        .to_matrix(args.get(1).unwrap_or(&Value::blank()))?
        .flat()?;
    if xs.len() != ys.len() {
        return Err(ErrorCode::Na);
    }
    let mut total = 0.0;
    for (x, y) in xs.iter().zip(ys.iter()) {
        if let Scalar::Error(code) = x {
            return Err(*code);
        }
        if let Scalar::Error(code) = y {
            return Err(*code);
        }
        if let (Scalar::Number(x), Scalar::Number(y)) = (x, y) {
            total += func(*x, *y);
        }
    }
    Ok(total)
}

fn subtotal_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(?:_xlfn\.)?(?:SUBTOTAL|AGGREGATE)\s*\(").expect("subtotal pattern")
    })
}

/// Skips nested subtotals and, when asked, hidden rows.
/// `TODO(#72)`: autofilter criteria. Without a filter, filtered-out is always false, matching
/// `isFilteredRow` on a sheet that has no `autoFilter`.
fn subtotal_values(
    ctx: &CallCtx<'_>,
    args: &[Value],
    skip_hidden: bool,
    skip_errors: bool,
    skip_nested: bool,
) -> Fx<Vec<Scalar>> {
    let mut out = Vec::new();
    for arg in args {
        let Value::Ref(reference) = arg else {
            ctx.for_each(arg, &mut |value, _| {
                if !(skip_errors && value.is_error()) {
                    out.push(value.clone());
                }
            });
            continue;
        };
        for area in &reference.areas {
            ctx.host
                .for_each_stored(area.sheet, *area, &mut |value, row, col| {
                    if skip_hidden && ctx.host.row_hidden(area.sheet, row) {
                        return;
                    }
                    if skip_nested
                        && ctx
                            .host
                            .cell_formula(area.sheet, row, col)
                            .is_some_and(|text| subtotal_re().is_match(&text))
                    {
                        return;
                    }
                    if skip_errors && value.is_error() {
                        return;
                    }
                    out.push(value);
                });
        }
    }
    Ok(out)
}

pub(crate) fn aggregate(code: i32, values: &[Scalar], k: Option<f64>) -> Fx<f64> {
    if code == 2 {
        return Ok(numbers_of(values).len() as f64);
    }
    if code == 3 {
        return Ok(values
            .iter()
            .filter(|value| !matches!(value, Scalar::Blank))
            .count() as f64);
    }
    for value in values {
        if let Scalar::Error(code) = value {
            return Err(*code);
        }
    }
    let nums = numbers_of(values);
    match code {
        1 => stats::mean(&nums),
        4 => Ok(stats::extreme(&nums, true)),
        5 => Ok(stats::extreme(&nums, false)),
        6 => Ok(if nums.is_empty() {
            0.0
        } else {
            nums.iter().product()
        }),
        7 => stats::stdev(&nums, true),
        8 => stats::stdev(&nums, false),
        9 => Ok(stats::sum(&nums)),
        10 => stats::variance(&nums, true),
        11 => stats::variance(&nums, false),
        12 => stats::median(&nums),
        13 => Ok(stats::modes(&nums)?[0]),
        14 => stats::kth(&nums, k.unwrap_or(1.0), true),
        15 => stats::kth(&nums, k.unwrap_or(1.0), false),
        16 => stats::percentile_inc(&nums, k.unwrap_or(0.0)),
        17 => stats::quartile(&nums, k.unwrap_or(0.0), false),
        18 => stats::percentile_exc(&nums, k.unwrap_or(0.0)),
        19 => stats::quartile(&nums, k.unwrap_or(0.0), true),
        _ => Err(ErrorCode::Value),
    }
}

fn fn_sum(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(stats::sum(&collect_numbers(
        ctx, args, false, false,
    )?)))
}
fn fn_product(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let nums = collect_numbers(ctx, args, false, false)?;
    Ok(Value::number(if nums.is_empty() {
        0.0
    } else {
        nums.iter().product()
    }))
}
fn fn_sumsq(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(
        collect_numbers(ctx, args, false, false)?
            .iter()
            .fold(0.0, |acc, n| acc + n * n),
    ))
}
fn fn_sumproduct(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(sum_product(ctx, args)?))
}
fn fn_sumx2my2(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(pairwise(ctx, args, |x, y| x * x - y * y)?))
}
fn fn_sumx2py2(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(pairwise(ctx, args, |x, y| x * x + y * y)?))
}
fn fn_sumxmy2(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(pairwise(ctx, args, |x, y| {
        (x - y) * (x - y)
    })?))
}
fn fn_sumif(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let pairs = [CriteriaPair {
        range: args.first().cloned().unwrap_or_else(Value::blank),
        criteria: args.get(1).cloned().unwrap_or_else(Value::blank),
    }];
    let target = args.get(2);
    Ok(Value::number(stats::sum(&numbers_of(&matching_values(
        ctx, &pairs, target, false,
    )?))))
}
fn fn_sumifs(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let pairs = criteria_pairs(args, 1)?;
    let sum_range = args.first().cloned().unwrap_or_else(Value::blank);
    lift_criteria(ctx, &pairs, |pairs| {
        Ok(Value::number(stats::sum(&numbers_of(&matching_values(
            ctx,
            pairs,
            Some(&sum_range),
            true,
        )?))))
    })
}
fn fn_subtotal(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let code = int_of(ctx, &args[0])? as i32;
    let base = if code > 100 { code - 100 } else { code };
    if !(1..=11).contains(&base) {
        return Err(ErrorCode::Value);
    }
    let values = subtotal_values(ctx, &args[1..], code > 100, false, true)?;
    Ok(Value::number(aggregate(base, &values, None)?))
}
fn fn_aggregate(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let code = int_of(ctx, &args[0])? as i32;
    let options = int_of(ctx, args.get(1).unwrap_or(&Value::blank()))? as i32;
    if !(1..=19).contains(&code) || !(0..=7).contains(&options) {
        return Err(ErrorCode::Value);
    }
    let skip_nested = options <= 3;
    let skip_hidden = matches!(options, 1 | 3 | 5 | 7);
    let skip_errors = matches!(options, 2 | 3 | 6 | 7);
    if code >= 14 {
        if args.len() != 4 {
            return Err(ErrorCode::Value);
        }
        let values = subtotal_values(
            ctx,
            std::slice::from_ref(args.get(2).unwrap_or(&Value::blank())),
            skip_hidden,
            skip_errors,
            skip_nested,
        )?;
        let k = args.get(3).cloned().unwrap_or_else(Value::blank);
        let one = |kv: &Scalar| -> Scalar {
            if let Scalar::Error(code) = kv {
                return Scalar::Error(*code);
            }
            match super::coerce::to_number(kv, ctx.date1904)
                .and_then(|n| aggregate(code, &values, Some(n)))
            {
                Ok(n) => Scalar::Number(n),
                Err(code) => Scalar::Error(code),
            }
        };
        if let Value::Ref(reference) = &k {
            if reference.is_cell() {
                return Ok(Value::Scalar(one(&ctx.to_scalar(&k))));
            }
        }
        if matches!(k, Value::Ref(_) | Value::Matrix(_)) {
            return Ok(Value::Matrix(
                ctx.to_matrix(&k)?.map(|cell, _, _| one(cell))?,
            ));
        }
        return Ok(Value::number(aggregate(
            code,
            &values,
            Some(super::coerce::to_number(&ctx.to_scalar(&k), ctx.date1904)?),
        )?));
    }
    if args[2..].iter().any(|arg| !matches!(arg, Value::Ref(_))) {
        return Err(ErrorCode::Value);
    }
    let values = subtotal_values(ctx, &args[2..], skip_hidden, skip_errors, skip_nested)?;
    Ok(Value::number(aggregate(code, &values, None)?))
}

const ROMAN: [(i32, &str); 13] = [
    (1000, "M"),
    (900, "CM"),
    (500, "D"),
    (400, "CD"),
    (100, "C"),
    (90, "XC"),
    (50, "L"),
    (40, "XL"),
    (10, "X"),
    (9, "IX"),
    (5, "V"),
    (4, "IV"),
    (1, "I"),
];

fn fn_roman(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let _ = args.get(1);
    let mut n = num(ctx, &args[0])?.trunc() as i32;
    if !(0..=3999).contains(&n) {
        return Err(ErrorCode::Value);
    }
    let mut out = String::new();
    for (value, letters) in ROMAN {
        while n >= value {
            out.push_str(letters);
            n -= value;
        }
    }
    Ok(Value::Scalar(Scalar::Text(out)))
}
fn fn_arabic(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let text = str_of(ctx, &args[0])?.trim().to_uppercase();
    let negative = text.starts_with('-');
    let body = if negative { &text[1..] } else { text.as_str() };
    if !body
        .chars()
        .all(|ch| matches!(ch, 'M' | 'D' | 'C' | 'L' | 'X' | 'V' | 'I'))
    {
        return Err(ErrorCode::Value);
    }
    let value_of = |ch: char| match ch {
        'M' => 1000,
        'D' => 500,
        'C' => 100,
        'L' => 50,
        'X' => 10,
        'V' => 5,
        'I' => 1,
        _ => 0,
    };
    let chars: Vec<char> = body.chars().collect();
    let mut total = 0i32;
    for i in 0..chars.len() {
        let cur = value_of(chars[i]);
        let next = chars.get(i + 1).copied().map(value_of).unwrap_or(0);
        total += if cur < next { -cur } else { cur };
    }
    Ok(Value::number(if negative { -total } else { total } as f64))
}
fn fn_base(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let n = num(ctx, &args[0])?.trunc();
    let radix = num(ctx, args.get(1).unwrap_or(&Value::blank()))?.trunc();
    if n < 0.0 || n >= 2f64.powi(53) || !(2.0..=36.0).contains(&radix) {
        return Err(ErrorCode::Num);
    }
    let width =
        if args.len() > 2 && !matches!(args.get(2), Some(Value::Scalar(Scalar::Blank)) | None) {
            int_of(ctx, &args[2])?.trunc().max(0.0) as usize
        } else {
            0
        };
    let digits = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut body = String::new();
    let mut left = n as u64;
    let rad = radix as u64;
    if left == 0 {
        body.push('0');
    }
    while left > 0 {
        body.insert(0, digits.as_bytes()[(left % rad) as usize] as char);
        left /= rad;
    }
    if body.len() < width {
        body = format!("{}{body}", "0".repeat(width - body.len()));
    }
    Ok(Value::Scalar(Scalar::Text(body)))
}
fn fn_decimal(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let text = str_of(ctx, &args[0])?.trim().to_lowercase();
    let radix = num(ctx, args.get(1).unwrap_or(&Value::blank()))?.trunc();
    if !(2.0..=36.0).contains(&radix) {
        return Err(ErrorCode::Num);
    }
    let mut total = 0.0;
    for ch in text.chars() {
        let d = ch.to_digit(36).ok_or(ErrorCode::Num)? as f64;
        if d >= radix {
            return Err(ErrorCode::Num);
        }
        total = total * radix + d;
    }
    Ok(Value::number(total))
}

fn n(
    name: &'static str,
    syntax: &'static str,
    description: &'static str,
    min: usize,
    max: usize,
    f: fn(&[f64]) -> Fx<f64>,
) -> FunctionSpec {
    num_spec(name, C, syntax, description, min, max, &[], f)
}

pub(crate) fn specs() -> Vec<FunctionSpec> {
    vec![
        n(
            "SIN",
            "SIN(number)",
            "The sine of an angle in radians.",
            1,
            1,
            fn_sin,
        ),
        n(
            "COS",
            "COS(number)",
            "The cosine of an angle in radians.",
            1,
            1,
            fn_cos,
        ),
        n(
            "TAN",
            "TAN(number)",
            "The tangent of an angle in radians.",
            1,
            1,
            fn_tan,
        ),
        n(
            "ASIN",
            "ASIN(number)",
            "The arcsine in radians.",
            1,
            1,
            fn_asin,
        ),
        n(
            "ACOS",
            "ACOS(number)",
            "The arccosine in radians.",
            1,
            1,
            fn_acos,
        ),
        n(
            "ATAN",
            "ATAN(number)",
            "The arctangent in radians.",
            1,
            1,
            fn_atan,
        ),
        n(
            "ATAN2",
            "ATAN2(x_num, y_num)",
            "The arctangent of x and y coordinates.",
            2,
            2,
            fn_atan2,
        ),
        n(
            "SINH",
            "SINH(number)",
            "The hyperbolic sine.",
            1,
            1,
            fn_sinh,
        ),
        n(
            "COSH",
            "COSH(number)",
            "The hyperbolic cosine.",
            1,
            1,
            fn_cosh,
        ),
        n(
            "TANH",
            "TANH(number)",
            "The hyperbolic tangent.",
            1,
            1,
            fn_tanh,
        ),
        n(
            "ASINH",
            "ASINH(number)",
            "The inverse hyperbolic sine.",
            1,
            1,
            fn_asinh,
        ),
        n(
            "ACOSH",
            "ACOSH(number)",
            "The inverse hyperbolic cosine.",
            1,
            1,
            fn_acosh,
        ),
        n(
            "ATANH",
            "ATANH(number)",
            "The inverse hyperbolic tangent.",
            1,
            1,
            fn_atanh,
        ),
        n(
            "COT",
            "COT(number)",
            "The cotangent of an angle.",
            1,
            1,
            fn_cot,
        ),
        n(
            "COTH",
            "COTH(number)",
            "The hyperbolic cotangent.",
            1,
            1,
            fn_coth,
        ),
        n(
            "CSC",
            "CSC(number)",
            "The cosecant of an angle.",
            1,
            1,
            fn_csc,
        ),
        n(
            "CSCH",
            "CSCH(number)",
            "The hyperbolic cosecant.",
            1,
            1,
            fn_csch,
        ),
        n(
            "SEC",
            "SEC(number)",
            "The secant of an angle.",
            1,
            1,
            fn_sec,
        ),
        n(
            "SECH",
            "SECH(number)",
            "The hyperbolic secant.",
            1,
            1,
            fn_sech,
        ),
        n("ACOT", "ACOT(number)", "The arccotangent.", 1, 1, fn_acot),
        n(
            "ACOTH",
            "ACOTH(number)",
            "The inverse hyperbolic cotangent.",
            1,
            1,
            fn_acoth,
        ),
        n(
            "DEGREES",
            "DEGREES(angle)",
            "Converts radians to degrees.",
            1,
            1,
            fn_degrees,
        ),
        n(
            "RADIANS",
            "RADIANS(angle)",
            "Converts degrees to radians.",
            1,
            1,
            fn_radians,
        ),
        gen_spec(
            "RAND",
            C,
            "RAND()",
            "A random number between 0 and 1.",
            0,
            0,
            &[],
            true,
            fn_rand,
        ),
        gen_spec(
            "RANDBETWEEN",
            C,
            "RANDBETWEEN(bottom, top)",
            "A random integer between two numbers.",
            2,
            2,
            &[],
            true,
            fn_randbetween,
        ),
        gen_spec(
            "SERIESSUM",
            C,
            "SERIESSUM(x, n, m, coefficients)",
            "The sum of a power series.",
            4,
            4,
            SERIES_PARAMS,
            false,
            fn_series,
        ),
        gen_spec(
            "SUM",
            C,
            "SUM(number1, [number2], ...)",
            "Adds its arguments.",
            0,
            255,
            ANY,
            false,
            fn_sum,
        ),
        gen_spec(
            "PRODUCT",
            C,
            "PRODUCT(number1, [number2], ...)",
            "Multiplies its arguments.",
            1,
            255,
            ANY,
            false,
            fn_product,
        ),
        gen_spec(
            "SUMSQ",
            C,
            "SUMSQ(number1, [number2], ...)",
            "The sum of the squares of the arguments.",
            1,
            255,
            ANY,
            false,
            fn_sumsq,
        ),
        gen_spec(
            "SUMPRODUCT",
            C,
            "SUMPRODUCT(array1, [array2], ...)",
            "The sum of the products of corresponding array elements.",
            1,
            255,
            ANY,
            false,
            fn_sumproduct,
        ),
        gen_spec(
            "SUMX2MY2",
            C,
            "SUMX2MY2(array_x, array_y)",
            "Sum of the differences of squares.",
            2,
            2,
            ANY,
            false,
            fn_sumx2my2,
        ),
        gen_spec(
            "SUMX2PY2",
            C,
            "SUMX2PY2(array_x, array_y)",
            "Sum of the sums of squares.",
            2,
            2,
            ANY,
            false,
            fn_sumx2py2,
        ),
        gen_spec(
            "SUMXMY2",
            C,
            "SUMXMY2(array_x, array_y)",
            "Sum of the squares of differences.",
            2,
            2,
            ANY,
            false,
            fn_sumxmy2,
        ),
        gen_spec(
            "SUMIF",
            C,
            "SUMIF(range, criteria, [sum_range])",
            "Adds the cells that meet a criterion.",
            2,
            3,
            SUMIF_PARAMS,
            false,
            fn_sumif,
        ),
        gen_spec(
            "SUMIFS",
            C,
            "SUMIFS(sum_range, criteria_range1, criteria1, ...)",
            "Adds the cells that meet several criteria.",
            3,
            255,
            ANY,
            false,
            fn_sumifs,
        ),
        gen_spec(
            "SUBTOTAL",
            C,
            "SUBTOTAL(function_num, ref1, ...)",
            "A subtotal (1-11 include hidden rows, 101-111 ignore them).",
            2,
            255,
            VALUE_ANY,
            false,
            fn_subtotal,
        ),
        gen_spec(
            "AGGREGATE",
            C,
            "AGGREGATE(function_num, options, ref1, ...)",
            "An aggregate that can ignore hidden rows and errors.",
            3,
            255,
            &[ParamKind::Value, ParamKind::Value, ParamKind::Any],
            false,
            fn_aggregate,
        ),
        n(
            "ABS",
            "ABS(number)",
            "The absolute value of a number.",
            1,
            1,
            fn_abs,
        ),
        n(
            "SIGN",
            "SIGN(number)",
            "The sign of a number: 1, 0 or -1.",
            1,
            1,
            fn_sign,
        ),
        n(
            "INT",
            "INT(number)",
            "Rounds a number down to the nearest integer.",
            1,
            1,
            fn_int,
        ),
        n(
            "TRUNC",
            "TRUNC(number, [num_digits])",
            "Truncates a number toward zero.",
            1,
            2,
            fn_trunc,
        ),
        n(
            "ROUND",
            "ROUND(number, num_digits)",
            "Rounds a number to a number of digits.",
            2,
            2,
            fn_round,
        ),
        n(
            "ROUNDUP",
            "ROUNDUP(number, num_digits)",
            "Rounds a number away from zero.",
            2,
            2,
            fn_roundup,
        ),
        n(
            "ROUNDDOWN",
            "ROUNDDOWN(number, num_digits)",
            "Rounds a number toward zero.",
            2,
            2,
            fn_trunc,
        ),
        n(
            "MROUND",
            "MROUND(number, multiple)",
            "Rounds a number to the nearest multiple.",
            2,
            2,
            fn_mround,
        ),
        n(
            "CEILING",
            "CEILING(number, significance)",
            "Rounds a number up to a multiple of significance.",
            2,
            2,
            fn_ceiling,
        ),
        n(
            "FLOOR",
            "FLOOR(number, significance)",
            "Rounds a number down to a multiple of significance.",
            2,
            2,
            fn_floor,
        ),
        n(
            "CEILING.MATH",
            "CEILING.MATH(number, [significance], [mode])",
            "Rounds a number up to an integer or multiple.",
            1,
            3,
            fn_ceiling_math,
        ),
        n(
            "FLOOR.MATH",
            "FLOOR.MATH(number, [significance], [mode])",
            "Rounds a number down to an integer or multiple.",
            1,
            3,
            fn_floor_math,
        ),
        n(
            "CEILING.PRECISE",
            "CEILING.PRECISE(number, [significance])",
            "Rounds a number up regardless of sign.",
            1,
            2,
            fn_ceiling_precise,
        ),
        n(
            "ISO.CEILING",
            "ISO.CEILING(number, [significance])",
            "Rounds a number up regardless of sign.",
            1,
            2,
            fn_ceiling_precise,
        ),
        n(
            "FLOOR.PRECISE",
            "FLOOR.PRECISE(number, [significance])",
            "Rounds a number down regardless of sign.",
            1,
            2,
            fn_floor_precise,
        ),
        n(
            "EVEN",
            "EVEN(number)",
            "Rounds a number away from zero to an even integer.",
            1,
            1,
            fn_even,
        ),
        n(
            "ODD",
            "ODD(number)",
            "Rounds a number away from zero to an odd integer.",
            1,
            1,
            fn_odd,
        ),
        n(
            "MOD",
            "MOD(number, divisor)",
            "The remainder after division (sign of the divisor).",
            2,
            2,
            fn_mod,
        ),
        n(
            "QUOTIENT",
            "QUOTIENT(numerator, denominator)",
            "The integer part of a division.",
            2,
            2,
            fn_quotient,
        ),
        n(
            "SQRT",
            "SQRT(number)",
            "The positive square root.",
            1,
            1,
            fn_sqrt,
        ),
        n(
            "SQRTPI",
            "SQRTPI(number)",
            "The square root of number * pi.",
            1,
            1,
            fn_sqrtpi,
        ),
        n(
            "POWER",
            "POWER(number, power)",
            "A number raised to a power.",
            2,
            2,
            fn_power,
        ),
        n("EXP", "EXP(number)", "e raised to a power.", 1, 1, fn_exp),
        n("LN", "LN(number)", "The natural logarithm.", 1, 1, fn_ln),
        n(
            "LOG10",
            "LOG10(number)",
            "The base-10 logarithm.",
            1,
            1,
            fn_log10,
        ),
        n(
            "LOG",
            "LOG(number, [base])",
            "The logarithm to a base (10 by default).",
            1,
            2,
            fn_log,
        ),
        n("PI", "PI()", "The value of pi.", 0, 0, fn_pi),
        n(
            "FACT",
            "FACT(number)",
            "The factorial of a number.",
            1,
            1,
            fn_fact,
        ),
        n(
            "FACTDOUBLE",
            "FACTDOUBLE(number)",
            "The double factorial of a number.",
            1,
            1,
            fn_factdouble,
        ),
        n(
            "COMBIN",
            "COMBIN(number, number_chosen)",
            "The number of combinations.",
            2,
            2,
            fn_combin,
        ),
        n(
            "COMBINA",
            "COMBINA(number, number_chosen)",
            "Combinations with repetitions.",
            2,
            2,
            fn_combina,
        ),
        gen_spec(
            "MULTINOMIAL",
            C,
            "MULTINOMIAL(number1, ...)",
            "The multinomial of a set of numbers.",
            1,
            255,
            ANY,
            false,
            fn_multinomial,
        ),
        gen_spec(
            "GCD",
            C,
            "GCD(number1, ...)",
            "The greatest common divisor.",
            1,
            255,
            ANY,
            false,
            fn_gcd,
        ),
        gen_spec(
            "LCM",
            C,
            "LCM(number1, ...)",
            "The least common multiple.",
            1,
            255,
            ANY,
            false,
            fn_lcm,
        ),
        gen_spec(
            "MMULT",
            C,
            "MMULT(array1, array2)",
            "The matrix product of two arrays.",
            2,
            2,
            ANY,
            false,
            fn_mmult,
        ),
        gen_spec(
            "MDETERM",
            C,
            "MDETERM(array)",
            "The determinant of a square matrix.",
            1,
            1,
            ANY,
            false,
            fn_mdeterm,
        ),
        gen_spec(
            "MINVERSE",
            C,
            "MINVERSE(array)",
            "The inverse of a square matrix.",
            1,
            1,
            ANY,
            false,
            fn_minverse,
        ),
        gen_spec(
            "MUNIT",
            C,
            "MUNIT(dimension)",
            "The identity matrix.",
            1,
            1,
            &[],
            false,
            fn_munit,
        ),
        gen_spec(
            "ROMAN",
            C,
            "ROMAN(number, [form])",
            "Converts a number to Roman numerals (classic form).",
            1,
            2,
            &[],
            false,
            fn_roman,
        ),
        gen_spec(
            "ARABIC",
            C,
            "ARABIC(text)",
            "Converts Roman numerals to a number.",
            1,
            1,
            &[],
            false,
            fn_arabic,
        ),
        gen_spec(
            "BASE",
            C,
            "BASE(number, radix, [min_length])",
            "Converts a number to text in a radix.",
            2,
            3,
            &[],
            false,
            fn_base,
        ),
        gen_spec(
            "DECIMAL",
            C,
            "DECIMAL(text, radix)",
            "Converts text in a radix to a number.",
            2,
            2,
            &[],
            false,
            fn_decimal,
        ),
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn mod_keeps_inexact_tenth() {
        // Catches: MOD(0.3, 0.1) must keep Excel's inexact remainder (0.0999…98); 15-digit dumps collapse it onto 0.1.
        let got = super::excel_mod(0.3, 0.1).unwrap();
        assert_eq!(got.to_bits(), 0.09999999999999998_f64.to_bits());
    }
}
