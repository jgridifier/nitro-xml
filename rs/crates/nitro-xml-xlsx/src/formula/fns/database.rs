// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/database.ts; changes listed in PROVENANCE.md.
//! Database functions: aggregate one column of the records that match a criteria range.
//!
//! Criteria cells use `make_criteria`, the same COUNTIF predicate `criteria.rs` scans with.
//! A database criteria range is a header plus OR-rows, so the SUMIF pair scanner does not apply.

use crate::number_format::js_number_string;
use crate::numfmt::js_int_string;

use super::helpers::{make_criteria, numbers_of, scalar_of};
use super::host::CallCtx;
use super::spec::{gen_spec, FunctionSpec, ParamKind};
use super::stats_core;
use super::value::{ErrorCode, Fx, Matrix, Scalar, Value};

const C: &str = "Database";
const ANY: &[ParamKind] = &[ParamKind::Any];

#[derive(Clone, Copy)]
enum Agg {
    Sum,
    Count,
    CountA,
    Average,
    Max,
    Min,
    Product,
    Get,
    Stdev,
    StdevP,
    Var,
    VarP,
}

enum Test {
    Never,
    Match {
        column: usize,
        pred: Box<dyn Fn(&Scalar) -> bool>,
    },
}

/// `String(value)` then lower-cased, matching the header labels upstream stores.
fn label(value: &Scalar) -> String {
    match value {
        Scalar::Blank => String::new(),
        Scalar::Text(text) => text.clone(),
        Scalar::Bool(true) => "true".to_owned(),
        Scalar::Bool(false) => "false".to_owned(),
        // A `CellError` object stringifies as `[object Object]`.
        Scalar::Error(_) => "[object Object]".to_owned(),
        Scalar::Number(n) if !n.is_finite() => js_int_string(*n),
        Scalar::Number(n) => js_number_string(*n),
    }
}

fn blank_criteria(value: &Scalar) -> bool {
    match value {
        Scalar::Blank => true,
        Scalar::Text(text) => text.is_empty(),
        _ => false,
    }
}

fn column_from_number(n: f64, width: usize) -> Fx<usize> {
    if !n.is_finite() {
        return Err(ErrorCode::Value);
    }
    let truncated = n.trunc();
    // Excel column counts fit in the exact integer range of f64.
    #[allow(clippy::cast_precision_loss)]
    let width_f = width as f64;
    if !(truncated >= 1.0 && truncated <= width_f) {
        return Err(ErrorCode::Value);
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let index = (truncated - 1.0) as usize;
    Ok(index)
}

fn field_index(headers: &[String], field: &Value) -> Fx<usize> {
    let value = scalar_of(field)?;
    match value {
        Scalar::Error(code) => Err(code),
        Scalar::Number(n) => column_from_number(n, headers.len()),
        other => {
            let key = label(&other).to_lowercase();
            headers
                .iter()
                .position(|header| header == &key)
                .ok_or(ErrorCode::Value)
        }
    }
}

/// Blank and unknown criteria headers are the same: a filled cell in that column never matches.
fn header_column(headers: &[String], name: &Scalar) -> Option<usize> {
    if blank_criteria(name) {
        return None;
    }
    let key = label(name).to_lowercase();
    headers.iter().position(|header| header == &key)
}

fn criteria_tests(headers: &[String], criteria: &Matrix, date1904: bool) -> Fx<Vec<Vec<Test>>> {
    let rows = criteria.rows();
    let cols = criteria.cols();
    if rows < 2 {
        return Err(ErrorCode::Value);
    }
    let columns: Vec<Option<usize>> = (0..cols)
        .map(|col| header_column(headers, &criteria.get(0, col)))
        .collect();
    let mut alternatives = Vec::with_capacity(rows - 1);
    for row in 1..rows {
        let mut tests = Vec::new();
        for col in 0..cols {
            let cell = criteria.get(row, col);
            if blank_criteria(&cell) {
                continue;
            }
            match columns.get(col).copied().flatten() {
                None => tests.push(Test::Never),
                Some(column) => tests.push(Test::Match {
                    column,
                    pred: make_criteria(cell, date1904),
                }),
            }
        }
        alternatives.push(tests);
    }
    Ok(alternatives)
}

fn holds(test: &Test, database: &Matrix, row: usize) -> bool {
    match test {
        Test::Never => false,
        Test::Match { column, pred } => pred(&database.get(row, *column)),
    }
}

fn selected(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Vec<Scalar>> {
    let blank = Value::blank();
    let database = ctx.to_matrix(args.first().unwrap_or(&blank))?;
    let rows = database.rows();
    let cols = database.cols();
    if rows < 2 {
        return Err(ErrorCode::Value);
    }
    let headers: Vec<String> = (0..cols)
        .map(|col| label(&database.get(0, col)).to_lowercase())
        .collect();
    let column = field_index(&headers, args.get(1).unwrap_or(&blank))?;
    let criteria = ctx.to_matrix(args.get(2).unwrap_or(&blank))?;
    let alternatives = criteria_tests(&headers, &criteria, ctx.date1904)?;
    let mut out = Vec::new();
    for row in 1..rows {
        let matched = alternatives
            .iter()
            .any(|tests| tests.iter().all(|test| holds(test, &database, row)));
        if matched {
            out.push(database.get(row, column));
        }
    }
    Ok(out)
}

fn numbers(values: &[Scalar]) -> Fx<Vec<f64>> {
    for value in values {
        if let Scalar::Error(code) = value {
            return Err(*code);
        }
    }
    Ok(numbers_of(values))
}

#[allow(clippy::cast_precision_loss)]
fn count_num(count: usize) -> f64 {
    count as f64
}

fn aggregate(ctx: &CallCtx<'_>, args: &[Value], agg: Agg) -> Fx<Value> {
    let values = selected(ctx, args)?;
    match agg {
        Agg::CountA => {
            let count = values
                .iter()
                .filter(|value| match value {
                    Scalar::Blank => false,
                    Scalar::Text(text) => !text.is_empty(),
                    _ => true,
                })
                .count();
            Ok(Value::number(count_num(count)))
        }
        Agg::Get => match values.as_slice() {
            [] => Err(ErrorCode::Value),
            [one] => Ok(Value::Scalar(one.clone())),
            _ => Err(ErrorCode::Num),
        },
        other => {
            let nums = numbers(&values)?;
            let result = match other {
                Agg::Sum => stats_core::sum(&nums),
                Agg::Count => count_num(nums.len()),
                Agg::Average => stats_core::mean(&nums)?,
                Agg::Max if nums.is_empty() => 0.0,
                Agg::Max => stats_core::extreme(&nums, true),
                Agg::Min if nums.is_empty() => 0.0,
                Agg::Min => stats_core::extreme(&nums, false),
                Agg::Product if nums.is_empty() => 0.0,
                Agg::Product => nums.iter().fold(1.0, |acc, value| acc * value),
                Agg::Stdev => stats_core::stdev(&nums, true)?,
                Agg::StdevP => stats_core::stdev(&nums, false)?,
                Agg::Var => stats_core::variance(&nums, true)?,
                Agg::VarP => stats_core::variance(&nums, false)?,
                Agg::CountA | Agg::Get => unreachable!("counted above"),
            };
            Ok(Value::number(result))
        }
    }
}

fn fn_dsum(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    aggregate(ctx, args, Agg::Sum)
}
fn fn_dcount(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    aggregate(ctx, args, Agg::Count)
}
fn fn_dcounta(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    aggregate(ctx, args, Agg::CountA)
}
fn fn_daverage(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    aggregate(ctx, args, Agg::Average)
}
fn fn_dmax(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    aggregate(ctx, args, Agg::Max)
}
fn fn_dmin(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    aggregate(ctx, args, Agg::Min)
}
fn fn_dproduct(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    aggregate(ctx, args, Agg::Product)
}
fn fn_dget(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    aggregate(ctx, args, Agg::Get)
}
fn fn_dstdev(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    aggregate(ctx, args, Agg::Stdev)
}
fn fn_dstdevp(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    aggregate(ctx, args, Agg::StdevP)
}
fn fn_dvar(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    aggregate(ctx, args, Agg::Var)
}
fn fn_dvarp(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    aggregate(ctx, args, Agg::VarP)
}

pub(super) fn specs() -> Vec<FunctionSpec> {
    vec![
        gen_spec(
            "DSUM",
            C,
            "DSUM(database, field, criteria)",
            "The sum of the matching records in a column.",
            3,
            3,
            ANY,
            false,
            fn_dsum,
        ),
        gen_spec(
            "DCOUNT",
            C,
            "DCOUNT(database, field, criteria)",
            "The count of numbers in the matching records.",
            3,
            3,
            ANY,
            false,
            fn_dcount,
        ),
        gen_spec(
            "DCOUNTA",
            C,
            "DCOUNTA(database, field, criteria)",
            "The count of non-blank cells in the matching records.",
            3,
            3,
            ANY,
            false,
            fn_dcounta,
        ),
        gen_spec(
            "DAVERAGE",
            C,
            "DAVERAGE(database, field, criteria)",
            "The mean of the matching records in a column.",
            3,
            3,
            ANY,
            false,
            fn_daverage,
        ),
        gen_spec(
            "DMAX",
            C,
            "DMAX(database, field, criteria)",
            "The largest number among the matching records.",
            3,
            3,
            ANY,
            false,
            fn_dmax,
        ),
        gen_spec(
            "DMIN",
            C,
            "DMIN(database, field, criteria)",
            "The smallest number among the matching records.",
            3,
            3,
            ANY,
            false,
            fn_dmin,
        ),
        gen_spec(
            "DPRODUCT",
            C,
            "DPRODUCT(database, field, criteria)",
            "The product of the matching records in a column.",
            3,
            3,
            ANY,
            false,
            fn_dproduct,
        ),
        gen_spec(
            "DGET",
            C,
            "DGET(database, field, criteria)",
            "The single value of the one matching record.",
            3,
            3,
            ANY,
            false,
            fn_dget,
        ),
        gen_spec(
            "DSTDEV",
            C,
            "DSTDEV(database, field, criteria)",
            "The sample standard deviation of the matching records.",
            3,
            3,
            ANY,
            false,
            fn_dstdev,
        ),
        gen_spec(
            "DSTDEVP",
            C,
            "DSTDEVP(database, field, criteria)",
            "The population standard deviation of the matching records.",
            3,
            3,
            ANY,
            false,
            fn_dstdevp,
        ),
        gen_spec(
            "DVAR",
            C,
            "DVAR(database, field, criteria)",
            "The sample variance of the matching records.",
            3,
            3,
            ANY,
            false,
            fn_dvar,
        ),
        gen_spec(
            "DVARP",
            C,
            "DVARP(database, field, criteria)",
            "The population variance of the matching records.",
            3,
            3,
            ANY,
            false,
            fn_dvarp,
        ),
    ]
}
