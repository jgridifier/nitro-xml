// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/financial.ts,
// financial-core.ts, depreciation.ts, amorlinc.ts, coupons.ts, bonds.ts and securities.ts;
// changes listed in PROVENANCE.md.
//! Financial functions from ACCRINT through YIELDMAT.
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::similar_names)]
#![allow(clippy::many_single_char_names)]

use super::coerce::finite;
use super::datetime::{add_months, days360, serial_arg, year_frac};
use super::helpers::{bool_of, collect_numbers, int_of, num, omitted};
use super::host::CallCtx;
use super::ops::js_round;
use super::spec::{gen_spec, num_spec, FunctionSpec, ParamKind};
use super::text_num::{days_in_month, is_leap_year, serial_to_ymd};
use super::value::{ErrorCode, Fx, Scalar, Value};

const C: &str = "Financial";
const VALUE_ANY: &[ParamKind] = &[ParamKind::Value, ParamKind::Any];
const IRR_PARAMS: &[ParamKind] = &[ParamKind::Any, ParamKind::Value];
const XIRR_PARAMS: &[ParamKind] = &[ParamKind::Any, ParamKind::Any, ParamKind::Value];
const MIRR_PARAMS: &[ParamKind] = &[ParamKind::Any, ParamKind::Value, ParamKind::Value];
const Z: &[Option<f64>] = &[None, None, None, Some(0.0), Some(0.0)];
const Z6: &[Option<f64>] = &[None, None, None, None, Some(0.0), Some(0.0)];
const RATE_DEF: &[Option<f64>] = &[None, None, None, Some(0.0), Some(0.0), Some(0.1)];
const DDB_DEF: &[Option<f64>] = &[None, None, None, None, Some(2.0)];
const DB_DEF: &[Option<f64>] = &[None, None, None, None, Some(12.0)];

fn is_blank(value: &Value) -> bool {
    matches!(value, Value::Scalar(Scalar::Blank))
}

/// `Math.pow`. Integer exponents use exponentiation by squaring, which is what
/// V8 does; `powf` drifts a few ulps and fails the 14-digit parity gate.
trait JsPow {
    fn js_pow(self, exp: f64) -> f64;
}

impl JsPow for f64 {
    fn js_pow(self, exp: f64) -> f64 {
        if exp.fract() == 0.0 && (i32::MIN as f64..=i32::MAX as f64).contains(&exp) {
            self.powi(exp as i32)
        } else {
            self.powf(exp)
        }
    }
}

fn ok_num(n: f64) -> Fx<Value> {
    Ok(Value::number(finite(n)?))
}

fn at(args: &[f64], index: usize, fallback: f64) -> f64 {
    args.get(index).copied().unwrap_or(fallback)
}

fn pmt(rate: f64, nper: f64, pv: f64, fv: f64, typ: f64) -> Fx<f64> {
    if nper == 0.0 {
        return Err(ErrorCode::Num);
    }
    if rate == 0.0 {
        return Ok(-(pv + fv) / nper);
    }
    let grown = (1.0 + rate).js_pow(nper);
    Ok((-rate * (fv + pv * grown)) / ((1.0 + rate * typ) * (grown - 1.0)))
}

fn fv(rate: f64, nper: f64, payment: f64, present: f64, typ: f64) -> Fx<f64> {
    if rate == 0.0 {
        return Ok(-(present + payment * nper));
    }
    let grown = (1.0 + rate).js_pow(nper);
    Ok(-(present * grown + (payment * (1.0 + rate * typ) * (grown - 1.0)) / rate))
}

fn pv(rate: f64, nper: f64, payment: f64, future: f64, typ: f64) -> Fx<f64> {
    if rate == 0.0 {
        return Ok(-(future + payment * nper));
    }
    let grown = (1.0 + rate).js_pow(nper);
    Ok(-(future + (payment * (1.0 + rate * typ) * (grown - 1.0)) / rate) / grown)
}

fn ipmt(rate: f64, per: f64, nper: f64, present: f64, future: f64, typ: f64) -> Fx<f64> {
    if per < 1.0 || per > nper {
        return Err(ErrorCode::Num);
    }
    let payment = pmt(rate, nper, present, future, typ)?;
    let interest = if per == 1.0 {
        if typ == 1.0 {
            0.0
        } else {
            -present
        }
    } else if typ == 1.0 {
        fv(rate, per - 2.0, payment, present, 1.0)? - payment
    } else {
        fv(rate, per - 1.0, payment, present, 0.0)?
    };
    Ok(interest * rate)
}

/// Newton's method, then bisection. `#NUM!` when no root is found.
fn solve(f: impl Fn(f64) -> f64, guess: f64) -> Fx<f64> {
    let mut x = guess;
    for _ in 0..100 {
        let y = f(x);
        if !y.is_finite() {
            break;
        }
        if y == 0.0 {
            return Ok(x);
        }
        let h = 1e-9_f64.max(x.abs() * 1e-9);
        let slope = (f(x + h) - f(x - h)) / (2.0 * h);
        if slope == 0.0 || !slope.is_finite() {
            break;
        }
        let next = x - y / slope;
        if (next - x).abs() <= 1e-14 * 1.0_f64.max(next.abs()) {
            return Ok(next);
        }
        x = next;
        if x <= -1.0 {
            x = -0.999_999;
        }
    }
    let mut lo = -0.999_999;
    let mut hi = 10.0;
    let mut flo = f(lo);
    if !flo.is_finite() || flo * f(hi) > 0.0 {
        return Err(ErrorCode::Num);
    }
    for _ in 0..300 {
        let mid = (lo + hi) / 2.0;
        let fm = f(mid);
        if fm.abs() < 1e-10 {
            return Ok(mid);
        }
        if fm * flo < 0.0 {
            hi = mid;
        } else {
            lo = mid;
            flo = fm;
        }
    }
    Ok((lo + hi) / 2.0)
}

fn cumulative(args: &[f64], principal: bool) -> Fx<f64> {
    let rate = at(args, 0, 0.0);
    let nper = at(args, 1, 0.0);
    let present = at(args, 2, 0.0);
    let start = at(args, 3, 0.0);
    let end = at(args, 4, 0.0);
    let typ = at(args, 5, 0.0);
    if rate <= 0.0
        || nper <= 0.0
        || present <= 0.0
        || start < 1.0
        || end < start
        || end > nper
        || (typ != 0.0 && typ != 1.0)
    {
        return Err(ErrorCode::Num);
    }
    let payment = pmt(rate, nper, present, 0.0, typ)?;
    let mut total = 0.0;
    let mut per = start.trunc();
    let last = end.trunc();
    while per <= last {
        let interest = ipmt(rate, per, nper, present, 0.0, typ)?;
        total += if principal {
            payment - interest
        } else {
            interest
        };
        per += 1.0;
    }
    Ok(total)
}

fn fn_pmt(a: &[f64]) -> Fx<f64> {
    pmt(
        a[0],
        at(a, 1, 0.0),
        at(a, 2, 0.0),
        at(a, 3, 0.0),
        at(a, 4, 0.0),
    )
}

fn fn_fv(a: &[f64]) -> Fx<f64> {
    fv(
        a[0],
        at(a, 1, 0.0),
        at(a, 2, 0.0),
        at(a, 3, 0.0),
        at(a, 4, 0.0),
    )
}

fn fn_pv(a: &[f64]) -> Fx<f64> {
    pv(
        a[0],
        at(a, 1, 0.0),
        at(a, 2, 0.0),
        at(a, 3, 0.0),
        at(a, 4, 0.0),
    )
}

fn fn_nper(a: &[f64]) -> Fx<f64> {
    let rate = a[0];
    let payment = at(a, 1, 0.0);
    let present = at(a, 2, 0.0);
    let future = at(a, 3, 0.0);
    let typ = at(a, 4, 0.0);
    if rate == 0.0 {
        return if payment == 0.0 {
            Err(ErrorCode::Num)
        } else {
            Ok(-(present + future) / payment)
        };
    }
    let num1 = payment * (1.0 + rate * typ) - future * rate;
    let den = present * rate + payment * (1.0 + rate * typ);
    if num1 / den <= 0.0 {
        return Err(ErrorCode::Num);
    }
    Ok((num1 / den).ln() / (1.0 + rate).ln())
}

fn fn_ipmt(a: &[f64]) -> Fx<f64> {
    ipmt(
        a[0],
        at(a, 1, 0.0),
        at(a, 2, 0.0),
        at(a, 3, 0.0),
        at(a, 4, 0.0),
        at(a, 5, 0.0),
    )
}

fn fn_ppmt(a: &[f64]) -> Fx<f64> {
    let rate = a[0];
    let per = at(a, 1, 0.0);
    let nper = at(a, 2, 0.0);
    let present = at(a, 3, 0.0);
    let future = at(a, 4, 0.0);
    let typ = at(a, 5, 0.0);
    Ok(pmt(rate, nper, present, future, typ)? - ipmt(rate, per, nper, present, future, typ)?)
}

fn fn_rate(a: &[f64]) -> Fx<f64> {
    let n = a[0];
    let p = at(a, 1, 0.0);
    let v = at(a, 2, 0.0);
    let f = at(a, 3, 0.0);
    let t = at(a, 4, 0.0);
    let guess = at(a, 5, 0.1);
    solve(
        |rate| {
            if rate.abs() < 1e-12 {
                return v + p * n + f;
            }
            let growth = (n * rate.ln_1p()).exp_m1();
            v * (1.0 + growth) + (p * (1.0 + rate * t) * growth) / rate + f
        },
        guess,
    )
}

fn fn_npv(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let rate = num(ctx, &args[0])?;
    let values = collect_numbers(ctx, &args[1..], false, false)?;
    let total = values.iter().enumerate().fold(0.0, |acc, (i, value)| {
        acc + value / (1.0 + rate).js_pow(i as f64 + 1.0)
    });
    ok_num(total)
}

fn fn_irr(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = collect_numbers(ctx, &args[..1], false, false)?;
    if !values.iter().any(|v| *v > 0.0) || !values.iter().any(|v| *v < 0.0) {
        return Err(ErrorCode::Num);
    }
    let guess = if args.len() > 1 {
        num(ctx, &args[1])?
    } else {
        0.1
    };
    ok_num(solve(
        |rate| {
            values.iter().enumerate().fold(0.0, |acc, (i, value)| {
                acc + value / (1.0 + rate).js_pow(i as f64)
            })
        },
        guess,
    )?)
}

fn dated_flows(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<(Vec<f64>, Vec<f64>)> {
    let values = collect_numbers(ctx, &args[..1], false, false)?;
    let dates: Vec<f64> = collect_numbers(ctx, args.get(1..2).unwrap_or(&[]), false, false)?
        .into_iter()
        .map(f64::floor)
        .collect();
    if values.len() != dates.len() || values.is_empty() {
        return Err(ErrorCode::Num);
    }
    let first = dates[0];
    if dates.iter().any(|date| *date < first) {
        return Err(ErrorCode::Num);
    }
    Ok((values, dates))
}

fn xnpv(rate: f64, values: &[f64], dates: &[f64]) -> f64 {
    let first = dates.first().copied().unwrap_or(0.0);
    values.iter().enumerate().fold(0.0, |acc, (i, value)| {
        acc + value / (1.0 + rate).js_pow((dates[i] - first) / 365.0)
    })
}

fn fn_xnpv(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let (values, dates) = dated_flows(ctx, &args[1..])?;
    ok_num(xnpv(num(ctx, &args[0])?, &values, &dates))
}

fn fn_xirr(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let (values, dates) = dated_flows(ctx, args)?;
    if !values.iter().any(|v| *v > 0.0) || !values.iter().any(|v| *v < 0.0) {
        return Err(ErrorCode::Num);
    }
    let guess = if args.len() > 2 {
        num(ctx, &args[2])?
    } else {
        0.1
    };
    ok_num(solve(|rate| xnpv(rate, &values, &dates), guess)?)
}

fn fn_mirr(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = collect_numbers(ctx, &args[..1], false, false)?;
    let finance = num(ctx, &args[1])?;
    let reinvest = num(ctx, &args[2])?;
    let n = values.len() as f64;
    let mut fv_pos = 0.0;
    let mut pv_neg = 0.0;
    for (i, value) in values.iter().enumerate() {
        let i = i as f64;
        if *value > 0.0 {
            fv_pos += value * (1.0 + reinvest).js_pow(n - 1.0 - i);
        } else if *value < 0.0 {
            pv_neg += value / (1.0 + finance).js_pow(i);
        }
    }
    if fv_pos == 0.0 || pv_neg == 0.0 {
        return Err(ErrorCode::Div0);
    }
    ok_num((-fv_pos / pv_neg).js_pow(1.0 / (n - 1.0)) - 1.0)
}

fn fn_sln(a: &[f64]) -> Fx<f64> {
    let life = a[2];
    if life == 0.0 {
        Err(ErrorCode::Div0)
    } else {
        Ok((a[0] - a[1]) / life)
    }
}

fn fn_syd(a: &[f64]) -> Fx<f64> {
    let (cost, salvage, life, per) = (a[0], a[1], a[2], a[3]);
    if life <= 0.0 || per <= 0.0 || per > life {
        return Err(ErrorCode::Num);
    }
    Ok(((cost - salvage) * (life - per + 1.0) * 2.0) / (life * (life + 1.0)))
}

fn fn_ddb(a: &[f64]) -> Fx<f64> {
    let (cost, salvage, life, per) = (a[0], a[1], a[2], a[3]);
    let factor = at(a, 4, 2.0);
    if cost < 0.0 || salvage < 0.0 || life <= 0.0 || per <= 0.0 || per > life || factor <= 0.0 {
        return Err(ErrorCode::Num);
    }
    let rate = (factor / life).min(1.0);
    let book = cost * (1.0 - rate).js_pow((per - 1.0).max(0.0));
    Ok((book * rate).max(0.0).min(book - salvage))
}

fn variable_declining(
    cost: f64,
    salvage: f64,
    life: f64,
    start: f64,
    end: f64,
    factor: f64,
    no_switch: bool,
) -> f64 {
    let mut book = cost;
    let mut total = 0.0;
    let mut period = 1.0;
    while period - 1.0 < end {
        let declining = (book * factor) / life;
        let straight = (book - salvage) / (life - (period - 1.0)).max(1.0);
        let wanted = if no_switch {
            declining
        } else {
            declining.max(straight)
        };
        let dep = wanted.max(0.0).min(book - salvage);
        let covered = end.min(period) - start.max(period - 1.0);
        if covered > 0.0 {
            total += dep * covered.min(1.0);
        }
        book -= dep;
        period += 1.0;
    }
    total
}

fn fn_vdb(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let cost = num(ctx, &args[0])?;
    let salvage = num(ctx, &args[1])?;
    let life = num(ctx, &args[2])?;
    let start = num(ctx, &args[3])?;
    let end = num(ctx, &args[4])?;
    let factor = if args.len() > 5 && !is_blank(&args[5]) {
        num(ctx, &args[5])?
    } else {
        2.0
    };
    let no_switch = args.len() > 6 && !is_blank(&args[6]) && bool_of(&args[6])?;
    if cost < 0.0
        || salvage < 0.0
        || life <= 0.0
        || start < 0.0
        || end < start
        || end > life
        || factor <= 0.0
    {
        return Err(ErrorCode::Num);
    }
    ok_num(variable_declining(
        cost, salvage, life, start, end, factor, no_switch,
    ))
}

fn fn_db(a: &[f64]) -> Fx<f64> {
    let (cost, salvage, life, per) = (a[0], a[1], a[2], a[3]);
    let month = at(a, 4, 12.0).trunc();
    let period = per.trunc();
    if cost < 0.0
        || salvage < 0.0
        || life <= 0.0
        || per <= 0.0
        || !(1.0..=12.0).contains(&month)
        || period > (if month < 12.0 { life + 1.0 } else { life })
    {
        return Err(ErrorCode::Num);
    }
    if cost == 0.0 {
        return Ok(0.0);
    }
    let rate = js_round((1.0 - (salvage / cost).js_pow(1.0 / life)) * 1000.0) / 1000.0;
    let first = (cost * rate * month) / 12.0;
    if period <= 1.0 {
        return Ok(first);
    }
    let book = (cost - first) * (1.0 - rate).js_pow(period - 2.0);
    if period == life.trunc() + 1.0 {
        Ok((book * rate * (12.0 - month)) / 12.0)
    } else {
        Ok(book * rate)
    }
}

fn fn_effect(a: &[f64]) -> Fx<f64> {
    let periods = a[1].trunc();
    if a[0] <= 0.0 || periods < 1.0 {
        return Err(ErrorCode::Num);
    }
    Ok((1.0 + a[0] / periods).js_pow(periods) - 1.0)
}

fn fn_nominal(a: &[f64]) -> Fx<f64> {
    let periods = a[1].trunc();
    if a[0] <= 0.0 || periods < 1.0 {
        return Err(ErrorCode::Num);
    }
    Ok(periods * ((1.0 + a[0]).js_pow(1.0 / periods) - 1.0))
}

fn fn_cumipmt(a: &[f64]) -> Fx<f64> {
    cumulative(a, false)
}

fn fn_cumprinc(a: &[f64]) -> Fx<f64> {
    cumulative(a, true)
}

fn fn_ispmt(a: &[f64]) -> Fx<f64> {
    if a[2] == 0.0 {
        Err(ErrorCode::Div0)
    } else {
        Ok(a[3] * a[0] * (a[1] / a[2] - 1.0))
    }
}

fn fn_pduration(a: &[f64]) -> Fx<f64> {
    if a[0] <= 0.0 || a[1] <= 0.0 || a[2] <= 0.0 {
        return Err(ErrorCode::Num);
    }
    Ok((a[2].ln() - a[1].ln()) / (1.0 + a[0]).ln())
}

fn fn_rri(a: &[f64]) -> Fx<f64> {
    if a[0] <= 0.0 || a[1] == 0.0 {
        return Err(ErrorCode::Num);
    }
    Ok((a[2] / a[1]).js_pow(1.0 / a[0]) - 1.0)
}

fn fn_fvschedule(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let rates = collect_numbers(ctx, &args[1..], false, false)?;
    ok_num(
        rates
            .iter()
            .fold(num(ctx, &args[0])?, |acc, rate| acc * (1.0 + rate)),
    )
}

fn dollar_digits(fraction: f64) -> Fx<f64> {
    let frac = fraction.trunc();
    if frac < 0.0 {
        return Err(ErrorCode::Num);
    }
    if frac == 0.0 {
        return Err(ErrorCode::Div0);
    }
    Ok(frac.log10().ceil())
}

fn fn_dollarde(a: &[f64]) -> Fx<f64> {
    let digits = dollar_digits(a[1])?;
    let whole = a[0].trunc();
    Ok(whole + ((a[0] - whole) * 10f64.js_pow(digits)) / a[1].trunc())
}

fn fn_dollarfr(a: &[f64]) -> Fx<f64> {
    let digits = dollar_digits(a[1])?;
    let whole = a[0].trunc();
    Ok(whole + ((a[0] - whole) * a[1].trunc()) / 10f64.js_pow(digits))
}

fn fn_amorlinc(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let cost = num(ctx, &args[0])?;
    let salvage = num(ctx, &args[3])?;
    let period = num(ctx, &args[4])?;
    let rate = num(ctx, &args[5])?;
    let raw_basis = if args.len() > 6 {
        num(ctx, &args[6])?
    } else {
        0.0
    };
    let basis = raw_basis.trunc() as i32;
    let mut purchase = serial_arg(ctx, &args[1])?.floor();
    let mut first_period = serial_arg(ctx, &args[2])?.floor();
    if cost <= 0.0
        || salvage < 0.0
        || salvage > cost
        || rate <= 0.0
        || period < 0.0
        || raw_basis < 0.0
        || !matches!(basis, 0 | 1 | 3 | 4)
        || purchase > first_period
    {
        return Err(ErrorCode::Num);
    }
    let annual = finite(cost * rate)?;
    if basis == 1 || basis == 3 {
        let normalize = |serial: f64| {
            let date = serial_to_ymd(serial, ctx.date1904);
            if date.month == 2 && date.day == 29 {
                serial - 1.0
            } else {
                serial
            }
        };
        purchase = normalize(purchase);
        first_period = normalize(first_period);
    }
    let fraction = if purchase == first_period {
        1.0
    } else if basis == 1 {
        let year = serial_to_ymd(purchase, ctx.date1904).year;
        (first_period - purchase) / if is_leap_year(year) { 366.0 } else { 365.0 }
    } else {
        year_frac(purchase, first_period, f64::from(basis), ctx.date1904)?
    };
    let first = (cost - salvage).min(annual * fraction);
    let index = period.trunc();
    if index == 0.0 {
        return ok_num(first);
    }
    ok_num((annual.min(cost - salvage - first - (index - 1.0) * annual)).max(0.0))
}

struct CouponFacts {
    previous: f64,
    next: f64,
    count: f64,
    days_since: f64,
    days_in: f64,
    days_to_next: f64,
}

fn check_security(settlement: f64, maturity: f64, frequency: f64, basis: i32) -> Fx<()> {
    if !matches!(frequency, 1.0 | 2.0 | 4.0) || !(0..=4).contains(&basis) || settlement >= maturity
    {
        Err(ErrorCode::Num)
    } else {
        Ok(())
    }
}

fn coupon_facts(
    settlement: f64,
    maturity: f64,
    frequency: f64,
    basis: i32,
    date1904: bool,
) -> Fx<CouponFacts> {
    let s = settlement.floor();
    let m = maturity.floor();
    check_security(s, m, frequency, basis)?;
    let months = 12.0 / frequency;
    let date = serial_to_ymd(m, date1904);
    let month_end = date.day == days_in_month(date.year, date.month);
    let mut count = 0.0;
    let mut next = m;
    let mut previous = add_months(m, -months, date1904, month_end)?;
    let mut guard = 0;
    while previous > s {
        guard += 1;
        if guard > 1_000_000 {
            return Err(ErrorCode::Num);
        }
        count += 1.0;
        next = previous;
        previous = add_months(m, -months * (count + 1.0), date1904, month_end)?;
    }
    count += 1.0;
    let day_count = |a: f64, b: f64, european: bool| -> f64 {
        if basis == 0 || basis == 4 {
            days360(a, b, basis == 4 || european, date1904)
        } else {
            b - a
        }
    };
    let days_since = day_count(previous, s, false);
    let days_in = if basis == 1 {
        next - previous
    } else if basis == 3 {
        365.0 / frequency
    } else {
        360.0 / frequency
    };
    let days_to_next = if basis == 0 {
        days_in - days_since
    } else if basis == 4 {
        days360(s, next, true, date1904)
    } else {
        next - s
    };
    Ok(CouponFacts {
        previous,
        next,
        count,
        days_since,
        days_in,
        days_to_next,
    })
}

fn basis_of(ctx: &CallCtx<'_>, args: &[Value], index: usize) -> Fx<i32> {
    let basis = if index < args.len() && !omitted(args, index) {
        int_of(ctx, &args[index])?
    } else {
        0.0
    };
    let basis = basis as i32;
    if !(0..=4).contains(&basis) {
        Err(ErrorCode::Num)
    } else {
        Ok(basis)
    }
}

fn coupon_of(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<CouponFacts> {
    coupon_facts(
        serial_arg(ctx, &args[0])?,
        serial_arg(ctx, &args[1])?,
        int_of(ctx, &args[2])?,
        basis_of(ctx, args, 3)?,
        ctx.date1904,
    )
}

fn fn_couppcd(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    ok_num(coupon_of(ctx, args)?.previous)
}
fn fn_coupncd(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    ok_num(coupon_of(ctx, args)?.next)
}
fn fn_coupnum(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    ok_num(coupon_of(ctx, args)?.count)
}
fn fn_coupdaybs(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    ok_num(coupon_of(ctx, args)?.days_since)
}
fn fn_coupdays(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    ok_num(coupon_of(ctx, args)?.days_in)
}
fn fn_coupdaysnc(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    ok_num(coupon_of(ctx, args)?.days_to_next)
}

fn price_per_hundred(
    facts: &CouponFacts,
    rate: f64,
    yld: f64,
    redemption: f64,
    frequency: f64,
) -> f64 {
    let n = facts.count;
    let coupon = (100.0 * rate) / frequency;
    let accrued = coupon * (facts.days_since / facts.days_in);
    if n == 1.0 {
        return (redemption + coupon)
            / (1.0 + ((facts.days_to_next / facts.days_in) * yld) / frequency)
            - accrued;
    }
    let frac = facts.days_to_next / facts.days_in;
    let mut price = redemption / (1.0 + yld / frequency).js_pow(n - 1.0 + frac);
    let mut k = 1.0;
    while k <= n {
        price += coupon / (1.0 + yld / frequency).js_pow(k - 1.0 + frac);
        k += 1.0;
    }
    price - accrued
}

struct Bond {
    facts: CouponFacts,
    rate: f64,
    redemption: f64,
    frequency: f64,
}

fn bond_of(ctx: &CallCtx<'_>, args: &[Value], redemption_at: usize, basis_at: usize) -> Fx<Bond> {
    let frequency = int_of(ctx, &args[5])?;
    let facts = coupon_facts(
        serial_arg(ctx, &args[0])?,
        serial_arg(ctx, &args[1])?,
        frequency,
        basis_of(ctx, args, basis_at)?,
        ctx.date1904,
    )?;
    Ok(Bond {
        facts,
        rate: num(ctx, &args[2])?,
        redemption: num(ctx, &args[redemption_at])?,
        frequency,
    })
}

fn macaulay(bond: &Bond, yld: f64) -> f64 {
    let coupon = (100.0 * bond.rate) / bond.frequency;
    let frac = bond.facts.days_to_next / bond.facts.days_in;
    let mut weighted = 0.0;
    let mut total = 0.0;
    let mut k = 1.0;
    while k <= bond.facts.count {
        let periods = k - 1.0 + frac;
        let flow = if k == bond.facts.count {
            coupon + 100.0
        } else {
            coupon
        };
        let value = flow / (1.0 + yld / bond.frequency).js_pow(periods);
        weighted += (periods / bond.frequency) * value;
        total += value;
        k += 1.0;
    }
    weighted / total
}

fn fn_price(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let bond = bond_of(ctx, args, 4, 6)?;
    let yld = num(ctx, &args[3])?;
    if bond.rate < 0.0 || yld < 0.0 || bond.redemption <= 0.0 {
        return Err(ErrorCode::Num);
    }
    ok_num(price_per_hundred(
        &bond.facts,
        bond.rate,
        yld,
        bond.redemption,
        bond.frequency,
    ))
}

fn fn_yield(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let bond = bond_of(ctx, args, 4, 6)?;
    let price = num(ctx, &args[3])?;
    if bond.rate < 0.0 || price <= 0.0 || bond.redemption <= 0.0 {
        return Err(ErrorCode::Num);
    }
    let guess = if bond.rate == 0.0 { 0.05 } else { bond.rate };
    ok_num(solve(
        |y| price_per_hundred(&bond.facts, bond.rate, y, bond.redemption, bond.frequency) - price,
        guess,
    )?)
}

fn duration_of(ctx: &CallCtx<'_>, args: &[Value], modified: bool) -> Fx<Value> {
    let frequency = int_of(ctx, &args[4])?;
    let facts = coupon_facts(
        serial_arg(ctx, &args[0])?,
        serial_arg(ctx, &args[1])?,
        frequency,
        basis_of(ctx, args, 5)?,
        ctx.date1904,
    )?;
    let rate = num(ctx, &args[2])?;
    let yld = num(ctx, &args[3])?;
    if rate < 0.0 || yld < 0.0 {
        return Err(ErrorCode::Num);
    }
    let bond = Bond {
        facts,
        rate,
        redemption: 100.0,
        frequency,
    };
    let macaulay = macaulay(&bond, yld);
    ok_num(if modified {
        macaulay / (1.0 + yld / frequency)
    } else {
        macaulay
    })
}

fn fn_duration(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    duration_of(ctx, args, false)
}

fn fn_mduration(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    duration_of(ctx, args, true)
}

fn quasi_periods(anchor: f64, from: f64, to: f64, frequency: f64, date1904: bool) -> Fx<Vec<f64>> {
    let months = 12.0 / frequency;
    let date = serial_to_ymd(anchor, date1904);
    let month_end = date.day == days_in_month(date.year, date.month);
    let at = |k: f64| add_months(anchor, k * months, date1904, month_end);
    let mut back = 0.0;
    let mut guard = 0;
    while at(back)? > from {
        guard += 1;
        if guard > 1_000_000 {
            return Err(ErrorCode::Num);
        }
        back -= 1.0;
    }
    let mut dates = Vec::new();
    let mut k = back;
    loop {
        let date = at(k)?;
        dates.push(date);
        if date >= to || dates.len() > 1_000_000 {
            break;
        }
        k += 1.0;
    }
    Ok(dates)
}

fn fn_accrint(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let issue = serial_arg(ctx, &args[0])?;
    let first = serial_arg(ctx, &args[1])?;
    let settlement = serial_arg(ctx, &args[2])?;
    let rate = num(ctx, &args[3])?;
    let par = if args.len() > 4 {
        num(ctx, &args[4])?
    } else {
        1000.0
    };
    let frequency = int_of(ctx, &args[5])?;
    let basis = basis_of(ctx, args, 6)?;
    let from_issue = if args.len() > 7 && !is_blank(&args[7]) {
        bool_of(&args[7])?
    } else {
        true
    };
    if rate <= 0.0 || par <= 0.0 || !matches!(frequency, 1.0 | 2.0 | 4.0) || issue >= settlement {
        return Err(ErrorCode::Num);
    }
    let start = if from_issue { issue } else { issue.max(first) };
    if start >= settlement {
        return ok_num(0.0);
    }
    let dates = quasi_periods(first, start, settlement, frequency, ctx.date1904)?;
    let count = |a: f64, b: f64| {
        if basis == 0 || basis == 4 {
            days360(a, b, basis == 4, ctx.date1904)
        } else {
            b - a
        }
    };
    let mut sum = 0.0;
    for window in dates.windows(2) {
        let (p, q) = (window[0], window[1]);
        let accrued = count(start.max(p), settlement.min(q));
        if accrued <= 0.0 {
            continue;
        }
        let length = if basis == 1 {
            q - p
        } else if basis == 3 {
            365.0 / frequency
        } else {
            360.0 / frequency
        };
        sum += accrued / length;
    }
    ok_num(((par * rate) / frequency) * sum)
}

fn fn_accrintm(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let issue = serial_arg(ctx, &args[0])?;
    let settlement = serial_arg(ctx, &args[1])?;
    let rate = num(ctx, &args[2])?;
    let par = if args.len() > 3 {
        num(ctx, &args[3])?
    } else {
        1000.0
    };
    if rate <= 0.0 || par <= 0.0 || issue >= settlement {
        return Err(ErrorCode::Num);
    }
    ok_num(
        par * rate
            * year_frac(
                issue,
                settlement,
                f64::from(basis_of(ctx, args, 4)?),
                ctx.date1904,
            )?,
    )
}

fn span(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<(f64, f64)> {
    let s = serial_arg(ctx, &args[0])?.floor();
    let m = serial_arg(ctx, &args[1])?.floor();
    if s >= m {
        Err(ErrorCode::Num)
    } else {
        Ok((s, m))
    }
}

fn positive(values: &[f64]) -> Fx<()> {
    if values.iter().all(|v| *v > 0.0) {
        Ok(())
    } else {
        Err(ErrorCode::Num)
    }
}

fn discount(ctx: &CallCtx<'_>, args: &[Value], f: fn(f64, f64, f64) -> f64) -> Fx<Value> {
    let (s, m) = span(ctx, args)?;
    let a = num(ctx, &args[2])?;
    let b = num(ctx, &args[3])?;
    positive(&[a, b])?;
    let fraction = year_frac(s, m, f64::from(basis_of(ctx, args, 4)?), ctx.date1904)?;
    ok_num(f(a, b, fraction))
}

fn fn_disc(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    discount(ctx, args, |pr, redemption, f| {
        (redemption - pr) / redemption / f
    })
}
fn fn_pricedisc(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    discount(ctx, args, |rate, redemption, f| {
        redemption - rate * redemption * f
    })
}
fn fn_yielddisc(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    discount(ctx, args, |pr, redemption, f| (redemption - pr) / pr / f)
}
fn fn_intrate(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    discount(ctx, args, |investment, redemption, f| {
        (redemption - investment) / investment / f
    })
}
fn fn_received(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    discount(ctx, args, |investment, rate, f| {
        investment / (1.0 - rate * f)
    })
}

fn at_maturity(
    ctx: &CallCtx<'_>,
    args: &[Value],
    f: fn(f64, f64, f64, f64, f64) -> f64,
) -> Fx<Value> {
    let (s, m) = span(ctx, args)?;
    let issue = serial_arg(ctx, &args[2])?.floor();
    let rate = num(ctx, &args[3])?;
    let other = num(ctx, &args[4])?;
    let basis = f64::from(basis_of(ctx, args, 5)?);
    if issue > s || rate < 0.0 || other < 0.0 {
        return Err(ErrorCode::Num);
    }
    ok_num(f(
        rate,
        other,
        year_frac(issue, m, basis, ctx.date1904)?,
        year_frac(issue, s, basis, ctx.date1904)?,
        year_frac(s, m, basis, ctx.date1904)?,
    ))
}

fn fn_pricemat(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    at_maturity(ctx, args, |rate, yld, dim, a, dsm| {
        (100.0 + dim * rate * 100.0) / (1.0 + dsm * yld) - a * rate * 100.0
    })
}

fn fn_yieldmat(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    at_maturity(ctx, args, |rate, price, dim, a, dsm| {
        (1.0 + dim * rate - (price / 100.0 + a * rate)) / (price / 100.0 + a * rate) / dsm
    })
}

fn bill_days(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<f64> {
    let (s, m) = span(ctx, args)?;
    let days = m - s;
    if days > 365.0 {
        Err(ErrorCode::Num)
    } else {
        Ok(days)
    }
}

fn fn_tbillprice(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let days = bill_days(ctx, args)?;
    let rate = num(ctx, &args[2])?;
    positive(&[rate])?;
    let price = 100.0 * (1.0 - (rate * days) / 360.0);
    if price > 0.0 {
        ok_num(price)
    } else {
        Err(ErrorCode::Num)
    }
}

fn fn_tbillyield(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let days = bill_days(ctx, args)?;
    let price = num(ctx, &args[2])?;
    positive(&[price])?;
    ok_num(((100.0 - price) / price) * (360.0 / days))
}

fn fn_tbilleq(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let days = bill_days(ctx, args)?;
    let rate = num(ctx, &args[2])?;
    positive(&[rate])?;
    if days <= 182.0 {
        return ok_num((365.0 * rate) / (360.0 - rate * days));
    }
    let price = 100.0 - rate * 100.0 * (days / 360.0);
    let t = days / 365.0;
    let root = (t * t - (2.0 * t - 1.0) * (1.0 - 100.0 / price)).sqrt();
    ok_num((-t + root) / (t - 0.5))
}

pub(super) fn specs() -> Vec<FunctionSpec> {
    vec![
        gen_spec("AMORLINC", C, "AMORLINC(cost, date_purchased, first_period, salvage, period, rate, [basis])", "Straight-line depreciation with a prorated initial accounting period.", 6, 7, &[], false, fn_amorlinc),
        num_spec("SLN", C, "SLN(cost, salvage, life)", "Straight-line depreciation.", 3, 3, &[], fn_sln),
        num_spec("SYD", C, "SYD(cost, salvage, life, per)", "Sum-of-years' digits depreciation.", 4, 4, &[], fn_syd),
        num_spec("DDB", C, "DDB(cost, salvage, life, period, [factor])", "Double-declining balance depreciation.", 4, 5, DDB_DEF, fn_ddb),
        gen_spec("VDB", C, "VDB(cost, salvage, life, start_period, end_period, [factor], [no_switch])", "Depreciation over any span of periods, switching to straight-line when that is larger.", 5, 7, &[], false, fn_vdb),
        num_spec("DB", C, "DB(cost, salvage, life, period, [month])", "Fixed-declining balance depreciation.", 4, 5, DB_DEF, fn_db),
        num_spec("EFFECT", C, "EFFECT(nominal_rate, npery)", "The effective annual interest rate.", 2, 2, &[], fn_effect),
        num_spec("NOMINAL", C, "NOMINAL(effect_rate, npery)", "The nominal annual interest rate.", 2, 2, &[], fn_nominal),
        num_spec("CUMIPMT", C, "CUMIPMT(rate, nper, pv, start_period, end_period, type)", "Cumulative interest paid between periods.", 6, 6, &[], fn_cumipmt),
        num_spec("CUMPRINC", C, "CUMPRINC(rate, nper, pv, start_period, end_period, type)", "Cumulative principal paid between periods.", 6, 6, &[], fn_cumprinc),
        num_spec("ISPMT", C, "ISPMT(rate, per, nper, pv)", "Interest paid in a period of a straight-line loan.", 4, 4, &[], fn_ispmt),
        num_spec("PDURATION", C, "PDURATION(rate, pv, fv)", "Periods required to reach a value.", 3, 3, &[], fn_pduration),
        num_spec("RRI", C, "RRI(nper, pv, fv)", "An equivalent interest rate for growth.", 3, 3, &[], fn_rri),
        gen_spec("FVSCHEDULE", C, "FVSCHEDULE(principal, schedule)", "Future value with a schedule of rates.", 2, 2, VALUE_ANY, false, fn_fvschedule),
        num_spec("DOLLARDE", C, "DOLLARDE(fractional_dollar, fraction)", "Converts a fractional price to a decimal.", 2, 2, &[], fn_dollarde),
        num_spec("DOLLARFR", C, "DOLLARFR(decimal_dollar, fraction)", "Converts a decimal price to a fraction.", 2, 2, &[], fn_dollarfr),
        gen_spec("COUPPCD", C, "COUPPCD(settlement, maturity, frequency, [basis])", "The coupon date before the settlement date.", 3, 4, &[], false, fn_couppcd),
        gen_spec("COUPNCD", C, "COUPNCD(settlement, maturity, frequency, [basis])", "The coupon date after the settlement date.", 3, 4, &[], false, fn_coupncd),
        gen_spec("COUPNUM", C, "COUPNUM(settlement, maturity, frequency, [basis])", "The number of coupons between settlement and maturity.", 3, 4, &[], false, fn_coupnum),
        gen_spec("COUPDAYBS", C, "COUPDAYBS(settlement, maturity, frequency, [basis])", "The days from the start of the coupon period to settlement.", 3, 4, &[], false, fn_coupdaybs),
        gen_spec("COUPDAYS", C, "COUPDAYS(settlement, maturity, frequency, [basis])", "The days in the coupon period that contains the settlement date.", 3, 4, &[], false, fn_coupdays),
        gen_spec("COUPDAYSNC", C, "COUPDAYSNC(settlement, maturity, frequency, [basis])", "The days from settlement to the next coupon date.", 3, 4, &[], false, fn_coupdaysnc),
        gen_spec("PRICE", C, "PRICE(settlement, maturity, rate, yld, redemption, frequency, [basis])", "The price per 100 face value of a security that pays periodic interest.", 6, 7, &[], false, fn_price),
        gen_spec("YIELD", C, "YIELD(settlement, maturity, rate, pr, redemption, frequency, [basis])", "The yield of a security that pays periodic interest.", 6, 7, &[], false, fn_yield),
        gen_spec("DURATION", C, "DURATION(settlement, maturity, coupon, yld, frequency, [basis])", "The Macaulay duration of a security with periodic interest.", 5, 6, &[], false, fn_duration),
        gen_spec("MDURATION", C, "MDURATION(settlement, maturity, coupon, yld, frequency, [basis])", "The modified Macaulay duration of a security.", 5, 6, &[], false, fn_mduration),
        gen_spec("ACCRINT", C, "ACCRINT(issue, first_interest, settlement, rate, par, frequency, [basis], [calc_method])", "The accrued interest of a security that pays periodic interest.", 6, 8, &[], false, fn_accrint),
        gen_spec("ACCRINTM", C, "ACCRINTM(issue, settlement, rate, [par], [basis])", "The accrued interest of a security that pays interest at maturity.", 3, 5, &[], false, fn_accrintm),
        gen_spec("DISC", C, "DISC(settlement, maturity, pr, redemption, [basis])", "The discount rate of a security.", 4, 5, &[], false, fn_disc),
        gen_spec("PRICEDISC", C, "PRICEDISC(settlement, maturity, discount, redemption, [basis])", "The price per 100 face value of a discounted security.", 4, 5, &[], false, fn_pricedisc),
        gen_spec("YIELDDISC", C, "YIELDDISC(settlement, maturity, pr, redemption, [basis])", "The annual yield of a discounted security.", 4, 5, &[], false, fn_yielddisc),
        gen_spec("INTRATE", C, "INTRATE(settlement, maturity, investment, redemption, [basis])", "The interest rate of a fully invested security.", 4, 5, &[], false, fn_intrate),
        gen_spec("RECEIVED", C, "RECEIVED(settlement, maturity, investment, discount, [basis])", "The amount received at maturity for a fully invested security.", 4, 5, &[], false, fn_received),
        gen_spec("PRICEMAT", C, "PRICEMAT(settlement, maturity, issue, rate, yld, [basis])", "The price per 100 face value of a security that pays interest at maturity.", 5, 6, &[], false, fn_pricemat),
        gen_spec("YIELDMAT", C, "YIELDMAT(settlement, maturity, issue, rate, pr, [basis])", "The annual yield of a security that pays interest at maturity.", 5, 6, &[], false, fn_yieldmat),
        gen_spec("TBILLPRICE", C, "TBILLPRICE(settlement, maturity, discount)", "The price per 100 face value of a Treasury bill.", 3, 3, &[], false, fn_tbillprice),
        gen_spec("TBILLYIELD", C, "TBILLYIELD(settlement, maturity, pr)", "The yield of a Treasury bill.", 3, 3, &[], false, fn_tbillyield),
        gen_spec("TBILLEQ", C, "TBILLEQ(settlement, maturity, discount)", "The bond-equivalent yield of a Treasury bill.", 3, 3, &[], false, fn_tbilleq),
        num_spec("PMT", C, "PMT(rate, nper, pv, [fv], [type])", "The payment for a loan.", 3, 5, Z, fn_pmt),
        num_spec("FV", C, "FV(rate, nper, pmt, [pv], [type])", "The future value of an investment.", 3, 5, Z, fn_fv),
        num_spec("PV", C, "PV(rate, nper, pmt, [fv], [type])", "The present value of an investment.", 3, 5, Z, fn_pv),
        num_spec("NPER", C, "NPER(rate, pmt, pv, [fv], [type])", "The number of periods for an investment.", 3, 5, Z, fn_nper),
        num_spec("IPMT", C, "IPMT(rate, per, nper, pv, [fv], [type])", "The interest part of a payment.", 4, 6, Z6, fn_ipmt),
        num_spec("PPMT", C, "PPMT(rate, per, nper, pv, [fv], [type])", "The principal part of a payment.", 4, 6, Z6, fn_ppmt),
        num_spec("RATE", C, "RATE(nper, pmt, pv, [fv], [type], [guess])", "The interest rate per period.", 3, 6, RATE_DEF, fn_rate),
        gen_spec("NPV", C, "NPV(rate, value1, [value2], ...)", "The net present value of cash flows.", 2, 255, VALUE_ANY, false, fn_npv),
        gen_spec("IRR", C, "IRR(values, [guess])", "The internal rate of return.", 1, 2, IRR_PARAMS, false, fn_irr),
        gen_spec("XNPV", C, "XNPV(rate, values, dates)", "The net present value of dated cash flows.", 3, 3, VALUE_ANY, false, fn_xnpv),
        gen_spec("XIRR", C, "XIRR(values, dates, [guess])", "The internal rate of return of dated cash flows.", 2, 3, XIRR_PARAMS, false, fn_xirr),
        gen_spec("MIRR", C, "MIRR(values, finance_rate, reinvest_rate)", "The modified internal rate of return.", 3, 3, MIRR_PARAMS, false, fn_mirr),
    ]
}
