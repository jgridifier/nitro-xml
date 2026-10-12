// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/logical.ts and lambda.ts;
// changes listed in PROVENANCE.md.
//! Logical functions, including `LET`, `LAMBDA` and the array helpers that call them.
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]

use std::collections::HashMap;

use crate::formula::ast::{normalize_local_name, FormulaAst};

use super::coerce::to_bool;
use super::host::CallCtx;
use super::ops::{broadcast_shape, pick};
use super::spec::{lazy_spec, FunctionSpec, ParamKind};
use super::value::{ErrorCode, Fx, LambdaValue, Matrix, Scalar, Value};

const C: &str = "Logical";
const ANY: &[ParamKind] = &[ParamKind::Any];

pub fn specs() -> Vec<FunctionSpec> {
    let mut out = lambda_specs();
    out.extend([
        lazy_spec(
            "IF",
            C,
            "IF(logical_test, [value_if_true], [value_if_false])",
            "Returns one value if a condition is TRUE and another if FALSE.",
            1,
            3,
            if_fn,
        ),
        lazy_spec(
            "IFERROR",
            C,
            "IFERROR(value, value_if_error)",
            "A fallback value when an expression is an error.",
            2,
            2,
            |ctx, args| if_error(ctx, args, false),
        ),
        lazy_spec(
            "IFNA",
            C,
            "IFNA(value, value_if_na)",
            "A fallback value when an expression is #N/A.",
            2,
            2,
            |ctx, args| if_error(ctx, args, true),
        ),
        lazy_spec(
            "IFS",
            C,
            "IFS(logical_test1, value_if_true1, ...)",
            "The value for the first TRUE condition.",
            2,
            254,
            ifs_fn,
        ),
        lazy_spec(
            "SWITCH",
            C,
            "SWITCH(expression, value1, result1, ..., [default])",
            "The result for the first value matching an expression.",
            3,
            254,
            switch_fn,
        ),
        lazy_spec(
            "AND",
            C,
            "AND(logical1, [logical2], ...)",
            "TRUE when every argument is TRUE.",
            1,
            255,
            |ctx, args| logical_fold(ctx, args, LogicalFold::And),
        ),
        lazy_spec(
            "OR",
            C,
            "OR(logical1, [logical2], ...)",
            "TRUE when any argument is TRUE.",
            1,
            255,
            |ctx, args| logical_fold(ctx, args, LogicalFold::Or),
        ),
        lazy_spec(
            "XOR",
            C,
            "XOR(logical1, [logical2], ...)",
            "TRUE when an odd number of arguments are TRUE.",
            1,
            255,
            |ctx, args| logical_fold(ctx, args, LogicalFold::Xor),
        ),
        super::spec::gen_spec(
            "NOT",
            C,
            "NOT(logical)",
            "Reverses a logical value.",
            1,
            1,
            &[],
            false,
            not_fn,
        ),
        super::spec::gen_spec(
            "TRUE",
            C,
            "TRUE()",
            "The logical value TRUE.",
            0,
            0,
            &[],
            false,
            |_, _| Ok(Value::Scalar(Scalar::Bool(true))),
        ),
        super::spec::gen_spec(
            "FALSE",
            C,
            "FALSE()",
            "The logical value FALSE.",
            0,
            0,
            &[],
            false,
            |_, _| Ok(Value::Scalar(Scalar::Bool(false))),
        ),
        lazy_spec(
            "LET",
            C,
            "LET(name1, value1, [name2, value2, ...], calculation)",
            "Assigns names to values for use in a calculation.",
            3,
            253,
            let_fn,
        ),
        lazy_spec(
            "LAMBDA",
            C,
            "LAMBDA([parameter1, ...], calculation)",
            "Creates a reusable function.",
            1,
            254,
            lambda_fn,
        ),
    ]);
    out
}

fn lambda_specs() -> Vec<FunctionSpec> {
    vec![
        super::spec::gen_spec(
            "MAP",
            C,
            "MAP(array1, ..., lambda)",
            "Applies a LAMBDA to each value of arrays.",
            2,
            254,
            ANY,
            false,
            map_fn,
        ),
        super::spec::gen_spec(
            "REDUCE",
            C,
            "REDUCE([initial_value], array, lambda)",
            "Reduces an array to an accumulated value.",
            2,
            3,
            ANY,
            false,
            reduce_fn,
        ),
        super::spec::gen_spec(
            "SCAN",
            C,
            "SCAN([initial_value], array, lambda)",
            "The intermediate values of a reduction, as an array.",
            2,
            3,
            ANY,
            false,
            scan_fn,
        ),
        super::spec::gen_spec(
            "BYROW",
            C,
            "BYROW(array, lambda)",
            "Applies a LAMBDA to each row, returning a column.",
            2,
            2,
            ANY,
            false,
            byrow_fn,
        ),
        super::spec::gen_spec(
            "BYCOL",
            C,
            "BYCOL(array, lambda)",
            "Applies a LAMBDA to each column, returning a row.",
            2,
            2,
            ANY,
            false,
            bycol_fn,
        ),
        super::spec::gen_spec(
            "MAKEARRAY",
            C,
            "MAKEARRAY(rows, columns, lambda)",
            "An array built by calling a LAMBDA for each position.",
            3,
            3,
            ANY,
            false,
            makearray_fn,
        ),
    ]
}

fn eval_node(ctx: &CallCtx<'_>, node: &FormulaAst) -> Value {
    ctx.eval
        .map(|eval| eval(node))
        .unwrap_or_else(|| Value::error(ErrorCode::Value))
}

fn eval_in(ctx: &CallCtx<'_>, node: &FormulaAst, scope: &HashMap<String, Value>) -> Value {
    ctx.eval_in
        .map(|eval| eval(node, scope))
        .unwrap_or_else(|| Value::error(ErrorCode::Value))
}

fn call_lambda(ctx: &CallCtx<'_>, lambda: &LambdaValue, args: &[Value]) -> Value {
    ctx.call_lambda
        .map(|call| call(lambda, args))
        .unwrap_or_else(|| Value::error(ErrorCode::Value))
}

fn deref(ctx: &CallCtx<'_>, value: Value) -> Fx<Value> {
    let Value::Ref(reference) = &value else {
        return Ok(value);
    };
    if reference.is_cell() {
        let Some(area) = reference.areas.first() else {
            return Ok(Value::blank());
        };
        return Ok(Value::Scalar(ctx.read_cell(
            area.sheet,
            area.start_row,
            area.start_col,
        )));
    }
    Ok(Value::Matrix(ctx.to_matrix(&value)?))
}

fn as_matrix(value: Value) -> Matrix {
    match value {
        Value::Matrix(matrix) => matrix,
        Value::Scalar(scalar) => Matrix::from_rows(vec![vec![scalar]]),
        Value::Ref(_) | Value::Lambda(_) => {
            Matrix::from_rows(vec![vec![Scalar::Error(ErrorCode::Value)]])
        }
    }
}

fn local_name(node: &FormulaAst) -> Fx<String> {
    match node {
        FormulaAst::Name { prefix: None, name } => Ok(normalize_local_name(name)),
        _ => Err(ErrorCode::Value),
    }
}

fn if_fn(ctx: &CallCtx<'_>, args: &[FormulaAst]) -> Fx<Value> {
    let cond = deref(ctx, eval_node(ctx, &args[0]))?;
    let branch = |truthy: bool| -> Fx<Value> {
        let index = if truthy { 1 } else { 2 };
        match args.get(index) {
            Some(node) => Ok(eval_node(ctx, node)),
            None => Ok(Value::Scalar(Scalar::Bool(truthy))),
        }
    };
    if let Value::Matrix(cond) = cond {
        let then_m = as_matrix(deref(ctx, branch(true)?)?);
        let else_m = as_matrix(deref(ctx, branch(false)?)?);
        let matrix = broadcast_shape(&[cond.clone(), then_m.clone(), else_m.clone()], |r, c| {
            let value = pick(&cond, r, c);
            if let Scalar::Error(code) = value {
                return Scalar::Error(code);
            }
            match to_bool(&value) {
                Ok(true) => pick(&then_m, r, c),
                Ok(false) => pick(&else_m, r, c),
                Err(code) => Scalar::Error(code),
            }
        })?;
        return Ok(Value::Matrix(matrix));
    }
    if let Value::Scalar(Scalar::Error(code)) = cond {
        return Ok(Value::error(code));
    }
    if matches!(cond, Value::Lambda(_)) {
        return Err(ErrorCode::Value);
    }
    let truthy = to_bool(&ctx.to_scalar(&cond))?;
    branch(truthy)
}

fn matches_error(value: &Value, only_na: bool) -> bool {
    match value {
        Value::Scalar(Scalar::Error(ErrorCode::Na)) => true,
        Value::Scalar(Scalar::Error(_)) => !only_na,
        _ => false,
    }
}

fn if_error(ctx: &CallCtx<'_>, args: &[FormulaAst], only_na: bool) -> Fx<Value> {
    let value = deref(ctx, eval_node(ctx, &args[0]))?;
    let fallback = |ctx: &CallCtx<'_>| -> Fx<Value> {
        let value = deref(ctx, eval_node(ctx, &args[1]))?;
        if matches!(value, Value::Scalar(Scalar::Blank)) {
            Ok(Value::number(0.0))
        } else {
            Ok(value)
        }
    };
    if let Value::Matrix(matrix) = value {
        let mut alt: Option<Scalar> = None;
        let mapped = matrix.map(|cell, _, _| {
            if matches_error(&Value::Scalar(cell.clone()), only_na) {
                if alt.is_none() {
                    alt = Some(match fallback(ctx) {
                        Ok(value) => first_scalar(value),
                        Err(code) => Scalar::Error(code),
                    });
                }
                alt.clone().unwrap_or(Scalar::Number(0.0))
            } else {
                cell.clone()
            }
        })?;
        return Ok(Value::Matrix(mapped));
    }
    if matches_error(&value, only_na) {
        fallback(ctx)
    } else {
        Ok(value)
    }
}

fn first_scalar(value: Value) -> Scalar {
    match value {
        Value::Matrix(matrix) => matrix.get(0, 0),
        Value::Scalar(Scalar::Blank) => Scalar::Number(0.0),
        Value::Scalar(scalar) => scalar,
        Value::Ref(_) | Value::Lambda(_) => Scalar::Error(ErrorCode::Value),
    }
}

fn ifs_fn(ctx: &CallCtx<'_>, args: &[FormulaAst]) -> Fx<Value> {
    if !args.len().is_multiple_of(2) {
        return Err(ErrorCode::Value);
    }
    let mut index = 0;
    while index < args.len() {
        let cond = deref(ctx, eval_node(ctx, &args[index]))?;
        let cond = ctx.to_scalar(&cond);
        if let Scalar::Error(code) = cond {
            return Ok(Value::error(code));
        }
        if to_bool(&cond)? {
            return Ok(eval_node(ctx, &args[index + 1]));
        }
        index += 2;
    }
    Ok(Value::error(ErrorCode::Na))
}

fn switch_fn(ctx: &CallCtx<'_>, args: &[FormulaAst]) -> Fx<Value> {
    let target = ctx.to_scalar(&deref(ctx, eval_node(ctx, &args[0]))?);
    if let Scalar::Error(code) = target {
        return Ok(Value::error(code));
    }
    let mut index = 1;
    while index + 1 < args.len() {
        let candidate = ctx.to_scalar(&deref(ctx, eval_node(ctx, &args[index]))?);
        if let Scalar::Error(code) = candidate {
            return Ok(Value::error(code));
        }
        if same_type(&candidate, &target)
            && super::coerce::compare_scalars(&candidate, &target).ok() == Some(0)
        {
            return Ok(eval_node(ctx, &args[index + 1]));
        }
        index += 2;
    }
    if index < args.len() {
        Ok(eval_node(ctx, &args[index]))
    } else {
        Ok(Value::error(ErrorCode::Na))
    }
}

fn same_type(a: &Scalar, b: &Scalar) -> bool {
    matches!(
        (a, b),
        (Scalar::Number(_), Scalar::Number(_))
            | (Scalar::Text(_), Scalar::Text(_))
            | (Scalar::Bool(_), Scalar::Bool(_))
            | (Scalar::Blank, Scalar::Blank)
            | (Scalar::Error(_), Scalar::Error(_))
    )
}

enum LogicalFold {
    And,
    Or,
    Xor,
}

fn logical_fold(ctx: &CallCtx<'_>, args: &[FormulaAst], fold: LogicalFold) -> Fx<Value> {
    let mut bits = Vec::new();
    for arg in args {
        let value = eval_node(ctx, arg);
        let mut error = None;
        ctx.for_each(&value, &mut |scalar, kind| {
            if error.is_some() {
                return;
            }
            if let Scalar::Error(code) = scalar {
                error = Some(*code);
                return;
            }
            match scalar {
                Scalar::Bool(bit) => bits.push(*bit),
                Scalar::Number(n) => bits.push(*n != 0.0),
                Scalar::Text(_) if kind == super::value::VisitKind::Direct => {
                    match to_bool(scalar) {
                        Ok(bit) => bits.push(bit),
                        Err(code) => error = Some(code),
                    }
                }
                Scalar::Blank if kind == super::value::VisitKind::Direct => bits.push(false),
                _ => {}
            }
        });
        if let Some(code) = error {
            return Err(code);
        }
    }
    if bits.is_empty() {
        return Err(ErrorCode::Value);
    }
    let result = match fold {
        LogicalFold::And => bits.iter().all(|bit| *bit),
        LogicalFold::Or => bits.iter().any(|bit| *bit),
        LogicalFold::Xor => bits.iter().filter(|bit| **bit).count() % 2 == 1,
    };
    Ok(Value::Scalar(Scalar::Bool(result)))
}

fn not_fn(_ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let scalar = super::helpers::scalar_of(args.first().unwrap_or(&Value::blank()))?;
    Ok(Value::Scalar(Scalar::Bool(!to_bool(&scalar)?)))
}

fn let_fn(ctx: &CallCtx<'_>, args: &[FormulaAst]) -> Fx<Value> {
    if args.len().is_multiple_of(2) {
        return Err(ErrorCode::Value);
    }
    let mut scope = ctx.scope.clone();
    let mut index = 0;
    while index + 1 < args.len() {
        let name = local_name(&args[index])?;
        let value = eval_in(ctx, &args[index + 1], &scope);
        scope.insert(name, value);
        index += 2;
    }
    Ok(eval_in(ctx, &args[args.len() - 1], &scope))
}

fn lambda_fn(ctx: &CallCtx<'_>, args: &[FormulaAst]) -> Fx<Value> {
    let mut params = Vec::new();
    for arg in &args[..args.len() - 1] {
        params.push(local_name(arg)?);
    }
    Ok(Value::Lambda(Box::new(LambdaValue {
        params,
        body: args[args.len() - 1].clone(),
        scope: ctx.scope.clone(),
    })))
}

fn lambda_of(value: &Value) -> Fx<LambdaValue> {
    match value {
        Value::Lambda(lambda) => Ok((**lambda).clone()),
        _ => Err(ErrorCode::Value),
    }
}

/// The scalar a lambda result contributes to an array.
fn element(ctx: &CallCtx<'_>, value: Value) -> Scalar {
    match value {
        Value::Matrix(matrix) => {
            if matrix.rows() != 1 || matrix.cols() != 1 {
                Scalar::Error(ErrorCode::Calc)
            } else {
                matrix.get(0, 0)
            }
        }
        Value::Ref(reference) => {
            if reference.is_cell() {
                ctx.to_scalar(&Value::Ref(reference))
            } else {
                Scalar::Error(ErrorCode::Calc)
            }
        }
        Value::Lambda(_) => Scalar::Error(ErrorCode::Calc),
        Value::Scalar(Scalar::Blank) => Scalar::Number(0.0),
        Value::Scalar(scalar) => scalar,
    }
}

fn map_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let lambda = lambda_of(args.last().unwrap_or(&Value::blank()))?;
    let arrays: Vec<Matrix> = args[..args.len() - 1]
        .iter()
        .map(|arg| ctx.to_matrix(arg))
        .collect::<Fx<_>>()?;
    let rows = arrays.iter().map(Matrix::rows).max().unwrap_or(0);
    let cols = arrays.iter().map(Matrix::cols).max().unwrap_or(0);
    let matrix = Matrix::build(rows, cols, |r, c| {
        let values: Vec<Value> = arrays
            .iter()
            .map(|matrix| Value::Scalar(pick(matrix, r, c)))
            .collect();
        Ok(element(ctx, call_lambda(ctx, &lambda, &values)))
    })?;
    Ok(Value::Matrix(matrix))
}

fn reduce_inputs(args: &[Value]) -> (Value, Value) {
    if args.len() == 3 {
        (args[1].clone(), args[0].clone())
    } else {
        (args[0].clone(), Value::blank())
    }
}

fn reduce_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let lambda = lambda_of(args.last().unwrap_or(&Value::blank()))?;
    let (array_value, initial) = reduce_inputs(args);
    let array = ctx.to_matrix(&array_value)?;
    let mut acc = initial;
    for value in array.flat()? {
        acc = call_lambda(ctx, &lambda, &[acc, Value::Scalar(value)]);
    }
    Ok(acc)
}

fn scan_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let lambda = lambda_of(args.last().unwrap_or(&Value::blank()))?;
    let (array_value, initial) = reduce_inputs(args);
    let array = ctx.to_matrix(&array_value)?;
    let data = array.data()?;
    let mut acc = initial;
    let mut rows = Vec::with_capacity(data.len());
    for line in data {
        let mut out = Vec::with_capacity(line.len());
        for value in line {
            acc = call_lambda(ctx, &lambda, &[acc.clone(), Value::Scalar(value)]);
            out.push(element(ctx, acc.clone()));
        }
        rows.push(out);
    }
    Ok(Value::Matrix(Matrix::from_rows(rows)))
}

fn byrow_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let lambda = lambda_of(args.get(1).unwrap_or(&Value::blank()))?;
    let array = ctx.to_matrix(args.first().unwrap_or(&Value::blank()))?;
    let data = array.data()?;
    let mut rows = Vec::with_capacity(data.len());
    for row in data {
        let matrix = Matrix::from_rows(vec![row]);
        let value = element(ctx, call_lambda(ctx, &lambda, &[Value::Matrix(matrix)]));
        rows.push(vec![value]);
    }
    Ok(Value::Matrix(Matrix::from_rows(rows)))
}

fn bycol_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let lambda = lambda_of(args.get(1).unwrap_or(&Value::blank()))?;
    let array = ctx.to_matrix(args.first().unwrap_or(&Value::blank()))?;
    let data = array.data()?;
    let cols = data.first().map(Vec::len).unwrap_or(0);
    let mut out = Vec::with_capacity(cols);
    for col in 0..cols {
        let column: Vec<Vec<Scalar>> = data
            .iter()
            .map(|row| vec![row.get(col).cloned().unwrap_or(Scalar::Blank)])
            .collect();
        let value = element(
            ctx,
            call_lambda(ctx, &lambda, &[Value::Matrix(Matrix::from_rows(column))]),
        );
        out.push(value);
    }
    Ok(Value::Matrix(Matrix::from_rows(vec![out])))
}

fn makearray_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let rows = super::helpers::int_of(ctx, args.first().unwrap_or(&Value::blank()))?;
    let cols = super::helpers::int_of(ctx, args.get(1).unwrap_or(&Value::blank()))?;
    if rows < 1.0 || cols < 1.0 || !rows.is_finite() || !cols.is_finite() {
        return Err(ErrorCode::Value);
    }
    let lambda = lambda_of(args.get(2).unwrap_or(&Value::blank()))?;
    let matrix = Matrix::build(rows as usize, cols as usize, |r, c| {
        Ok(element(
            ctx,
            call_lambda(
                ctx,
                &lambda,
                &[Value::number(r as f64 + 1.0), Value::number(c as f64 + 1.0)],
            ),
        ))
    })?;
    Ok(Value::Matrix(matrix))
}
