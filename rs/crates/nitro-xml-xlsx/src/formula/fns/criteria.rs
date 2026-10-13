// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/criteria.ts; changes listed in PROVENANCE.md.
//! SUMIF / COUNTIF / AVERAGEIF / MAXIFS / MINIFS matching.
//! The criteria index in criteria-index.ts is an optimization; scanning is the same result.
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]

use super::helpers::{make_criteria, scalar_of};
use super::host::CallCtx;
use super::ops::pick;
use super::value::{Area, ErrorCode, Fx, Matrix, Scalar, Value};

const CLIP: i64 = 100_000;

#[derive(Clone)]
pub struct CriteriaPair {
    pub range: Value,
    pub criteria: Value,
}

pub struct Matches {
    pub values: Vec<Scalar>,
    pub blank_tail: f64,
}

fn area_of(value: &Value) -> Fx<Area> {
    let Value::Ref(reference) = value else {
        return Err(ErrorCode::Value);
    };
    if reference.areas.len() != 1 {
        return Err(ErrorCode::Value);
    }
    reference.areas.first().copied().ok_or(ErrorCode::Value)
}

fn shape_of(value: &Value) -> Fx<(i64, i64)> {
    match value {
        Value::Ref(reference) => {
            let area = area_of(value)?;
            let _ = reference;
            Ok((
                (area.end_row as i64 - area.start_row as i64 + 1).max(0),
                (area.end_col as i64 - area.start_col as i64 + 1).max(0),
            ))
        }
        Value::Matrix(matrix) => Ok((matrix.rows() as i64, matrix.cols() as i64)),
        Value::Scalar(_) => Ok((1, 1)),
        Value::Lambda(_) => Err(ErrorCode::Calc),
    }
}

fn read_block(ctx: &CallCtx<'_>, area: Area, rows: usize, cols: usize) -> Fx<Matrix> {
    Matrix::build(rows, cols, |r, c| {
        Ok(ctx.read_cell(
            area.sheet,
            area.start_row + r as i32,
            area.start_col + c as i32,
        ))
    })
}

fn shaped_matrices(
    ctx: &CallCtx<'_>,
    values: &[Value],
    same_shape: bool,
) -> Fx<(Vec<Matrix>, f64, f64)> {
    let (mut rows, mut cols) = shape_of(values.first().unwrap_or(&Value::blank()))?;
    if same_shape {
        for value in values {
            let shape = shape_of(value)?;
            if shape.0 != rows || shape.1 != cols {
                return Err(ErrorCode::Value);
            }
        }
    }
    let total = rows as f64 * cols as f64;
    if total > CLIP as f64 && values.iter().all(|value| matches!(value, Value::Ref(_))) {
        let mut max_rows = 1i64;
        let mut max_cols = 1i64;
        for value in values {
            let area = area_of(value)?;
            let (bound_rows, bound_cols) = ctx.host.bounds(area.sheet);
            max_rows = max_rows.max(bound_rows as i64 - area.start_row as i64);
            max_cols = max_cols.max(bound_cols as i64 - area.start_col as i64);
        }
        rows = rows.min(max_rows).max(0);
        cols = cols.min(max_cols).max(0);
    }
    let rows_us = usize::try_from(rows).unwrap_or(0);
    let cols_us = usize::try_from(cols).unwrap_or(0);
    let mut matrices = Vec::with_capacity(values.len());
    for value in values {
        let matrix = if matches!(value, Value::Ref(_)) {
            read_block(ctx, area_of(value)?, rows_us, cols_us)?
        } else if let Value::Matrix(matrix) = value {
            matrix.clone()
        } else {
            Matrix::from_rows(vec![vec![scalar_of(value)?]])
        };
        matrices.push(matrix);
    }
    Ok((matrices, total, (rows_us as f64) * (cols_us as f64)))
}

pub fn matching_cells(
    ctx: &CallCtx<'_>,
    pairs: &[CriteriaPair],
    target: Option<&Value>,
    same_shape: bool,
) -> Fx<Matches> {
    let ranges: Vec<Value> = pairs.iter().map(|pair| pair.range.clone()).collect();
    let owned_target;
    let inputs: Vec<Value> = if let Some(target) = target {
        owned_target = target.clone();
        let mut all = ranges;
        all.push(owned_target);
        all
    } else {
        ranges
    };
    let (matrices, total, block) = shaped_matrices(ctx, &inputs, same_shape)?;
    let mut tests = Vec::with_capacity(pairs.len());
    for pair in pairs {
        tests.push(make_criteria(scalar_of(&pair.criteria)?, ctx.date1904));
    }
    let values = matrices
        .last()
        .cloned()
        .unwrap_or_else(|| Matrix::from_rows(vec![]));
    let first = matrices
        .first()
        .cloned()
        .unwrap_or_else(|| Matrix::from_rows(vec![]));
    let mut out = Vec::new();
    for r in 0..first.rows() {
        for c in 0..first.cols() {
            let mut holds = true;
            for (index, test) in tests.iter().enumerate() {
                let cell = matrices
                    .get(index)
                    .map(|matrix| matrix.get(r, c))
                    .unwrap_or(Scalar::Blank);
                if !test(&cell) {
                    holds = false;
                    break;
                }
            }
            if holds {
                out.push(values.get(r, c));
            }
        }
    }
    let blank_tail = if total > block && tests.iter().all(|test| test(&Scalar::Blank)) {
        total - block
    } else {
        0.0
    };
    Ok(Matches {
        values: out,
        blank_tail,
    })
}

pub fn matching_values(
    ctx: &CallCtx<'_>,
    pairs: &[CriteriaPair],
    target: Option<&Value>,
    same_shape: bool,
) -> Fx<Vec<Scalar>> {
    Ok(matching_cells(ctx, pairs, target, same_shape)?.values)
}

pub fn criteria_pairs(args: &[Value], from: usize) -> Fx<Vec<CriteriaPair>> {
    if !(args.len() - from).is_multiple_of(2) {
        return Err(ErrorCode::Value);
    }
    let mut pairs = Vec::new();
    let mut i = from;
    while i < args.len() {
        pairs.push(CriteriaPair {
            range: args.get(i).cloned().unwrap_or_else(Value::blank),
            criteria: args.get(i + 1).cloned().unwrap_or_else(Value::blank),
        });
        i += 2;
    }
    Ok(pairs)
}

fn criteria_value(ctx: &CallCtx<'_>, value: &Value) -> Fx<Value> {
    if matches!(value, Value::Lambda(_)) {
        return Err(ErrorCode::Calc);
    }
    let mut current = value.clone();
    if let Value::Ref(reference) = &current {
        if reference.is_cell() {
            return Ok(Value::Scalar(ctx.to_scalar(&current)));
        }
        current = Value::Matrix(ctx.to_matrix(&current)?);
    }
    if let Value::Matrix(matrix) = &current {
        if matrix.rows() == 1 && matrix.cols() == 1 {
            return Ok(Value::Scalar(matrix.get(0, 0)));
        }
    }
    Ok(current)
}

pub fn lift_criteria(
    ctx: &CallCtx<'_>,
    pairs: &[CriteriaPair],
    mut func: impl FnMut(&[CriteriaPair]) -> Fx<Value>,
) -> Fx<Value> {
    let mut resolved = Vec::with_capacity(pairs.len());
    for pair in pairs {
        resolved.push(CriteriaPair {
            range: pair.range.clone(),
            criteria: criteria_value(ctx, &pair.criteria)?,
        });
    }
    let arrays: Vec<&Matrix> = resolved
        .iter()
        .filter_map(|pair| match &pair.criteria {
            Value::Matrix(matrix) => Some(matrix),
            _ => None,
        })
        .collect();
    if arrays.is_empty() {
        return func(&resolved);
    }
    let mut rows = 1usize;
    let mut cols = 1usize;
    for matrix in &arrays {
        rows = rows.max(matrix.rows());
        cols = cols.max(matrix.cols());
    }
    let built = Matrix::build(rows, cols, |r, c| {
        let one: Vec<CriteriaPair> = resolved
            .iter()
            .map(|pair| CriteriaPair {
                range: pair.range.clone(),
                criteria: match &pair.criteria {
                    Value::Matrix(matrix) => Value::Scalar(pick(matrix, r, c)),
                    other => other.clone(),
                },
            })
            .collect();
        match func(&one) {
            Ok(Value::Matrix(matrix)) => Ok(matrix.get(0, 0)),
            Ok(other) => match scalar_of(&other) {
                Ok(scalar) => Ok(scalar),
                Err(code) => Ok(Scalar::Error(code)),
            },
            Err(code) => Ok(Scalar::Error(code)),
        }
    })?;
    Ok(Value::Matrix(built))
}
