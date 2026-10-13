// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/{statistical,statistical-count,
// statistical-spread,statistical-more,statistical-shape,hypothesis-tests,regression,regression-core,
// distributions,distributions-more,distributions-f}.ts; changes listed in PROVENANCE.md.
//! Statistical functions. ERF / ERF.PRECISE / ERFC / ERFC.PRECISE stay Engineering (`TODO(#75)`).
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::too_many_arguments)]

use super::criteria::{
    criteria_pairs, lift_criteria, matching_cells, matching_values, CriteriaPair,
};
use super::dist::{self, f_cdf};
use super::helpers::{collect_numbers, int_of, make_criteria, num, numbers_of, omitted, scalar_of};
use super::host::CallCtx;
use super::math::{self, combin};
use super::ops::{flag, js_is_integer};
use super::spec::{gen_spec, num_spec, FunctionSpec, ParamKind};
use super::stats_core::{self as stats, gamma, gamma_ln};
use super::text_num::parse_number_text;
use super::value::{ErrorCode, Fx, Matrix, Scalar, Value};

const C: &str = "Statistical";
const ANY: &[ParamKind] = &[ParamKind::Any];
const ANY_VALUE: &[ParamKind] = &[ParamKind::Any, ParamKind::Value];
const RANK_PARAMS: &[ParamKind] = &[ParamKind::Value, ParamKind::Any, ParamKind::Value];
const PERCENT_RANK: &[ParamKind] = &[ParamKind::Any, ParamKind::Value, ParamKind::Value];
const FORECAST_PARAMS: &[ParamKind] = &[ParamKind::Value, ParamKind::Any];
const PROB_PARAMS: &[ParamKind] = &[
    ParamKind::Any,
    ParamKind::Any,
    ParamKind::Value,
    ParamKind::Value,
];
const BETA_DEFAULTS: &[Option<f64>] = &[Some(0.0), Some(0.0), Some(0.0), Some(0.0), Some(1.0)];

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

fn g(
    name: &'static str,
    syntax: &'static str,
    description: &'static str,
    min: usize,
    max: usize,
    params: &'static [ParamKind],
    f: fn(&CallCtx<'_>, &[Value]) -> Fx<Value>,
) -> FunctionSpec {
    gen_spec(name, C, syntax, description, min, max, params, false, f)
}

fn at(a: &[f64], i: usize, fallback: f64) -> f64 {
    a.get(i).copied().unwrap_or(fallback)
}

fn degrees(values: &[f64]) -> Fx<()> {
    if values.iter().any(|value| *value < 1.0) {
        Err(ErrorCode::Num)
    } else {
        Ok(())
    }
}

fn count_values(ctx: &CallCtx<'_>, args: &[Value]) -> f64 {
    let mut n = 0.0;
    for arg in args {
        ctx.for_each(arg, &mut |value, kind| {
            if matches!(value, Scalar::Number(_)) {
                n += 1.0;
            } else if kind == super::value::VisitKind::Direct {
                if matches!(value, Scalar::Bool(_) | Scalar::Blank) {
                    n += 1.0;
                } else if let Scalar::Text(text) = value {
                    if parse_number_text(text, ctx.date1904).is_some() {
                        n += 1.0;
                    }
                }
            }
        });
    }
    n
}

fn count_a(ctx: &CallCtx<'_>, args: &[Value]) -> f64 {
    let mut n = 0.0;
    for arg in args {
        ctx.for_each(arg, &mut |value, kind| {
            if !matches!(value, Scalar::Blank) || kind == super::value::VisitKind::Direct {
                n += 1.0;
            }
        });
    }
    n
}

fn cells_in(value: &Value) -> f64 {
    match value {
        Value::Ref(reference) => reference
            .areas
            .iter()
            .map(|area| area.rows() as f64 * area.cols() as f64)
            .sum(),
        _ => 1.0,
    }
}

fn count_blank(ctx: &CallCtx<'_>, value: &Value) -> Fx<f64> {
    if !matches!(value, Value::Ref(_)) {
        let flat = ctx.to_matrix(value)?.flat()?;
        return Ok(flat
            .iter()
            .filter(|cell| match cell {
                Scalar::Blank => true,
                Scalar::Text(text) => text.is_empty(),
                _ => false,
            })
            .count() as f64);
    }
    let mut non_blank = 0.0;
    ctx.for_each(value, &mut |cell, _| {
        let empty_text = matches!(cell, Scalar::Text(text) if text.is_empty());
        if !matches!(cell, Scalar::Blank) && !empty_text {
            non_blank += 1.0;
        }
    });
    Ok(cells_in(value) - non_blank)
}

fn count_if(ctx: &CallCtx<'_>, range: &Value, criteria: &Value) -> Fx<f64> {
    if let Value::Ref(reference) = range {
        if reference.areas.len() == 1 {
            let found = matching_cells(
                ctx,
                &[CriteriaPair {
                    range: range.clone(),
                    criteria: criteria.clone(),
                }],
                None,
                false,
            )?;
            return Ok(found.values.len() as f64 + found.blank_tail);
        }
    }
    let test = make_criteria(scalar_of(criteria)?, ctx.date1904);
    if !matches!(range, Value::Ref(_)) {
        let mut n = 0.0;
        ctx.to_matrix(range)?.for_each_value(|cell| {
            if test(cell) {
                n += 1.0;
            }
        });
        return Ok(n);
    }
    let mut matched = 0.0;
    let mut stored = 0.0;
    ctx.for_each(range, &mut |cell, _| {
        stored += 1.0;
        if test(cell) {
            matched += 1.0;
        }
    });
    if test(&Scalar::Blank) {
        Ok(matched + cells_in(range) - stored)
    } else {
        Ok(matched)
    }
}

fn fn_count(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(count_values(ctx, args)))
}
fn fn_counta(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(count_a(ctx, args)))
}
fn fn_countblank(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(count_blank(
        ctx,
        args.first().unwrap_or(&Value::blank()),
    )?))
}
fn fn_countif(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(count_if(
        ctx,
        args.first().unwrap_or(&Value::blank()),
        args.get(1).unwrap_or(&Value::blank()),
    )?))
}
fn fn_countifs(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    lift_criteria(ctx, &criteria_pairs(args, 0)?, |pairs| {
        let found = matching_cells(ctx, pairs, None, true)?;
        Ok(Value::number(found.values.len() as f64 + found.blank_tail))
    })
}
fn fn_average(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(stats::mean(&collect_numbers(
        ctx, args, false, false,
    )?)?))
}

fn extreme_fn(ctx: &CallCtx<'_>, args: &[Value], max: bool, all: bool) -> Fx<Value> {
    Ok(Value::number(stats::extreme(
        &collect_numbers(ctx, args, all, false)?,
        max,
    )))
}

fn fn_averagea(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(stats::mean(&collect_numbers(
        ctx, args, true, false,
    )?)?))
}
fn fn_averageif(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let pairs = [CriteriaPair {
        range: args.first().cloned().unwrap_or_else(Value::blank),
        criteria: args.get(1).cloned().unwrap_or_else(Value::blank),
    }];
    Ok(Value::number(stats::mean(&numbers_of(&matching_values(
        ctx,
        &pairs,
        args.get(2),
        false,
    )?))?))
}
fn fn_averageifs(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let target = args.first().cloned().unwrap_or_else(Value::blank);
    lift_criteria(ctx, &criteria_pairs(args, 1)?, |pairs| {
        Ok(Value::number(stats::mean(&numbers_of(&matching_values(
            ctx,
            pairs,
            Some(&target),
            true,
        )?))?))
    })
}
fn fn_max(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    extreme_fn(ctx, args, true, false)
}
fn fn_min(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    extreme_fn(ctx, args, false, false)
}
fn fn_maxa(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    extreme_fn(ctx, args, true, true)
}
fn fn_mina(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    extreme_fn(ctx, args, false, true)
}
fn fn_maxifs(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    ifs_extreme(ctx, args, true)
}
fn fn_minifs(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    ifs_extreme(ctx, args, false)
}
fn ifs_extreme(ctx: &CallCtx<'_>, args: &[Value], max: bool) -> Fx<Value> {
    let target = args.first().cloned().unwrap_or_else(Value::blank);
    lift_criteria(ctx, &criteria_pairs(args, 1)?, |pairs| {
        Ok(Value::number(stats::extreme(
            &numbers_of(&matching_values(ctx, pairs, Some(&target), true)?),
            max,
        )))
    })
}
fn fn_median(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(stats::median(&collect_numbers(
        ctx, args, false, false,
    )?)?))
}
fn fn_mode(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(
        stats::modes(&collect_numbers(ctx, args, false, false)?)?[0],
    ))
}
fn fn_mode_mult(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let rows = stats::modes(&collect_numbers(ctx, args, false, false)?)?
        .into_iter()
        .map(|value| vec![Scalar::Number(value)])
        .collect();
    Ok(Value::Matrix(Matrix::from_rows(rows)))
}
fn fn_large(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = collect_numbers(
        ctx,
        &[args.first().cloned().unwrap_or_else(Value::blank)],
        false,
        false,
    )?;
    Ok(Value::number(stats::kth(
        &values,
        num(ctx, args.get(1).unwrap_or(&Value::blank()))?,
        true,
    )?))
}
fn fn_small(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = collect_numbers(
        ctx,
        &[args.first().cloned().unwrap_or_else(Value::blank)],
        false,
        false,
    )?;
    Ok(Value::number(stats::kth(
        &values,
        num(ctx, args.get(1).unwrap_or(&Value::blank()))?,
        false,
    )?))
}
fn rank_of(name: &'static str) -> fn(&CallCtx<'_>, &[Value]) -> Fx<Value> {
    match name {
        "RANK.AVG" => fn_rank_avg,
        _ => fn_rank_eq,
    }
}
fn fn_rank_eq(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    rank_call(ctx, args, false)
}
fn fn_rank_avg(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    rank_call(ctx, args, true)
}
fn rank_call(ctx: &CallCtx<'_>, args: &[Value], average: bool) -> Fx<Value> {
    let x = num(ctx, &args[0])?;
    let values = collect_numbers(
        ctx,
        &[args.get(1).cloned().unwrap_or_else(Value::blank)],
        false,
        false,
    )?;
    let ascending = args.len() > 2 && num(ctx, &args[2])? != 0.0;
    Ok(Value::number(stats::rank(&values, x, ascending, average)?))
}

fn stdev_call(sample: bool, all: bool) -> fn(&CallCtx<'_>, &[Value]) -> Fx<Value> {
    if sample && all {
        fn_stdeva
    } else if sample {
        fn_stdev
    } else if all {
        fn_stdevpa
    } else {
        fn_stdevp
    }
}
fn fn_stdev(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(stats::stdev(
        &collect_numbers(ctx, args, false, false)?,
        true,
    )?))
}
fn fn_stdevp(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(stats::stdev(
        &collect_numbers(ctx, args, false, false)?,
        false,
    )?))
}
fn fn_stdeva(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(stats::stdev(
        &collect_numbers(ctx, args, true, false)?,
        true,
    )?))
}
fn fn_stdevpa(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(stats::stdev(
        &collect_numbers(ctx, args, true, false)?,
        false,
    )?))
}
fn var_call(sample: bool, all: bool) -> fn(&CallCtx<'_>, &[Value]) -> Fx<Value> {
    if sample && all {
        fn_vara
    } else if sample {
        fn_var
    } else if all {
        fn_varpa
    } else {
        fn_varp
    }
}
fn fn_var(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(stats::variance(
        &collect_numbers(ctx, args, false, false)?,
        true,
    )?))
}
fn fn_varp(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(stats::variance(
        &collect_numbers(ctx, args, false, false)?,
        false,
    )?))
}
fn fn_vara(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(stats::variance(
        &collect_numbers(ctx, args, true, false)?,
        true,
    )?))
}
fn fn_varpa(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(stats::variance(
        &collect_numbers(ctx, args, true, false)?,
        false,
    )?))
}

fn percentile_call(exclusive: bool) -> fn(&CallCtx<'_>, &[Value]) -> Fx<Value> {
    if exclusive {
        fn_percentile_exc
    } else {
        fn_percentile_inc
    }
}
fn fn_percentile_inc(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = collect_numbers(
        ctx,
        &[args.first().cloned().unwrap_or_else(Value::blank)],
        false,
        false,
    )?;
    Ok(Value::number(stats::percentile_inc(
        &values,
        num(ctx, args.get(1).unwrap_or(&Value::blank()))?,
    )?))
}
fn fn_percentile_exc(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = collect_numbers(
        ctx,
        &[args.first().cloned().unwrap_or_else(Value::blank)],
        false,
        false,
    )?;
    Ok(Value::number(stats::percentile_exc(
        &values,
        num(ctx, args.get(1).unwrap_or(&Value::blank()))?,
    )?))
}
fn quartile_call(exclusive: bool) -> fn(&CallCtx<'_>, &[Value]) -> Fx<Value> {
    if exclusive {
        fn_quartile_exc
    } else {
        fn_quartile_inc
    }
}
fn fn_quartile_inc(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = collect_numbers(
        ctx,
        &[args.first().cloned().unwrap_or_else(Value::blank)],
        false,
        false,
    )?;
    Ok(Value::number(stats::quartile(
        &values,
        num(ctx, args.get(1).unwrap_or(&Value::blank()))?,
        false,
    )?))
}
fn fn_quartile_exc(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = collect_numbers(
        ctx,
        &[args.first().cloned().unwrap_or_else(Value::blank)],
        false,
        false,
    )?;
    Ok(Value::number(stats::quartile(
        &values,
        num(ctx, args.get(1).unwrap_or(&Value::blank()))?,
        true,
    )?))
}
fn percent_rank_call(exclusive: bool) -> fn(&CallCtx<'_>, &[Value]) -> Fx<Value> {
    if exclusive {
        fn_percent_rank_exc
    } else {
        fn_percent_rank_inc
    }
}
fn fn_percent_rank_inc(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    percent_rank_body(ctx, args, false)
}
fn fn_percent_rank_exc(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    percent_rank_body(ctx, args, true)
}
fn percent_rank_body(ctx: &CallCtx<'_>, args: &[Value], exclusive: bool) -> Fx<Value> {
    let values = collect_numbers(
        ctx,
        &[args.first().cloned().unwrap_or_else(Value::blank)],
        false,
        false,
    )?;
    let digits = if args.len() > 2 {
        num(ctx, &args[2])?
    } else {
        3.0
    };
    Ok(Value::number(stats::percent_rank(
        &values,
        num(ctx, args.get(1).unwrap_or(&Value::blank()))?,
        digits,
        exclusive,
    )?))
}

pub(crate) fn paired_numbers(ctx: &CallCtx<'_>, a: &Value, b: &Value) -> Fx<(Vec<f64>, Vec<f64>)> {
    let left = ctx.to_matrix(a)?.flat()?;
    let right = ctx.to_matrix(b)?.flat()?;
    if left.len() != right.len() {
        return Err(ErrorCode::Na);
    }
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for (x, y) in left.iter().zip(right.iter()) {
        if let Scalar::Error(code) = x {
            return Err(*code);
        }
        if let Scalar::Error(code) = y {
            return Err(*code);
        }
        if let (Scalar::Number(x), Scalar::Number(y)) = (x, y) {
            xs.push(*x);
            ys.push(*y);
        }
    }
    Ok((xs, ys))
}

fn pair(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<(Vec<f64>, Vec<f64>)> {
    paired_numbers(
        ctx,
        args.first().unwrap_or(&Value::blank()),
        args.get(1).unwrap_or(&Value::blank()),
    )
}
fn fn_correl(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let (xs, ys) = pair(ctx, args)?;
    Ok(Value::number(stats::correlation(&xs, &ys)?))
}
fn fn_rsq(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let (ys, xs) = pair(ctx, args)?;
    let r = stats::correlation(&xs, &ys)?;
    Ok(Value::number(r * r))
}
fn fn_covar(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let (xs, ys) = pair(ctx, args)?;
    Ok(Value::number(stats::covariance(&xs, &ys, false)?))
}
fn fn_covar_s(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let (xs, ys) = pair(ctx, args)?;
    Ok(Value::number(stats::covariance(&xs, &ys, true)?))
}
fn fn_slope(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let (ys, xs) = pair(ctx, args)?;
    Ok(Value::number(stats::linear_fit(&xs, &ys)?.slope))
}
fn fn_intercept(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let (ys, xs) = pair(ctx, args)?;
    Ok(Value::number(stats::linear_fit(&xs, &ys)?.intercept))
}
fn fn_steyx(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let (ys, xs) = pair(ctx, args)?;
    let n = xs.len();
    if n < 3 {
        return Err(ErrorCode::Div0);
    }
    let mx = stats::mean(&xs)?;
    let my = stats::mean(&ys)?;
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    for i in 0..n {
        let dx = xs[i] - mx;
        let dy = ys[i] - my;
        sxy += dx * dy;
        sxx += dx * dx;
        syy += dy * dy;
    }
    if sxx == 0.0 {
        return Err(ErrorCode::Div0);
    }
    Ok(Value::number(
        ((syy - (sxy * sxy) / sxx) / (n as f64 - 2.0)).sqrt(),
    ))
}
fn fn_forecast(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let x = num(ctx, &args[0])?;
    let (xs, ys) = paired_numbers(
        ctx,
        args.get(2).unwrap_or(&Value::blank()),
        args.get(1).unwrap_or(&Value::blank()),
    )?;
    let fit = stats::linear_fit(&xs, &ys)?;
    Ok(Value::number(fit.intercept + fit.slope * x))
}

fn moments(values: &[f64]) -> Fx<(usize, f64, f64)> {
    Ok((
        values.len(),
        stats::mean(values)?,
        stats::stdev(values, true)?,
    ))
}
fn fn_geomean(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = collect_numbers(ctx, args, false, false)?;
    if values.is_empty() || values.iter().any(|value| *value <= 0.0) {
        return Err(ErrorCode::Num);
    }
    Ok(Value::number(
        (values.iter().map(|value| value.ln()).sum::<f64>() / values.len() as f64).exp(),
    ))
}
fn fn_harmean(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = collect_numbers(ctx, args, false, false)?;
    if values.is_empty() || values.iter().any(|value| *value <= 0.0) {
        return Err(ErrorCode::Num);
    }
    Ok(Value::number(
        values.len() as f64 / values.iter().map(|value| 1.0 / value).sum::<f64>(),
    ))
}
fn fn_avedev(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = collect_numbers(ctx, args, false, false)?;
    let m = stats::mean(&values)?;
    if values.is_empty() {
        return Err(ErrorCode::Num);
    }
    Ok(Value::number(
        values.iter().map(|value| (value - m).abs()).sum::<f64>() / values.len() as f64,
    ))
}
fn fn_devsq(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = collect_numbers(ctx, args, false, false)?;
    if values.is_empty() {
        return Err(ErrorCode::Num);
    }
    Ok(Value::number(stats::devsq(&values)?))
}
fn fn_kurt(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = collect_numbers(ctx, args, false, false)?;
    let (n, m, s) = moments(&values)?;
    if n < 4 || s == 0.0 {
        return Err(ErrorCode::Div0);
    }
    let nf = n as f64;
    let sum4: f64 = values.iter().map(|value| ((value - m) / s).powi(4)).sum();
    Ok(Value::number(
        ((nf * (nf + 1.0)) / ((nf - 1.0) * (nf - 2.0) * (nf - 3.0))) * sum4
            - (3.0 * (nf - 1.0).powi(2)) / ((nf - 2.0) * (nf - 3.0)),
    ))
}
fn fn_skew(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = collect_numbers(ctx, args, false, false)?;
    let (n, m, s) = moments(&values)?;
    if n < 3 || s == 0.0 {
        return Err(ErrorCode::Div0);
    }
    let nf = n as f64;
    let sum3: f64 = values.iter().map(|value| ((value - m) / s).powi(3)).sum();
    Ok(Value::number((nf / ((nf - 1.0) * (nf - 2.0))) * sum3))
}
fn fn_skew_p(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = collect_numbers(ctx, args, false, false)?;
    let m = stats::mean(&values)?;
    let sd = stats::stdev(&values, false)?;
    if sd == 0.0 {
        return Err(ErrorCode::Div0);
    }
    Ok(Value::number(
        values
            .iter()
            .map(|value| ((value - m) / sd).powi(3))
            .sum::<f64>()
            / values.len() as f64,
    ))
}
fn fn_standardize(a: &[f64]) -> Fx<f64> {
    let s = at(a, 2, 0.0);
    if s <= 0.0 {
        return Err(ErrorCode::Num);
    }
    Ok((a[0] - at(a, 1, 0.0)) / s)
}
fn fn_trimmean(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = stats::sorted(&collect_numbers(
        ctx,
        &[args.first().cloned().unwrap_or_else(Value::blank)],
        false,
        false,
    )?);
    let percent = num(ctx, args.get(1).unwrap_or(&Value::blank()))?;
    if percent < 0.0 || percent >= 1.0 || values.is_empty() {
        return Err(ErrorCode::Num);
    }
    let k = ((values.len() as f64 * percent) / 2.0).floor() as usize;
    let end = values.len() - k;
    Ok(Value::number(stats::mean(&values[k..end])?))
}
fn fn_frequency(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let data = collect_numbers(
        ctx,
        &[args.first().cloned().unwrap_or_else(Value::blank)],
        false,
        false,
    )?;
    let bins = collect_numbers(
        ctx,
        &[args.get(1).cloned().unwrap_or_else(Value::blank)],
        false,
        false,
    )?;
    let mut order: Vec<(f64, usize)> = bins
        .iter()
        .copied()
        .enumerate()
        .map(|(i, b)| (b, i))
        .collect();
    order.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut counts = vec![0.0; bins.len() + 1];
    for value in data {
        let slot = order
            .iter()
            .find(|(bin, _)| value <= *bin)
            .map(|(_, index)| *index)
            .unwrap_or(bins.len());
        counts[slot] += 1.0;
    }
    Ok(Value::Matrix(Matrix::from_rows(
        counts
            .into_iter()
            .map(|count| vec![Scalar::Number(count)])
            .collect(),
    )))
}
fn fn_permut(a: &[f64]) -> Fx<f64> {
    let nn = a[0].trunc();
    let kk = at(a, 1, 0.0).trunc();
    if nn < 0.0 || kk < 0.0 || kk > nn {
        return Err(ErrorCode::Num);
    }
    let mut out = 1.0;
    let mut i = 0.0;
    while i < kk {
        out *= nn - i;
        if !out.is_finite() {
            break;
        }
        i += 1.0;
    }
    Ok(out)
}
fn fn_permutationa(a: &[f64]) -> Fx<f64> {
    let k = at(a, 1, 0.0);
    if a[0] < 0.0 || k < 0.0 {
        return Err(ErrorCode::Num);
    }
    Ok(a[0].trunc().powf(k.trunc()))
}
fn fn_fisher(a: &[f64]) -> Fx<f64> {
    if a[0] <= -1.0 || a[0] >= 1.0 {
        Err(ErrorCode::Num)
    } else {
        Ok(0.5 * ((1.0 + a[0]) / (1.0 - a[0])).ln())
    }
}
fn fn_fisherinv(a: &[f64]) -> Fx<f64> {
    Ok(a[0].tanh())
}
fn fn_gamma(a: &[f64]) -> Fx<f64> {
    if a[0] <= 0.0 && js_is_integer(a[0]) {
        Err(ErrorCode::Num)
    } else {
        Ok(gamma(a[0]))
    }
}
fn fn_gammaln(a: &[f64]) -> Fx<f64> {
    gamma_ln(a[0])
}

fn norm_dist_fn(a: &[f64]) -> Fx<f64> {
    dist::norm_dist(a[0], at(a, 1, 0.0), at(a, 2, 1.0), at(a, 3, 0.0))
}
fn norm_s_dist(a: &[f64]) -> Fx<f64> {
    if flag(at(a, 1, 0.0)) {
        dist::norm_cdf(a[0])
    } else {
        Ok(dist::norm_pdf(a[0]))
    }
}
fn normsdist(a: &[f64]) -> Fx<f64> {
    dist::norm_cdf(a[0])
}
fn norm_inv_fn(a: &[f64]) -> Fx<f64> {
    dist::positive(&[at(a, 2, 1.0)])?;
    Ok(at(a, 1, 0.0) + at(a, 2, 1.0) * dist::norm_inv(a[0])?)
}
fn phi(a: &[f64]) -> Fx<f64> {
    Ok(dist::norm_pdf(a[0]))
}
fn gauss(a: &[f64]) -> Fx<f64> {
    Ok(dist::norm_cdf(a[0])? - 0.5)
}
fn confidence_norm(a: &[f64]) -> Fx<f64> {
    dist::probability(a[0])?;
    dist::positive(&[at(a, 1, 1.0)])?;
    let n = at(a, 2, 1.0);
    if n < 1.0 {
        return Err(ErrorCode::Num);
    }
    Ok(dist::norm_inv(1.0 - a[0] / 2.0)? * at(a, 1, 1.0) / n.trunc().sqrt())
}
fn confidence_t(a: &[f64]) -> Fx<f64> {
    dist::probability(a[0])?;
    dist::positive(&[at(a, 1, 1.0)])?;
    let size = at(a, 2, 1.0).trunc();
    if size < 2.0 {
        return Err(ErrorCode::Div0);
    }
    let crit = dist::invert(|t| dist::t_cdf(t, size - 1.0), 1.0 - a[0] / 2.0, 0.0, 10.0)?;
    Ok(crit * at(a, 1, 1.0) / size.sqrt())
}
fn binom(a: &[f64]) -> Fx<f64> {
    dist::binom_dist(a[0], at(a, 1, 0.0), at(a, 2, 0.0), at(a, 3, 0.0))
}
fn poisson_fn(a: &[f64]) -> Fx<f64> {
    dist::poisson(a[0], at(a, 1, 0.0), at(a, 2, 0.0))
}
fn expon(a: &[f64]) -> Fx<f64> {
    if a[0] < 0.0 {
        return Err(ErrorCode::Num);
    }
    let l = at(a, 1, 1.0);
    dist::positive(&[l])?;
    if flag(at(a, 2, 0.0)) {
        Ok(1.0 - (-l * a[0]).exp())
    } else {
        Ok(l * (-l * a[0]).exp())
    }
}
fn lognorm_dist(a: &[f64]) -> Fx<f64> {
    let s = at(a, 2, 1.0);
    dist::positive(&[a[0], s])?;
    let z = (a[0].ln() - at(a, 1, 0.0)) / s;
    if flag(at(a, 3, 0.0)) {
        dist::norm_cdf(z)
    } else {
        Ok(dist::norm_pdf(z) / (a[0] * s))
    }
}
fn lognorm_inv(a: &[f64]) -> Fx<f64> {
    let s = at(a, 2, 1.0);
    dist::positive(&[s])?;
    Ok((at(a, 1, 0.0) + s * dist::norm_inv(a[0])?).exp())
}
fn weibull(a: &[f64]) -> Fx<f64> {
    if a[0] < 0.0 {
        return Err(ErrorCode::Num);
    }
    let al = at(a, 1, 1.0);
    let be = at(a, 2, 1.0);
    dist::positive(&[al, be])?;
    if flag(at(a, 3, 0.0)) {
        Ok(1.0 - (-(a[0] / be).powf(al)).exp())
    } else {
        Ok((al / be.powf(al)) * a[0].powf(al - 1.0) * (-(a[0] / be).powf(al)).exp())
    }
}
fn gamma_dist_fn(a: &[f64]) -> Fx<f64> {
    if a[0] < 0.0 {
        return Err(ErrorCode::Num);
    }
    let al = at(a, 1, 1.0);
    let be = at(a, 2, 1.0);
    dist::positive(&[al, be])?;
    if flag(at(a, 3, 0.0)) {
        dist::gamma_p(al, a[0] / be)
    } else {
        Ok(((al - 1.0) * a[0].ln() - a[0] / be - gamma_ln(al)? - al * be.ln()).exp())
    }
}
fn t_dist(a: &[f64]) -> Fx<f64> {
    let df = at(a, 1, 0.0);
    if df < 1.0 {
        return Err(ErrorCode::Num);
    }
    let d = df.trunc();
    if flag(at(a, 2, 0.0)) {
        dist::t_cdf(a[0], d)
    } else {
        dist::t_pdf(a[0], d)
    }
}
fn t_dist_rt(a: &[f64]) -> Fx<f64> {
    let df = at(a, 1, 0.0);
    if df < 1.0 {
        return Err(ErrorCode::Num);
    }
    Ok(1.0 - dist::t_cdf(a[0], df.trunc())?)
}
fn t_dist_2t(a: &[f64]) -> Fx<f64> {
    let df = at(a, 1, 0.0);
    if a[0] < 0.0 || df < 1.0 {
        return Err(ErrorCode::Num);
    }
    Ok(2.0 * (1.0 - dist::t_cdf(a[0], df.trunc())?))
}
fn tdist(a: &[f64]) -> Fx<f64> {
    let df = at(a, 1, 0.0);
    let tails = at(a, 2, 1.0);
    if a[0] < 0.0 || df < 1.0 || (tails != 1.0 && tails != 2.0) {
        return Err(ErrorCode::Num);
    }
    Ok(tails * (1.0 - dist::t_cdf(a[0], df.trunc())?))
}
fn t_inv(a: &[f64]) -> Fx<f64> {
    dist::probability(a[0])?;
    let df = at(a, 1, 0.0);
    if df < 1.0 {
        return Err(ErrorCode::Num);
    }
    let d = df.trunc();
    if a[0] < 0.5 {
        Ok(-dist::invert(|t| dist::t_cdf(t, d), 1.0 - a[0], 0.0, 10.0)?)
    } else {
        dist::invert(|t| dist::t_cdf(t, d), a[0], 0.0, 10.0)
    }
}
fn t_inv_2t(a: &[f64]) -> Fx<f64> {
    let df = at(a, 1, 0.0);
    if a[0] <= 0.0 || a[0] > 1.0 || df < 1.0 {
        return Err(ErrorCode::Num);
    }
    dist::invert(|t| dist::t_cdf(t, df.trunc()), 1.0 - a[0] / 2.0, 0.0, 10.0)
}
fn chisq_dist(a: &[f64]) -> Fx<f64> {
    let df = at(a, 1, 0.0);
    if a[0] < 0.0 || df < 1.0 {
        return Err(ErrorCode::Num);
    }
    let k = df.trunc();
    if flag(at(a, 2, 0.0)) {
        dist::chi_cdf(a[0], k)
    } else {
        Ok(((k / 2.0 - 1.0) * a[0].ln()
            - a[0] / 2.0
            - (k / 2.0) * std::f64::consts::LN_2
            - gamma_ln(k / 2.0)?)
        .exp())
    }
}
fn chisq_rt(a: &[f64]) -> Fx<f64> {
    let df = at(a, 1, 0.0);
    if a[0] < 0.0 || df < 1.0 {
        return Err(ErrorCode::Num);
    }
    Ok(1.0 - dist::chi_cdf(a[0], df.trunc())?)
}
fn chisq_inv(a: &[f64]) -> Fx<f64> {
    let df = at(a, 1, 0.0);
    if a[0] < 0.0 || a[0] >= 1.0 || df < 1.0 {
        return Err(ErrorCode::Num);
    }
    dist::invert(|x| dist::chi_cdf(x, df.trunc()), a[0], 0.0, 100.0)
}
fn chisq_inv_rt(a: &[f64]) -> Fx<f64> {
    let df = at(a, 1, 0.0);
    if a[0] <= 0.0 || a[0] > 1.0 || df < 1.0 {
        return Err(ErrorCode::Num);
    }
    dist::invert(|x| dist::chi_cdf(x, df.trunc()), 1.0 - a[0], 0.0, 100.0)
}
fn beta_dist(a: &[f64]) -> Fx<f64> {
    let lo = at(a, 4, 0.0);
    let hi = at(a, 5, 1.0);
    let al = at(a, 1, 1.0);
    let be = at(a, 2, 1.0);
    dist::positive(&[al, be])?;
    if a[0] < lo || a[0] > hi || lo == hi {
        return Err(ErrorCode::Num);
    }
    let t = (a[0] - lo) / (hi - lo);
    if flag(at(a, 3, 0.0)) {
        dist::beta_i(t, al, be)
    } else {
        let ln_b = gamma_ln(al)? + gamma_ln(be)? - gamma_ln(al + be)?;
        Ok(((al - 1.0) * t.ln() + (be - 1.0) * (1.0 - t).ln() - ln_b).exp() / (hi - lo))
    }
}
fn negbinom(a: &[f64]) -> Fx<f64> {
    let ff = a[0].trunc();
    let ss = at(a, 1, 0.0).trunc();
    let pp = at(a, 2, 0.0);
    if ff < 0.0 || ss < 1.0 || !(0.0..=1.0).contains(&pp) {
        return Err(ErrorCode::Num);
    }
    let pmf = |k: f64| -> Fx<f64> {
        Ok(combin(k + ss - 1.0, ss - 1.0)? * pp.powf(ss) * (1.0 - pp).powf(k))
    };
    if !flag(at(a, 3, 0.0)) {
        return pmf(ff);
    }
    let mut total = 0.0;
    let mut k = 0.0;
    while k <= ff {
        total += pmf(k)?;
        k += 1.0;
    }
    Ok(total)
}
fn hypgeom(a: &[f64]) -> Fx<f64> {
    let kk = a[0].trunc();
    let nn = at(a, 1, 0.0).trunc();
    let big_k = at(a, 2, 0.0).trunc();
    let big_n = at(a, 3, 0.0).trunc();
    if kk < 0.0 || nn > big_n || big_k > big_n || kk > nn.min(big_k) {
        return Err(ErrorCode::Num);
    }
    let pmf = |i: f64| -> Fx<f64> {
        Ok(combin(big_k, i)? * combin(big_n - big_k, nn - i)? / combin(big_n, nn)?)
    };
    if !flag(at(a, 4, 0.0)) {
        return pmf(kk);
    }
    let mut total = 0.0;
    let mut i = (nn - (big_n - big_k)).max(0.0);
    while i <= kk {
        total += pmf(i)?;
        i += 1.0;
    }
    Ok(total)
}
fn f_dist(a: &[f64]) -> Fx<f64> {
    degrees(&[at(a, 1, 0.0), at(a, 2, 0.0)])?;
    if a[0] < 0.0 {
        return Err(ErrorCode::Num);
    }
    let d1 = at(a, 1, 1.0).trunc();
    let d2 = at(a, 2, 1.0).trunc();
    if flag(at(a, 3, 0.0)) {
        f_cdf(a[0], d1, d2)
    } else {
        dist::f_pdf(a[0], d1, d2)
    }
}
fn f_dist_rt(a: &[f64]) -> Fx<f64> {
    degrees(&[at(a, 1, 0.0), at(a, 2, 0.0)])?;
    if a[0] < 0.0 {
        return Err(ErrorCode::Num);
    }
    Ok(1.0 - f_cdf(a[0], at(a, 1, 1.0).trunc(), at(a, 2, 1.0).trunc())?)
}
fn f_inv(a: &[f64]) -> Fx<f64> {
    degrees(&[at(a, 1, 0.0), at(a, 2, 0.0)])?;
    if a[0] < 0.0 || a[0] >= 1.0 {
        return Err(ErrorCode::Num);
    }
    dist::f_inverse(a[0], at(a, 1, 1.0).trunc(), at(a, 2, 1.0).trunc())
}
fn f_inv_rt(a: &[f64]) -> Fx<f64> {
    degrees(&[at(a, 1, 0.0), at(a, 2, 0.0)])?;
    if a[0] <= 0.0 || a[0] > 1.0 {
        return Err(ErrorCode::Num);
    }
    dist::f_inverse(1.0 - a[0], at(a, 1, 1.0).trunc(), at(a, 2, 1.0).trunc())
}
fn beta_inv(a: &[f64]) -> Fx<f64> {
    dist::beta_inverse(
        a[0],
        at(a, 1, 1.0),
        at(a, 2, 1.0),
        at(a, 3, 0.0),
        at(a, 4, 1.0),
    )
}
fn betadist(a: &[f64]) -> Fx<f64> {
    dist::positive(&[at(a, 1, 1.0), at(a, 2, 1.0)])?;
    dist::beta_scaled(
        a[0],
        at(a, 1, 1.0),
        at(a, 2, 1.0),
        at(a, 3, 0.0),
        at(a, 4, 1.0),
    )
}
fn gamma_inv(a: &[f64]) -> Fx<f64> {
    dist::gamma_inverse(a[0], at(a, 1, 1.0), at(a, 2, 1.0))
}
fn loginv(a: &[f64]) -> Fx<f64> {
    let s = at(a, 2, 1.0);
    dist::positive(&[s])?;
    if a[0] <= 0.0 || a[0] >= 1.0 {
        return Err(ErrorCode::Num);
    }
    Ok((at(a, 1, 0.0) + s * dist::norm_inv(a[0])?).exp())
}
fn lognormdist(a: &[f64]) -> Fx<f64> {
    let s = at(a, 2, 1.0);
    dist::positive(&[a[0], s])?;
    dist::norm_cdf((a[0].ln() - at(a, 1, 0.0)) / s)
}
fn binom_inv(a: &[f64]) -> Fx<f64> {
    dist::binom_inverse(a[0], at(a, 1, 0.0), at(a, 2, 0.0))
}
fn binom_range(a: &[f64]) -> Fx<f64> {
    let n = a[0].trunc();
    let p = at(a, 1, 0.0);
    let lo = at(a, 2, 0.0).trunc();
    let hi = at(a, 3, lo).trunc();
    if !(0.0..=1.0).contains(&p) || n < 0.0 || lo < 0.0 || lo > n || hi < lo || hi > n {
        return Err(ErrorCode::Num);
    }
    let mut total = 0.0;
    let mut k = lo;
    while k <= hi {
        total += dist::binom_pmf(k, n, p)?;
        k += 1.0;
    }
    Ok(total.min(1.0))
}
fn hypgeomdist(a: &[f64]) -> Fx<f64> {
    dist::hypgeom_pmf(a[0], at(a, 1, 0.0), at(a, 2, 0.0), at(a, 3, 0.0))
}
fn negbinomdist(a: &[f64]) -> Fx<f64> {
    dist::negbinom_pmf(a[0], at(a, 1, 0.0), at(a, 2, 0.0))
}

fn numbers_arg(ctx: &CallCtx<'_>, value: &Value) -> Fx<Vec<f64>> {
    collect_numbers(ctx, std::slice::from_ref(value), false, false)
}

fn t_statistic(a: &[f64], b: &[f64], kind: f64) -> Fx<(f64, f64)> {
    if kind == 1.0 {
        if a.len() != b.len() {
            return Err(ErrorCode::Na);
        }
        let diffs: Vec<f64> = a.iter().zip(b.iter()).map(|(x, y)| x - y).collect();
        let n = diffs.len();
        if n < 2 {
            return Err(ErrorCode::Div0);
        }
        let sd = stats::stdev(&diffs, true)?;
        if sd == 0.0 {
            return Err(ErrorCode::Div0);
        }
        return Ok((
            stats::mean(&diffs)? / (sd / (n as f64).sqrt()),
            n as f64 - 1.0,
        ));
    }
    if a.len() < 2 || b.len() < 2 {
        return Err(ErrorCode::Div0);
    }
    let (n1, n2) = (a.len() as f64, b.len() as f64);
    let (v1, v2) = (stats::variance(a, true)?, stats::variance(b, true)?);
    let delta = stats::mean(a)? - stats::mean(b)?;
    if kind == 2.0 {
        let pooled = ((n1 - 1.0) * v1 + (n2 - 1.0) * v2) / (n1 + n2 - 2.0);
        if pooled == 0.0 {
            return Err(ErrorCode::Div0);
        }
        return Ok((
            delta / (pooled * (1.0 / n1 + 1.0 / n2)).sqrt(),
            n1 + n2 - 2.0,
        ));
    }
    let (s1, s2) = (v1 / n1, v2 / n2);
    if s1 + s2 == 0.0 {
        return Err(ErrorCode::Div0);
    }
    let df = (s1 + s2).powi(2) / (s1.powi(2) / (n1 - 1.0) + s2.powi(2) / (n2 - 1.0));
    Ok((delta / (s1 + s2).sqrt(), df))
}

fn student_p(a: &[f64], b: &[f64], tails: f64, kind: f64) -> Fx<f64> {
    if (tails != 1.0 && tails != 2.0) || !(1.0..=3.0).contains(&kind) {
        return Err(ErrorCode::Num);
    }
    let (t, df) = t_statistic(a, b, kind)?;
    Ok(tails * (1.0 - dist::t_cdf(t.abs(), df)?))
}

fn fn_ztest(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let data = numbers_arg(ctx, args.first().unwrap_or(&Value::blank()))?;
    if data.is_empty() {
        return Err(ErrorCode::Na);
    }
    let sigma = if args.len() > 2 && !omitted(args, 2) {
        num(ctx, &args[2])?
    } else {
        stats::stdev(&data, true)?
    };
    if sigma <= 0.0 {
        return Err(ErrorCode::Num);
    }
    let z = (stats::mean(&data)? - num(ctx, args.get(1).unwrap_or(&Value::blank()))?)
        / (sigma / (data.len() as f64).sqrt());
    Ok(Value::number(1.0 - dist::norm_cdf(z)?))
}
fn fn_ttest(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let tails = int_of(ctx, args.get(2).unwrap_or(&Value::blank()))?;
    let kind = int_of(ctx, args.get(3).unwrap_or(&Value::blank()))?;
    if kind == 1.0 {
        let (xs, ys) = paired_numbers(
            ctx,
            args.first().unwrap_or(&Value::blank()),
            args.get(1).unwrap_or(&Value::blank()),
        )?;
        return Ok(Value::number(student_p(&xs, &ys, tails, kind)?));
    }
    let a = numbers_arg(ctx, args.first().unwrap_or(&Value::blank()))?;
    let b = numbers_arg(ctx, args.get(1).unwrap_or(&Value::blank()))?;
    Ok(Value::number(student_p(&a, &b, tails, kind)?))
}
fn fn_ftest(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let a = numbers_arg(ctx, args.first().unwrap_or(&Value::blank()))?;
    let b = numbers_arg(ctx, args.get(1).unwrap_or(&Value::blank()))?;
    if a.len() < 2 || b.len() < 2 {
        return Err(ErrorCode::Div0);
    }
    let (v1, v2) = (stats::variance(&a, true)?, stats::variance(&b, true)?);
    if v1 == 0.0 || v2 == 0.0 {
        return Err(ErrorCode::Div0);
    }
    let left = f_cdf(v1 / v2, a.len() as f64 - 1.0, b.len() as f64 - 1.0)?;
    Ok(Value::number((2.0 * left.min(1.0 - left)).min(1.0)))
}
fn fn_chisq_test(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let actual = ctx
        .to_matrix(args.first().unwrap_or(&Value::blank()))?
        .data()?;
    let expected = ctx
        .to_matrix(args.get(1).unwrap_or(&Value::blank()))?
        .data()?;
    let rows = actual.len();
    let cols = actual.first().map(|row| row.len()).unwrap_or(0);
    if rows != expected.len() || cols != expected.first().map(|row| row.len()).unwrap_or(0) {
        return Err(ErrorCode::Na);
    }
    let mut stat = 0.0;
    for r in 0..rows {
        for c in 0..cols {
            let x = &actual[r][c];
            let y = &expected[r][c];
            if let Scalar::Error(code) = x {
                return Err(*code);
            }
            if let Scalar::Error(code) = y {
                return Err(*code);
            }
            let Scalar::Number(x) = x else { continue };
            if matches!(y, Scalar::Blank) || matches!(y, Scalar::Number(n) if *n == 0.0) {
                return Err(ErrorCode::Div0);
            }
            let Scalar::Number(y) = y else { continue };
            stat += (x - y).powi(2) / y;
        }
    }
    let df = if rows > 1 && cols > 1 {
        (rows - 1) * (cols - 1)
    } else {
        rows * cols - 1
    };
    if df < 1 {
        return Err(ErrorCode::Div0);
    }
    Ok(Value::number(1.0 - dist::chi_cdf(stat, df as f64)?))
}
fn fn_prob(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let xs = ctx
        .to_matrix(args.first().unwrap_or(&Value::blank()))?
        .flat()?;
    let ps = ctx
        .to_matrix(args.get(1).unwrap_or(&Value::blank()))?
        .flat()?;
    if xs.len() != ps.len() {
        return Err(ErrorCode::Na);
    }
    let lower = num(ctx, args.get(2).unwrap_or(&Value::blank()))?;
    let upper = if args.len() > 3 && !omitted(args, 3) {
        num(ctx, &args[3])?
    } else {
        lower
    };
    let mut total = 0.0;
    let mut hit = 0.0;
    for (x, p) in xs.iter().zip(ps.iter()) {
        if let Scalar::Error(code) = x {
            return Err(*code);
        }
        if let Scalar::Error(code) = p {
            return Err(*code);
        }
        let Scalar::Number(p) = p else {
            return Err(ErrorCode::Num);
        };
        if *p < 0.0 || *p > 1.0 {
            return Err(ErrorCode::Num);
        }
        total += p;
        if let Scalar::Number(x) = x {
            if *x >= lower && *x <= upper {
                hit += p;
            }
        }
    }
    if (total - 1.0).abs() > 1e-9 {
        return Err(ErrorCode::Num);
    }
    Ok(Value::number(hit))
}

struct Fit {
    slopes: Vec<f64>,
    intercept: f64,
    inverse: Vec<Vec<f64>>,
    means: Vec<f64>,
    n: usize,
    has_intercept: bool,
    ss_total: f64,
    ss_resid: f64,
}

fn fit_least_squares(x: &[Vec<f64>], y: &[f64], has_intercept: bool) -> Fx<Fit> {
    let n = y.len();
    let k = x.first().map(|row| row.len()).unwrap_or(0);
    if n == 0 || k == 0 {
        return Err(ErrorCode::Value);
    }
    let means = if has_intercept {
        (0..k)
            .map(|j| {
                x.iter()
                    .map(|row| row.get(j).copied().unwrap_or(0.0))
                    .sum::<f64>()
                    / n as f64
            })
            .collect::<Vec<_>>()
    } else {
        vec![0.0; k]
    };
    let y_mean = if has_intercept {
        y.iter().sum::<f64>() / n as f64
    } else {
        0.0
    };
    let xc: Vec<Vec<f64>> = x
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(j, value)| value - means.get(j).copied().unwrap_or(0.0))
                .collect()
        })
        .collect();
    let yc: Vec<f64> = y.iter().map(|value| value - y_mean).collect();
    let gram: Vec<Vec<f64>> = (0..k)
        .map(|a| {
            (0..k)
                .map(|b| {
                    xc.iter()
                        .map(|row| {
                            row.get(a).copied().unwrap_or(0.0) * row.get(b).copied().unwrap_or(0.0)
                        })
                        .sum()
                })
                .collect()
        })
        .collect();
    let moment: Vec<f64> = (0..k)
        .map(|a| {
            xc.iter()
                .enumerate()
                .map(|(i, row)| row.get(a).copied().unwrap_or(0.0) * yc[i])
                .sum()
        })
        .collect();
    let solved = math::eliminate(&gram, true)?;
    let Some(inverse) = solved.inverse else {
        return Err(ErrorCode::Num);
    };
    if solved.det == 0.0 || !solved.det.is_finite() {
        return Err(ErrorCode::Num);
    }
    let slopes: Vec<f64> = inverse
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(j, value)| value * moment.get(j).copied().unwrap_or(0.0))
                .sum()
        })
        .collect();
    let intercept = if has_intercept {
        y_mean
            - slopes
                .iter()
                .enumerate()
                .map(|(j, slope)| slope * means.get(j).copied().unwrap_or(0.0))
                .sum::<f64>()
    } else {
        0.0
    };
    let mut ss_resid = 0.0;
    let mut ss_total = 0.0;
    for i in 0..n {
        let predicted = intercept
            + slopes
                .iter()
                .enumerate()
                .map(|(j, slope)| slope * x[i].get(j).copied().unwrap_or(0.0))
                .sum::<f64>();
        ss_resid += (y[i] - predicted).powi(2);
        ss_total += (y[i] - y_mean).powi(2);
    }
    Ok(Fit {
        slopes,
        intercept,
        inverse,
        means,
        n,
        has_intercept,
        ss_total,
        ss_resid,
    })
}

struct FitStats {
    slope_errors: Vec<f64>,
    intercept_error: Option<f64>,
    r_squared: f64,
    standard_error: f64,
    f_statistic: f64,
    df: f64,
    ss_regression: f64,
    ss_residual: f64,
}

fn fit_statistics(fit: &Fit) -> Fx<FitStats> {
    let k = fit.slopes.len();
    let df = fit.n as f64 - k as f64 - if fit.has_intercept { 1.0 } else { 0.0 };
    if df < 1.0 {
        return Err(ErrorCode::Num);
    }
    let ss_regression = fit.ss_total - fit.ss_resid;
    let variance = fit.ss_resid / df;
    let mut intercept_error = None;
    if fit.has_intercept {
        let mut quad = 0.0;
        for a in 0..k {
            for b in 0..k {
                quad += fit.means.get(a).copied().unwrap_or(0.0)
                    * fit
                        .inverse
                        .get(a)
                        .and_then(|row| row.get(b))
                        .copied()
                        .unwrap_or(0.0)
                    * fit.means.get(b).copied().unwrap_or(0.0);
            }
        }
        intercept_error = Some((variance * (1.0 / fit.n as f64 + quad)).sqrt());
    }
    Ok(FitStats {
        slope_errors: fit
            .inverse
            .iter()
            .enumerate()
            .map(|(j, row)| (variance * row.get(j).copied().unwrap_or(0.0)).sqrt())
            .collect(),
        intercept_error,
        r_squared: if fit.ss_total == 0.0 {
            1.0
        } else {
            ss_regression / fit.ss_total
        },
        standard_error: variance.sqrt(),
        f_statistic: if fit.ss_resid == 0.0 {
            f64::INFINITY
        } else {
            ss_regression / k as f64 / variance
        },
        df,
        ss_regression,
        ss_residual: fit.ss_resid,
    })
}

fn number_grid(ctx: &CallCtx<'_>, value: &Value) -> Fx<Vec<Vec<f64>>> {
    let data = ctx.to_matrix(value)?.data()?;
    let mut rows = Vec::new();
    for line in data {
        let mut nums = Vec::new();
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

struct Layout {
    y: Vec<f64>,
    x: Vec<Vec<f64>>,
    column: bool,
}

fn as_observations(matrix: &[Vec<f64>], column: bool) -> Vec<Vec<f64>> {
    if column {
        return matrix.to_vec();
    }
    let width = matrix.first().map(|row| row.len()).unwrap_or(0);
    (0..width)
        .map(|c| {
            matrix
                .iter()
                .map(|row| row.get(c).copied().unwrap_or(0.0))
                .collect()
        })
        .collect()
}

fn layout(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Layout> {
    let ym = number_grid(ctx, args.first().unwrap_or(&Value::blank()))?;
    let rows = ym.len();
    let cols = ym.first().map(|row| row.len()).unwrap_or(0);
    if rows == 0 || cols == 0 || (rows > 1 && cols > 1) {
        return Err(ErrorCode::Ref);
    }
    let column = cols == 1;
    let y: Vec<f64> = ym.iter().flatten().copied().collect();
    if omitted(args, 1) {
        return Ok(Layout {
            x: y.iter()
                .enumerate()
                .map(|(i, _)| vec![i as f64 + 1.0])
                .collect(),
            y,
            column,
        });
    }
    let xm = number_grid(ctx, args.get(1).unwrap_or(&Value::blank()))?;
    let x = as_observations(&xm, column);
    if x.len() != y.len() {
        let flat: Vec<f64> = xm.iter().flatten().copied().collect();
        if flat.len() != y.len() {
            return Err(ErrorCode::Ref);
        }
        return Ok(Layout {
            x: flat.into_iter().map(|value| vec![value]).collect(),
            y,
            column,
        });
    }
    Ok(Layout { y, x, column })
}

fn with_const(_ctx: &CallCtx<'_>, args: &[Value], index: usize) -> Fx<bool> {
    if omitted(args, index) {
        Ok(true)
    } else {
        super::helpers::bool_of(args.get(index).unwrap_or(&Value::blank()))
    }
}

fn pad_row(mut values: Vec<Scalar>, width: usize) -> Vec<Scalar> {
    while values.len() < width {
        values.push(Scalar::Error(ErrorCode::Na));
    }
    values
}

fn coefficient_table(fit: &Fit, stats_on: bool, transform: fn(f64) -> f64) -> Fx<Matrix> {
    let k = fit.slopes.len();
    let width = k + 1;
    let mut slopes: Vec<f64> = fit.slopes.iter().copied().rev().map(transform).collect();
    slopes.push(transform(fit.intercept));
    let head: Vec<Scalar> = slopes.into_iter().map(Scalar::Number).collect();
    if !stats_on {
        return Ok(Matrix::from_rows(vec![head]));
    }
    let s = fit_statistics(fit)?;
    let mut errors: Vec<Scalar> = s
        .slope_errors
        .iter()
        .copied()
        .rev()
        .map(Scalar::Number)
        .collect();
    errors.push(
        s.intercept_error
            .map(Scalar::Number)
            .unwrap_or(Scalar::Error(ErrorCode::Na)),
    );
    let f_stat = if s.f_statistic.is_infinite() {
        Scalar::Error(ErrorCode::Num)
    } else {
        Scalar::Number(s.f_statistic)
    };
    Ok(Matrix::from_rows(vec![
        head,
        pad_row(errors, width),
        pad_row(
            vec![
                Scalar::Number(s.r_squared),
                Scalar::Number(s.standard_error),
            ],
            width,
        ),
        pad_row(vec![f_stat, Scalar::Number(s.df)], width),
        pad_row(
            vec![
                Scalar::Number(s.ss_regression),
                Scalar::Number(s.ss_residual),
            ],
            width,
        ),
    ]))
}

fn predictions(
    ctx: &CallCtx<'_>,
    args: &[Value],
    lay: &Layout,
    fit: &Fit,
    transform: fn(f64) -> f64,
) -> Fx<Matrix> {
    let predict = |obs: &[f64]| {
        transform(
            fit.intercept
                + fit
                    .slopes
                    .iter()
                    .enumerate()
                    .map(|(j, slope)| slope * obs.get(j).copied().unwrap_or(0.0))
                    .sum::<f64>(),
        )
    };
    if omitted(args, 2) {
        let out: Vec<f64> = lay.x.iter().map(|obs| predict(obs)).collect();
        return Ok(if lay.column {
            Matrix::from_rows(
                out.into_iter()
                    .map(|value| vec![Scalar::Number(value)])
                    .collect(),
            )
        } else {
            Matrix::from_rows(vec![out.into_iter().map(Scalar::Number).collect()])
        });
    }
    let target = number_grid(ctx, args.get(2).unwrap_or(&Value::blank()))?;
    if fit.slopes.len() == 1 {
        let rows = target
            .into_iter()
            .map(|line| {
                line.into_iter()
                    .map(|value| Scalar::Number(predict(&[value])))
                    .collect()
            })
            .collect();
        return Ok(Matrix::from_rows(rows));
    }
    let observations = as_observations(&target, lay.column);
    if observations.first().map(|row| row.len()).unwrap_or(0) != fit.slopes.len() {
        return Err(ErrorCode::Ref);
    }
    let out: Vec<f64> = observations.iter().map(|obs| predict(obs)).collect();
    Ok(if lay.column {
        Matrix::from_rows(
            out.into_iter()
                .map(|value| vec![Scalar::Number(value)])
                .collect(),
        )
    } else {
        Matrix::from_rows(vec![out.into_iter().map(Scalar::Number).collect()])
    })
}

fn logs(lay: Layout) -> Fx<Layout> {
    let mut y = Vec::with_capacity(lay.y.len());
    for value in lay.y {
        if value > 0.0 {
            y.push(value.ln());
        } else {
            return Err(ErrorCode::Num);
        }
    }
    Ok(Layout {
        y,
        x: lay.x,
        column: lay.column,
    })
}

fn fn_linest(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let lay = layout(ctx, args)?;
    let fit = fit_least_squares(&lay.x, &lay.y, with_const(ctx, args, 2)?)?;
    let stats_on =
        !omitted(args, 3) && super::helpers::bool_of(args.get(3).unwrap_or(&Value::blank()))?;
    Ok(Value::Matrix(coefficient_table(&fit, stats_on, |value| {
        value
    })?))
}
fn fn_logest(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let lay = logs(layout(ctx, args)?)?;
    let fit = fit_least_squares(&lay.x, &lay.y, with_const(ctx, args, 2)?)?;
    let stats_on =
        !omitted(args, 3) && super::helpers::bool_of(args.get(3).unwrap_or(&Value::blank()))?;
    let table = coefficient_table(&fit, stats_on, f64::exp)?;
    if !stats_on {
        return Ok(Value::Matrix(table));
    }
    let plain = coefficient_table(&fit, true, |value| value)?;
    let mut rows = table.data()?;
    let plain_rows = plain.data()?;
    if rows.len() > 1 && plain_rows.len() > 1 {
        rows.splice(1.., plain_rows.into_iter().skip(1));
    }
    Ok(Value::Matrix(Matrix::from_rows(rows)))
}
fn fn_trend(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let lay = layout(ctx, args)?;
    let fit = fit_least_squares(&lay.x, &lay.y, with_const(ctx, args, 3)?)?;
    Ok(Value::Matrix(predictions(
        ctx,
        args,
        &lay,
        &fit,
        |value| value,
    )?))
}
fn fn_growth(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let lay = logs(layout(ctx, args)?)?;
    let fit = fit_least_squares(&lay.x, &lay.y, with_const(ctx, args, 3)?)?;
    Ok(Value::Matrix(predictions(ctx, args, &lay, &fit, f64::exp)?))
}

fn num_with(
    name: &'static str,
    syntax: &'static str,
    description: &'static str,
    min: usize,
    max: usize,
    defaults: &'static [Option<f64>],
    f: fn(&[f64]) -> Fx<f64>,
) -> FunctionSpec {
    num_spec(name, C, syntax, description, min, max, defaults, f)
}

pub(crate) fn specs() -> Vec<FunctionSpec> {
    let out = vec![
        g(
            "LINEST",
            "LINEST(known_y, [known_x], [const], [stats])",
            "The statistics of a straight-line least-squares fit.",
            1,
            4,
            ANY,
            fn_linest,
        ),
        g(
            "LOGEST",
            "LOGEST(known_y, [known_x], [const], [stats])",
            "The statistics of an exponential-curve fit.",
            1,
            4,
            ANY,
            fn_logest,
        ),
        g(
            "TREND",
            "TREND(known_y, [known_x], [new_x], [const])",
            "Values along a linear trend.",
            1,
            4,
            ANY,
            fn_trend,
        ),
        g(
            "GROWTH",
            "GROWTH(known_y, [known_x], [new_x], [const])",
            "Values along an exponential trend.",
            1,
            4,
            ANY,
            fn_growth,
        ),
        g(
            "Z.TEST",
            "Z.TEST(array, x, [sigma])",
            "The one-tailed P-value of a z-test.",
            2,
            3,
            ANY,
            fn_ztest,
        ),
        g(
            "ZTEST",
            "ZTEST(array, x, [sigma])",
            "The one-tailed P-value of a z-test.",
            2,
            3,
            ANY,
            fn_ztest,
        ),
        g(
            "T.TEST",
            "T.TEST(array1, array2, tails, type)",
            "The probability associated with a Student's t-test.",
            4,
            4,
            ANY,
            fn_ttest,
        ),
        g(
            "TTEST",
            "TTEST(array1, array2, tails, type)",
            "The probability associated with a Student's t-test.",
            4,
            4,
            ANY,
            fn_ttest,
        ),
        g(
            "F.TEST",
            "F.TEST(array1, array2)",
            "The two-tailed probability that two variances are not different.",
            2,
            2,
            ANY,
            fn_ftest,
        ),
        g(
            "FTEST",
            "FTEST(array1, array2)",
            "The two-tailed probability that two variances are not different.",
            2,
            2,
            ANY,
            fn_ftest,
        ),
        g(
            "CHISQ.TEST",
            "CHISQ.TEST(actual_range, expected_range)",
            "The chi-squared test of independence.",
            2,
            2,
            ANY,
            fn_chisq_test,
        ),
        g(
            "CHITEST",
            "CHITEST(actual_range, expected_range)",
            "The chi-squared test of independence.",
            2,
            2,
            ANY,
            fn_chisq_test,
        ),
        g(
            "PROB",
            "PROB(x_range, prob_range, lower_limit, [upper_limit])",
            "The probability that values lie between two limits.",
            3,
            4,
            PROB_PARAMS,
            fn_prob,
        ),
        g(
            "STDEV",
            "STDEV(number1, [number2], ...)",
            "The sample standard deviation.",
            1,
            255,
            ANY,
            stdev_call(true, false),
        ),
        g(
            "STDEV.S",
            "STDEV.S(number1, [number2], ...)",
            "The sample standard deviation.",
            1,
            255,
            ANY,
            stdev_call(true, false),
        ),
        g(
            "STDEVP",
            "STDEVP(number1, [number2], ...)",
            "The population standard deviation.",
            1,
            255,
            ANY,
            stdev_call(false, false),
        ),
        g(
            "STDEV.P",
            "STDEV.P(number1, [number2], ...)",
            "The population standard deviation.",
            1,
            255,
            ANY,
            stdev_call(false, false),
        ),
        g(
            "STDEVA",
            "STDEVA(value1, [value2], ...)",
            "The sample standard deviation counting logicals and text.",
            1,
            255,
            ANY,
            stdev_call(true, true),
        ),
        g(
            "STDEVPA",
            "STDEVPA(value1, [value2], ...)",
            "The population standard deviation counting logicals and text.",
            1,
            255,
            ANY,
            stdev_call(false, true),
        ),
        g(
            "VAR",
            "VAR(number1, [number2], ...)",
            "The sample variance.",
            1,
            255,
            ANY,
            var_call(true, false),
        ),
        g(
            "VAR.S",
            "VAR.S(number1, [number2], ...)",
            "The sample variance.",
            1,
            255,
            ANY,
            var_call(true, false),
        ),
        g(
            "VARP",
            "VARP(number1, [number2], ...)",
            "The population variance.",
            1,
            255,
            ANY,
            var_call(false, false),
        ),
        g(
            "VAR.P",
            "VAR.P(number1, [number2], ...)",
            "The population variance.",
            1,
            255,
            ANY,
            var_call(false, false),
        ),
        g(
            "VARA",
            "VARA(value1, [value2], ...)",
            "The sample variance counting logicals and text.",
            1,
            255,
            ANY,
            var_call(true, true),
        ),
        g(
            "VARPA",
            "VARPA(value1, [value2], ...)",
            "The population variance counting logicals and text.",
            1,
            255,
            ANY,
            var_call(false, true),
        ),
        g(
            "PERCENTILE",
            "PERCENTILE(array, k)",
            "The k-th percentile.",
            2,
            2,
            ANY_VALUE,
            percentile_call(false),
        ),
        g(
            "PERCENTILE.INC",
            "PERCENTILE.INC(array, k)",
            "The k-th percentile.",
            2,
            2,
            ANY_VALUE,
            percentile_call(false),
        ),
        g(
            "PERCENTILE.EXC",
            "PERCENTILE.EXC(array, k)",
            "The k-th percentile.",
            2,
            2,
            ANY_VALUE,
            percentile_call(true),
        ),
        g(
            "QUARTILE",
            "QUARTILE(array, quart)",
            "A quartile of a data set.",
            2,
            2,
            ANY_VALUE,
            quartile_call(false),
        ),
        g(
            "QUARTILE.INC",
            "QUARTILE.INC(array, quart)",
            "A quartile of a data set.",
            2,
            2,
            ANY_VALUE,
            quartile_call(false),
        ),
        g(
            "QUARTILE.EXC",
            "QUARTILE.EXC(array, quart)",
            "A quartile of a data set.",
            2,
            2,
            ANY_VALUE,
            quartile_call(true),
        ),
        g(
            "PERCENTRANK",
            "PERCENTRANK(array, x, [significance])",
            "The rank of a value as a percentage.",
            2,
            3,
            PERCENT_RANK,
            percent_rank_call(false),
        ),
        g(
            "PERCENTRANK.INC",
            "PERCENTRANK.INC(array, x, [significance])",
            "The rank of a value as a percentage.",
            2,
            3,
            PERCENT_RANK,
            percent_rank_call(false),
        ),
        g(
            "PERCENTRANK.EXC",
            "PERCENTRANK.EXC(array, x, [significance])",
            "The rank of a value as a percentage.",
            2,
            3,
            PERCENT_RANK,
            percent_rank_call(true),
        ),
        g(
            "GEOMEAN",
            "GEOMEAN(number1, ...)",
            "The geometric mean.",
            1,
            255,
            ANY,
            fn_geomean,
        ),
        g(
            "HARMEAN",
            "HARMEAN(number1, ...)",
            "The harmonic mean.",
            1,
            255,
            ANY,
            fn_harmean,
        ),
        g(
            "AVEDEV",
            "AVEDEV(number1, ...)",
            "The mean absolute deviation from the mean.",
            1,
            255,
            ANY,
            fn_avedev,
        ),
        g(
            "DEVSQ",
            "DEVSQ(number1, ...)",
            "The sum of squared deviations from the mean.",
            1,
            255,
            ANY,
            fn_devsq,
        ),
        g(
            "KURT",
            "KURT(number1, ...)",
            "The kurtosis of a data set.",
            1,
            255,
            ANY,
            fn_kurt,
        ),
        g(
            "SKEW",
            "SKEW(number1, ...)",
            "The skewness of a distribution.",
            1,
            255,
            ANY,
            fn_skew,
        ),
        g(
            "SKEW.P",
            "SKEW.P(number1, ...)",
            "The population skewness.",
            1,
            255,
            ANY,
            fn_skew_p,
        ),
        n(
            "STANDARDIZE",
            "STANDARDIZE(x, mean, standard_dev)",
            "A normalized value.",
            3,
            3,
            fn_standardize,
        ),
        g(
            "TRIMMEAN",
            "TRIMMEAN(array, percent)",
            "The mean of the interior of a data set.",
            2,
            2,
            ANY_VALUE,
            fn_trimmean,
        ),
        g(
            "FREQUENCY",
            "FREQUENCY(data_array, bins_array)",
            "How often values occur within ranges.",
            2,
            2,
            ANY,
            fn_frequency,
        ),
        n(
            "PERMUT",
            "PERMUT(number, number_chosen)",
            "The number of permutations.",
            2,
            2,
            fn_permut,
        ),
        n(
            "PERMUTATIONA",
            "PERMUTATIONA(number, number_chosen)",
            "Permutations with repetitions.",
            2,
            2,
            fn_permutationa,
        ),
        n(
            "FISHER",
            "FISHER(x)",
            "The Fisher transformation.",
            1,
            1,
            fn_fisher,
        ),
        n(
            "FISHERINV",
            "FISHERINV(y)",
            "The inverse of the Fisher transformation.",
            1,
            1,
            fn_fisherinv,
        ),
        n(
            "GAMMA",
            "GAMMA(number)",
            "The gamma function value.",
            1,
            1,
            fn_gamma,
        ),
        n(
            "GAMMALN",
            "GAMMALN(x)",
            "The natural logarithm of the gamma function.",
            1,
            1,
            fn_gammaln,
        ),
        n(
            "GAMMALN.PRECISE",
            "GAMMALN.PRECISE(x)",
            "The natural logarithm of the gamma function.",
            1,
            1,
            fn_gammaln,
        ),
        g(
            "MODE.MULT",
            "MODE.MULT(number1, [number2], ...)",
            "A vertical array of the most frequent values.",
            1,
            255,
            ANY,
            fn_mode_mult,
        ),
        g(
            "CORREL",
            "CORREL(array1, array2)",
            "The correlation coefficient of two data sets.",
            2,
            2,
            ANY,
            fn_correl,
        ),
        g(
            "PEARSON",
            "PEARSON(array1, array2)",
            "The Pearson correlation coefficient.",
            2,
            2,
            ANY,
            fn_correl,
        ),
        g(
            "RSQ",
            "RSQ(known_ys, known_xs)",
            "The square of the Pearson correlation coefficient.",
            2,
            2,
            ANY,
            fn_rsq,
        ),
        g(
            "COVAR",
            "COVAR(array1, array2)",
            "The population covariance.",
            2,
            2,
            ANY,
            fn_covar,
        ),
        g(
            "COVARIANCE.P",
            "COVARIANCE.P(array1, array2)",
            "The population covariance.",
            2,
            2,
            ANY,
            fn_covar,
        ),
        g(
            "COVARIANCE.S",
            "COVARIANCE.S(array1, array2)",
            "The sample covariance.",
            2,
            2,
            ANY,
            fn_covar_s,
        ),
        g(
            "SLOPE",
            "SLOPE(known_ys, known_xs)",
            "The slope of the linear regression line.",
            2,
            2,
            ANY,
            fn_slope,
        ),
        g(
            "INTERCEPT",
            "INTERCEPT(known_ys, known_xs)",
            "The intercept of the linear regression line.",
            2,
            2,
            ANY,
            fn_intercept,
        ),
        g(
            "STEYX",
            "STEYX(known_ys, known_xs)",
            "The standard error of the predicted y values.",
            2,
            2,
            ANY,
            fn_steyx,
        ),
        g(
            "FORECAST",
            "FORECAST(x, known_ys, known_xs)",
            "Predicts a value along a linear trend.",
            3,
            3,
            FORECAST_PARAMS,
            fn_forecast,
        ),
        g(
            "FORECAST.LINEAR",
            "FORECAST.LINEAR(x, known_ys, known_xs)",
            "Predicts a value along a linear trend.",
            3,
            3,
            FORECAST_PARAMS,
            fn_forecast,
        ),
        g(
            "AVERAGEA",
            "AVERAGEA(value1, [value2], ...)",
            "The mean counting text as 0 and logicals.",
            1,
            255,
            ANY,
            fn_averagea,
        ),
        g(
            "AVERAGEIF",
            "AVERAGEIF(range, criteria, [average_range])",
            "The mean of the cells that meet a criterion.",
            2,
            3,
            &[ParamKind::Any, ParamKind::Value, ParamKind::Any],
            fn_averageif,
        ),
        g(
            "AVERAGEIFS",
            "AVERAGEIFS(average_range, criteria_range1, criteria1, ...)",
            "The mean of the cells that meet several criteria.",
            3,
            255,
            ANY,
            fn_averageifs,
        ),
        g(
            "MAX",
            "MAX(number1, [number2], ...)",
            "The largest value.",
            1,
            255,
            ANY,
            fn_max,
        ),
        g(
            "MIN",
            "MIN(number1, [number2], ...)",
            "The smallest value.",
            1,
            255,
            ANY,
            fn_min,
        ),
        g(
            "MAXA",
            "MAXA(value1, [value2], ...)",
            "The largest value counting logicals and text.",
            1,
            255,
            ANY,
            fn_maxa,
        ),
        g(
            "MINA",
            "MINA(value1, [value2], ...)",
            "The smallest value counting logicals and text.",
            1,
            255,
            ANY,
            fn_mina,
        ),
        g(
            "MAXIFS",
            "MAXIFS(max_range, criteria_range1, criteria1, ...)",
            "The largest value meeting criteria.",
            3,
            255,
            ANY,
            fn_maxifs,
        ),
        g(
            "MINIFS",
            "MINIFS(min_range, criteria_range1, criteria1, ...)",
            "The smallest value meeting criteria.",
            3,
            255,
            ANY,
            fn_minifs,
        ),
        g(
            "MEDIAN",
            "MEDIAN(number1, [number2], ...)",
            "The median.",
            1,
            255,
            ANY,
            fn_median,
        ),
        g(
            "MODE",
            "MODE(number1, [number2], ...)",
            "The most frequent value.",
            1,
            255,
            ANY,
            fn_mode,
        ),
        g(
            "MODE.SNGL",
            "MODE.SNGL(number1, [number2], ...)",
            "The most frequent value.",
            1,
            255,
            ANY,
            fn_mode,
        ),
        g(
            "LARGE",
            "LARGE(array, k)",
            "The k-th largest value.",
            2,
            2,
            ANY_VALUE,
            fn_large,
        ),
        g(
            "SMALL",
            "SMALL(array, k)",
            "The k-th smallest value.",
            2,
            2,
            ANY_VALUE,
            fn_small,
        ),
        g(
            "RANK",
            "RANK(number, ref, [order])",
            "The rank of a number in a list.",
            2,
            3,
            RANK_PARAMS,
            rank_of("RANK"),
        ),
        g(
            "RANK.EQ",
            "RANK.EQ(number, ref, [order])",
            "The rank of a number in a list.",
            2,
            3,
            RANK_PARAMS,
            rank_of("RANK.EQ"),
        ),
        g(
            "RANK.AVG",
            "RANK.AVG(number, ref, [order])",
            "The rank of a number in a list.",
            2,
            3,
            RANK_PARAMS,
            rank_of("RANK.AVG"),
        ),
        g(
            "COUNT",
            "COUNT(value1, [value2], ...)",
            "Counts the numbers in the arguments.",
            0,
            255,
            ANY,
            fn_count,
        ),
        g(
            "COUNTA",
            "COUNTA(value1, [value2], ...)",
            "Counts the non-empty values.",
            1,
            255,
            ANY,
            fn_counta,
        ),
        g(
            "COUNTBLANK",
            "COUNTBLANK(range)",
            "Counts the empty cells in a range.",
            1,
            1,
            ANY,
            fn_countblank,
        ),
        g(
            "COUNTIF",
            "COUNTIF(range, criteria)",
            "Counts the cells that meet a criterion.",
            2,
            2,
            ANY_VALUE,
            fn_countif,
        ),
        g(
            "COUNTIFS",
            "COUNTIFS(criteria_range1, criteria1, ...)",
            "Counts the cells that meet several criteria.",
            2,
            254,
            ANY,
            fn_countifs,
        ),
        g(
            "AVERAGE",
            "AVERAGE(number1, [number2], ...)",
            "The arithmetic mean.",
            1,
            255,
            ANY,
            fn_average,
        ),
        n(
            "NORM.DIST",
            "NORM.DIST(x, mean, standard_dev, cumulative)",
            "The normal distribution.",
            4,
            4,
            norm_dist_fn,
        ),
        n(
            "NORMDIST",
            "NORMDIST(x, mean, standard_dev, cumulative)",
            "The normal distribution.",
            4,
            4,
            norm_dist_fn,
        ),
        n(
            "NORM.S.DIST",
            "NORM.S.DIST(z, cumulative)",
            "The standard normal distribution.",
            2,
            2,
            norm_s_dist,
        ),
        n(
            "NORMSDIST",
            "NORMSDIST(z)",
            "The standard normal cumulative distribution.",
            1,
            1,
            normsdist,
        ),
        n(
            "NORM.INV",
            "NORM.INV(probability, mean, standard_dev)",
            "The inverse of the normal cumulative distribution.",
            3,
            3,
            norm_inv_fn,
        ),
        n(
            "NORMINV",
            "NORMINV(probability, mean, standard_dev)",
            "The inverse of the normal cumulative distribution.",
            3,
            3,
            norm_inv_fn,
        ),
        n(
            "NORM.S.INV",
            "NORM.S.INV(probability)",
            "The inverse of the standard normal distribution.",
            1,
            1,
            |a| dist::norm_inv(a[0]),
        ),
        n(
            "NORMSINV",
            "NORMSINV(probability)",
            "The inverse of the standard normal distribution.",
            1,
            1,
            |a| dist::norm_inv(a[0]),
        ),
        n("PHI", "PHI(x)", "The standard normal density.", 1, 1, phi),
        n(
            "GAUSS",
            "GAUSS(z)",
            "The probability between the mean and z standard deviations.",
            1,
            1,
            gauss,
        ),
        n(
            "CONFIDENCE",
            "CONFIDENCE(alpha, standard_dev, size)",
            "The confidence interval for a population mean (normal).",
            3,
            3,
            confidence_norm,
        ),
        n(
            "CONFIDENCE.NORM",
            "CONFIDENCE.NORM(alpha, standard_dev, size)",
            "The confidence interval for a population mean (normal).",
            3,
            3,
            confidence_norm,
        ),
        n(
            "CONFIDENCE.T",
            "CONFIDENCE.T(alpha, standard_dev, size)",
            "The confidence interval for a population mean (Student's t).",
            3,
            3,
            confidence_t,
        ),
        n(
            "BINOM.DIST",
            "BINOM.DIST(number_s, trials, probability_s, cumulative)",
            "The binomial distribution probability.",
            4,
            4,
            binom,
        ),
        n(
            "BINOMDIST",
            "BINOMDIST(number_s, trials, probability_s, cumulative)",
            "The binomial distribution probability.",
            4,
            4,
            binom,
        ),
        n(
            "POISSON.DIST",
            "POISSON.DIST(x, mean, cumulative)",
            "The Poisson distribution.",
            3,
            3,
            poisson_fn,
        ),
        n(
            "POISSON",
            "POISSON(x, mean, cumulative)",
            "The Poisson distribution.",
            3,
            3,
            poisson_fn,
        ),
        n(
            "EXPON.DIST",
            "EXPON.DIST(x, lambda, cumulative)",
            "The exponential distribution.",
            3,
            3,
            expon,
        ),
        n(
            "EXPONDIST",
            "EXPONDIST(x, lambda, cumulative)",
            "The exponential distribution.",
            3,
            3,
            expon,
        ),
        n(
            "LOGNORM.DIST",
            "LOGNORM.DIST(x, mean, standard_dev, cumulative)",
            "The lognormal distribution.",
            4,
            4,
            lognorm_dist,
        ),
        n(
            "LOGNORM.INV",
            "LOGNORM.INV(probability, mean, standard_dev)",
            "The inverse of the lognormal distribution.",
            3,
            3,
            lognorm_inv,
        ),
        n(
            "WEIBULL.DIST",
            "WEIBULL.DIST(x, alpha, beta, cumulative)",
            "The Weibull distribution.",
            4,
            4,
            weibull,
        ),
        n(
            "WEIBULL",
            "WEIBULL(x, alpha, beta, cumulative)",
            "The Weibull distribution.",
            4,
            4,
            weibull,
        ),
        n(
            "GAMMA.DIST",
            "GAMMA.DIST(x, alpha, beta, cumulative)",
            "The gamma distribution.",
            4,
            4,
            gamma_dist_fn,
        ),
        n(
            "T.DIST",
            "T.DIST(x, deg_freedom, cumulative)",
            "Student's left-tailed t-distribution.",
            3,
            3,
            t_dist,
        ),
        n(
            "T.DIST.RT",
            "T.DIST.RT(x, deg_freedom)",
            "Student's right-tailed t-distribution.",
            2,
            2,
            t_dist_rt,
        ),
        n(
            "T.DIST.2T",
            "T.DIST.2T(x, deg_freedom)",
            "Student's two-tailed t-distribution.",
            2,
            2,
            t_dist_2t,
        ),
        n(
            "TDIST",
            "TDIST(x, deg_freedom, tails)",
            "Student's t-distribution.",
            3,
            3,
            tdist,
        ),
        n(
            "T.INV",
            "T.INV(probability, deg_freedom)",
            "The left-tailed inverse of Student's t-distribution.",
            2,
            2,
            t_inv,
        ),
        n(
            "T.INV.2T",
            "T.INV.2T(probability, deg_freedom)",
            "The two-tailed inverse of Student's t-distribution.",
            2,
            2,
            t_inv_2t,
        ),
        n(
            "TINV",
            "TINV(probability, deg_freedom)",
            "The two-tailed inverse of Student's t-distribution.",
            2,
            2,
            t_inv_2t,
        ),
        n(
            "CHISQ.DIST",
            "CHISQ.DIST(x, deg_freedom, cumulative)",
            "The chi-squared distribution.",
            3,
            3,
            chisq_dist,
        ),
        n(
            "CHISQ.DIST.RT",
            "CHISQ.DIST.RT(x, deg_freedom)",
            "The right-tailed chi-squared probability.",
            2,
            2,
            chisq_rt,
        ),
        n(
            "CHIDIST",
            "CHIDIST(x, deg_freedom)",
            "The right-tailed chi-squared probability.",
            2,
            2,
            chisq_rt,
        ),
        n(
            "CHISQ.INV",
            "CHISQ.INV(probability, deg_freedom)",
            "The inverse of the left-tailed chi-squared distribution.",
            2,
            2,
            chisq_inv,
        ),
        n(
            "CHISQ.INV.RT",
            "CHISQ.INV.RT(probability, deg_freedom)",
            "The inverse of the right-tailed chi-squared distribution.",
            2,
            2,
            chisq_inv_rt,
        ),
        n(
            "CHIINV",
            "CHIINV(probability, deg_freedom)",
            "The inverse of the right-tailed chi-squared distribution.",
            2,
            2,
            chisq_inv_rt,
        ),
        n(
            "BETA.DIST",
            "BETA.DIST(x, alpha, beta, cumulative, [A], [B])",
            "The beta distribution.",
            4,
            6,
            beta_dist,
        ),
        n(
            "NEGBINOM.DIST",
            "NEGBINOM.DIST(number_f, number_s, probability_s, cumulative)",
            "The negative binomial distribution.",
            4,
            4,
            negbinom,
        ),
        n(
            "HYPGEOM.DIST",
            "HYPGEOM.DIST(sample_s, number_sample, population_s, number_pop, cumulative)",
            "The hypergeometric distribution.",
            5,
            5,
            hypgeom,
        ),
        n(
            "F.DIST",
            "F.DIST(x, deg_freedom1, deg_freedom2, cumulative)",
            "The left-tailed F probability distribution.",
            4,
            4,
            f_dist,
        ),
        n(
            "F.DIST.RT",
            "F.DIST.RT(x, deg_freedom1, deg_freedom2)",
            "The right-tailed F probability distribution.",
            3,
            3,
            f_dist_rt,
        ),
        n(
            "FDIST",
            "FDIST(x, deg_freedom1, deg_freedom2)",
            "The right-tailed F probability distribution.",
            3,
            3,
            f_dist_rt,
        ),
        n(
            "F.INV",
            "F.INV(probability, deg_freedom1, deg_freedom2)",
            "The inverse of the left-tailed F distribution.",
            3,
            3,
            f_inv,
        ),
        n(
            "F.INV.RT",
            "F.INV.RT(probability, deg_freedom1, deg_freedom2)",
            "The inverse of the right-tailed F distribution.",
            3,
            3,
            f_inv_rt,
        ),
        n(
            "FINV",
            "FINV(probability, deg_freedom1, deg_freedom2)",
            "The inverse of the right-tailed F distribution.",
            3,
            3,
            f_inv_rt,
        ),
        num_with(
            "BETA.INV",
            "BETA.INV(probability, alpha, beta, [A], [B])",
            "The inverse of the cumulative beta distribution.",
            3,
            5,
            BETA_DEFAULTS,
            beta_inv,
        ),
        num_with(
            "BETAINV",
            "BETAINV(probability, alpha, beta, [A], [B])",
            "The inverse of the cumulative beta distribution.",
            3,
            5,
            BETA_DEFAULTS,
            beta_inv,
        ),
        num_with(
            "BETADIST",
            "BETADIST(x, alpha, beta, [A], [B])",
            "The cumulative beta distribution.",
            3,
            5,
            BETA_DEFAULTS,
            betadist,
        ),
        n(
            "GAMMA.INV",
            "GAMMA.INV(probability, alpha, beta)",
            "The inverse of the gamma cumulative distribution.",
            3,
            3,
            gamma_inv,
        ),
        n(
            "GAMMAINV",
            "GAMMAINV(probability, alpha, beta)",
            "The inverse of the gamma cumulative distribution.",
            3,
            3,
            gamma_inv,
        ),
        n(
            "GAMMADIST",
            "GAMMADIST(x, alpha, beta, cumulative)",
            "The gamma distribution.",
            4,
            4,
            gamma_dist_fn,
        ),
        n(
            "LOGINV",
            "LOGINV(probability, mean, standard_dev)",
            "The inverse of the lognormal cumulative distribution.",
            3,
            3,
            loginv,
        ),
        n(
            "LOGNORMDIST",
            "LOGNORMDIST(x, mean, standard_dev)",
            "The cumulative lognormal distribution.",
            3,
            3,
            lognormdist,
        ),
        n(
            "BINOM.INV",
            "BINOM.INV(trials, probability_s, alpha)",
            "The smallest value for a cumulative binomial criterion.",
            3,
            3,
            binom_inv,
        ),
        n(
            "CRITBINOM",
            "CRITBINOM(trials, probability_s, alpha)",
            "The smallest value for a cumulative binomial criterion.",
            3,
            3,
            binom_inv,
        ),
        n(
            "BINOM.DIST.RANGE",
            "BINOM.DIST.RANGE(trials, probability_s, number_s, [number_s2])",
            "The binomial probability of a trial result in a range.",
            3,
            4,
            binom_range,
        ),
        n(
            "HYPGEOMDIST",
            "HYPGEOMDIST(sample_s, number_sample, population_s, number_pop)",
            "The hypergeometric distribution.",
            4,
            4,
            hypgeomdist,
        ),
        n(
            "NEGBINOMDIST",
            "NEGBINOMDIST(number_f, number_s, probability_s)",
            "The negative binomial distribution.",
            3,
            3,
            negbinomdist,
        ),
    ];
    out
}
