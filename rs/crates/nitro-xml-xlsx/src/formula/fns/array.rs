// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/array.ts
// and array-shape.ts; changes listed in PROVENANCE.md.
//! Dynamic-array functions registered under Lookup & Reference: FILTER, SORT, SEQUENCE
//! and the shape helpers TAKE through EXPAND.
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]

use std::cmp::Ordering;
use std::collections::HashMap;

use super::coerce::{compare_scalars, to_bool};
use super::helpers::num;
use super::host::CallCtx;
use super::spec::{gen_spec, FunctionSpec, ParamKind};
use super::value::{ErrorCode, Fx, Matrix, Scalar, Value, MAX_ARRAY_CELLS, MAX_ARRAY_SIDE};

const C: &str = "Lookup & Reference";
const ANY: &[ParamKind] = &[ParamKind::Any];

fn is_blank(value: &Value) -> bool {
    matches!(value, Value::Scalar(Scalar::Blank))
}

fn present_num(ctx: &CallCtx<'_>, args: &[Value], index: usize) -> Fx<Option<f64>> {
    if index < args.len() && !is_blank(&args[index]) {
        Ok(Some(num(ctx, &Value::Scalar(ctx.to_scalar(&args[index])))?))
    } else {
        Ok(None)
    }
}

fn opt_int(ctx: &CallCtx<'_>, args: &[Value], index: usize, fallback: f64) -> Fx<f64> {
    Ok(present_num(ctx, args, index)?.map_or(fallback, |n| n.trunc()))
}

fn int_at(ctx: &CallCtx<'_>, args: &[Value], index: usize) -> Fx<Option<i64>> {
    Ok(present_num(ctx, args, index)?.map(|n| n.trunc() as i64))
}

fn truthy(value: &Scalar) -> Fx<bool> {
    if let Scalar::Error(code) = value {
        return Err(*code);
    }
    if matches!(value, Scalar::Blank) {
        return Ok(false);
    }
    to_bool(value)
}

fn matrix_fill(matrix: &Matrix) -> Scalar {
    if !matrix.is_padded() {
        return Scalar::Blank;
    }
    if matrix.block_rows() < matrix.rows() {
        return matrix.get(matrix.block_rows(), 0);
    }
    matrix.get(0, matrix.block_cols())
}

fn transpose(matrix: &Matrix) -> Fx<Matrix> {
    Matrix::build(matrix.cols(), matrix.rows(), |row, col| {
        Ok(matrix.get(col, row))
    })
}

fn sort_compare(a: &Scalar, b: &Scalar, descending: bool) -> Ordering {
    if matches!(a, Scalar::Blank) || matches!(b, Scalar::Blank) {
        return if matches!(a, Scalar::Blank) && matches!(b, Scalar::Blank) {
            Ordering::Equal
        } else if matches!(a, Scalar::Blank) {
            Ordering::Greater
        } else {
            Ordering::Less
        };
    }
    let a_err = a.is_error();
    let b_err = b.is_error();
    let order = if a_err || b_err {
        match (a_err, b_err) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => Ordering::Equal,
        }
    } else {
        match compare_scalars(a, b).unwrap_or(0) {
            0 => Ordering::Equal,
            n if n < 0 => Ordering::Less,
            _ => Ordering::Greater,
        }
    };
    if descending {
        order.reverse()
    } else {
        order
    }
}

struct SortKey {
    values: Vec<Scalar>,
    descending: bool,
}

fn sort_rows(rows: &[Vec<Scalar>], keys: &[SortKey]) -> Vec<Vec<Scalar>> {
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&left, &right| {
        for key in keys {
            let a = key.values.get(left).unwrap_or(&Scalar::Blank);
            let b = key.values.get(right).unwrap_or(&Scalar::Blank);
            let cmp = sort_compare(a, b, key.descending);
            if cmp != Ordering::Equal {
                return cmp;
            }
        }
        left.cmp(&right)
    });
    order.into_iter().map(|index| rows[index].clone()).collect()
}

fn fn_filter(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let array = ctx.to_matrix(&args[0])?;
    let include = ctx.to_matrix(&args[1])?;
    let out = if include.cols() == 1 && include.rows() == array.rows() {
        let fill = matrix_fill(&include);
        let limit = if include.is_padded() && !fill.is_error() && !truthy(&fill)? {
            include.block_rows()
        } else {
            include.rows()
        };
        let mut kept = Vec::new();
        for row in 0..limit {
            if !truthy(&include.get(row, 0))? {
                continue;
            }
            let mut line = Vec::with_capacity(array.cols());
            for col in 0..array.cols() {
                line.push(array.get(row, col));
            }
            kept.push(line);
        }
        Matrix::from_rows(kept)
    } else if include.rows() == 1 && include.cols() == array.cols() {
        let header = include.data()?;
        let mut keep = Vec::new();
        if let Some(row) = header.first() {
            for value in row {
                keep.push(truthy(value)?);
            }
        }
        let body = array.data()?;
        Matrix::from_rows(
            body.into_iter()
                .map(|row| {
                    row.into_iter()
                        .enumerate()
                        .filter(|(col, _)| keep.get(*col).copied().unwrap_or(false))
                        .map(|(_, value)| value)
                        .collect()
                })
                .collect(),
        )
    } else {
        return Err(ErrorCode::Value);
    };
    if out.rows() == 0 || out.cols() == 0 {
        // A blank `if_empty` is the same as omitting it: `#CALC!`.
        if args.len() > 2 && !is_blank(&args[2]) {
            return Ok(args[2].clone());
        }
        return Err(ErrorCode::Calc);
    }
    Ok(Value::Matrix(out))
}

fn fn_sort(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let mut matrix = ctx.to_matrix(&args[0])?;
    let by_col = args.len() > 3 && !is_blank(&args[3]) && to_bool(&ctx.to_scalar(&args[3]))?;
    if by_col {
        matrix = transpose(&matrix)?;
    }
    let indexes = if args.len() > 1 && !is_blank(&args[1]) {
        ctx.to_matrix(&args[1])?
            .flat()?
            .into_iter()
            .map(|value| num(ctx, &Value::Scalar(value)).map(|n| n.trunc() as i64))
            .collect::<Fx<Vec<_>>>()?
    } else {
        vec![1]
    };
    let orders = if args.len() > 2 && !is_blank(&args[2]) {
        ctx.to_matrix(&args[2])?
            .flat()?
            .into_iter()
            .map(|value| num(ctx, &Value::Scalar(value)))
            .collect::<Fx<Vec<_>>>()?
    } else {
        vec![1.0]
    };
    let rows = matrix.data()?;
    let mut keys = Vec::new();
    for (k, index) in indexes.into_iter().enumerate() {
        if index < 1 || index as usize > matrix.cols() {
            return Err(ErrorCode::Value);
        }
        let order = if orders.is_empty() {
            1.0
        } else {
            orders[k.min(orders.len() - 1)]
        };
        if order != 1.0 && order != -1.0 {
            return Err(ErrorCode::Value);
        }
        keys.push(SortKey {
            values: rows
                .iter()
                .map(|row| row[index as usize - 1].clone())
                .collect(),
            descending: order == -1.0,
        });
    }
    let sorted = Matrix::from_rows(sort_rows(&rows, &keys));
    Ok(Value::Matrix(if by_col {
        transpose(&sorted)?
    } else {
        sorted
    }))
}

fn fn_sort_by(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let matrix = ctx.to_matrix(&args[0])?;
    let mut keys = Vec::new();
    let mut by_col: Option<bool> = None;
    let mut i = 1;
    while i < args.len() {
        let by = ctx.to_matrix(&args[i])?;
        let order = if i + 1 < args.len() && !is_blank(&args[i + 1]) {
            num(ctx, &Value::Scalar(ctx.to_scalar(&args[i + 1])))?
        } else {
            1.0
        };
        if order != 1.0 && order != -1.0 {
            return Err(ErrorCode::Value);
        }
        let column = by.cols() == 1 && by.rows() == matrix.rows();
        let row = by.rows() == 1 && by.cols() == matrix.cols();
        if !column && !row {
            return Err(ErrorCode::Value);
        }
        let this_by_col = !column;
        if let Some(previous) = by_col {
            if previous != this_by_col {
                return Err(ErrorCode::Value);
            }
        }
        by_col = Some(this_by_col);
        keys.push(SortKey {
            values: by.flat()?,
            descending: order == -1.0,
        });
        i += 2;
    }
    let rows = matrix.data()?;
    if by_col == Some(true) {
        let flipped = transpose(&matrix)?.data()?;
        let sorted = Matrix::from_rows(sort_rows(&flipped, &keys));
        return Ok(Value::Matrix(transpose(&sorted)?));
    }
    Ok(Value::Matrix(Matrix::from_rows(sort_rows(&rows, &keys))))
}

fn unique_key(row: &[Scalar]) -> String {
    let mut parts = Vec::with_capacity(row.len());
    for value in row {
        let piece = match value {
            Scalar::Error(code) => format!("\"#{}\"", code.as_excel()),
            Scalar::Text(text) => serde_json::to_string(&format!("s{}", text.to_lowercase()))
                .unwrap_or_else(|_| "\"\"".to_owned()),
            Scalar::Number(n) => {
                let n = if *n == 0.0 { 0.0 } else { *n };
                crate::number_format::js_number_string(n)
            }
            Scalar::Bool(true) => "true".to_owned(),
            Scalar::Bool(false) => "false".to_owned(),
            Scalar::Blank => "null".to_owned(),
        };
        parts.push(piece);
    }
    format!("[{}]", parts.join(","))
}

fn fn_unique(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let mut matrix = ctx.to_matrix(&args[0])?;
    let by_col = args.len() > 1 && !is_blank(&args[1]) && to_bool(&ctx.to_scalar(&args[1]))?;
    let once = args.len() > 2 && !is_blank(&args[2]) && to_bool(&ctx.to_scalar(&args[2]))?;
    if by_col {
        matrix = transpose(&matrix)?;
    }
    let mut counts: HashMap<String, (Vec<Scalar>, usize)> = HashMap::new();
    let mut order = Vec::new();
    for row in matrix.data()? {
        let key = unique_key(&row);
        if let Some(entry) = counts.get_mut(&key) {
            entry.1 += 1;
        } else {
            order.push(key.clone());
            counts.insert(key, (row, 1));
        }
    }
    let rows: Vec<Vec<Scalar>> = order
        .into_iter()
        .filter_map(|key| counts.remove(&key))
        .filter(|(_, count)| !once || *count == 1)
        .map(|(row, _)| row)
        .collect();
    if rows.is_empty() {
        return Err(ErrorCode::Calc);
    }
    let out = Matrix::from_rows(rows);
    Ok(Value::Matrix(if by_col { transpose(&out)? } else { out }))
}

fn fn_transpose(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let matrix = ctx.to_matrix(&args[0])?;
    let mapped = matrix.map(|value, _, _| {
        if matches!(value, Scalar::Blank) {
            Scalar::Number(0.0)
        } else {
            value.clone()
        }
    })?;
    Ok(Value::Matrix(transpose(&mapped)?))
}

fn fn_sequence(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let rows = opt_int(ctx, args, 0, 1.0)?;
    let cols = opt_int(ctx, args, 1, 1.0)?;
    let start = present_num(ctx, args, 2)?.unwrap_or(1.0);
    let step = present_num(ctx, args, 3)?.unwrap_or(1.0);
    let (rows, cols) = checked_dims(rows, cols)?;
    let matrix = Matrix::build(rows, cols, |row, col| {
        Ok(Scalar::Number(start + (row * cols + col) as f64 * step))
    })?;
    Ok(Value::Matrix(matrix))
}

fn fn_rand_array(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let rows = opt_int(ctx, args, 0, 1.0)?;
    let cols = opt_int(ctx, args, 1, 1.0)?;
    let min = present_num(ctx, args, 2)?.unwrap_or(0.0);
    let max = present_num(ctx, args, 3)?.unwrap_or(1.0);
    let integer = args.len() > 4 && !is_blank(&args[4]) && to_bool(&ctx.to_scalar(&args[4]))?;
    let (rows, cols) = checked_dims(rows, cols)?;
    if min > max {
        return Err(ErrorCode::Value);
    }
    let matrix = Matrix::build(rows, cols, |_, _| {
        let random = ctx.random();
        let value = if integer {
            let lo = min.ceil();
            let hi = max.floor();
            lo + (random * (hi - lo + 1.0)).floor()
        } else {
            min + random * (max - min)
        };
        Ok(Scalar::Number(value))
    })?;
    Ok(Value::Matrix(matrix))
}

/// `#CALC!` for a non-positive size, `#NUM!` past the cell budget, `#VALUE!` past the side limit.
fn checked_dims(rows: f64, cols: f64) -> Fx<(usize, usize)> {
    if rows < 1.0 || cols < 1.0 {
        return Err(ErrorCode::Calc);
    }
    if !rows.is_finite() || !cols.is_finite() || rows * cols > MAX_ARRAY_CELLS as f64 {
        return Err(ErrorCode::Num);
    }
    if rows > MAX_ARRAY_SIDE as f64 || cols > MAX_ARRAY_SIDE as f64 {
        return Err(ErrorCode::Value);
    }
    Ok((rows as usize, cols as usize))
}

fn take_slice(length: usize, count: Option<i64>, drop: bool) -> (usize, usize) {
    let Some(count) = count else {
        return (0, length);
    };
    let count = count as isize;
    let length = length as isize;
    if drop {
        if count >= 0 {
            (count.min(length).max(0) as usize, length.max(0) as usize)
        } else {
            (0, (length + count).max(0) as usize)
        }
    } else if count >= 0 {
        (0, count.min(length).max(0) as usize)
    } else {
        ((length + count).max(0) as usize, length.max(0) as usize)
    }
}

fn take_matrix(matrix: &Matrix, rows: Option<i64>, cols: Option<i64>, drop: bool) -> Fx<Value> {
    if !drop && (rows == Some(0) || cols == Some(0)) {
        return Err(ErrorCode::Calc);
    }
    let (r1, r2) = take_slice(matrix.rows(), rows, drop);
    let (c1, c2) = take_slice(matrix.cols(), cols, drop);
    if r2 <= r1 || c2 <= c1 {
        return Err(ErrorCode::Calc);
    }
    let body = matrix.data()?;
    let rows = body
        .into_iter()
        .skip(r1)
        .take(r2 - r1)
        .map(|row| row.into_iter().skip(c1).take(c2 - c1).collect())
        .collect();
    Ok(Value::Matrix(Matrix::from_rows(rows)))
}

fn fn_take(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    take_matrix(
        &ctx.to_matrix(&args[0])?,
        int_at(ctx, args, 1)?,
        int_at(ctx, args, 2)?,
        false,
    )
}

fn fn_drop(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    take_matrix(
        &ctx.to_matrix(&args[0])?,
        int_at(ctx, args, 1)?,
        int_at(ctx, args, 2)?,
        true,
    )
}

fn choose_indexes(indexes: &[i64], length: usize) -> Fx<Vec<usize>> {
    indexes
        .iter()
        .map(|index| {
            let at = if *index < 0 {
                length as i64 + index
            } else {
                index - 1
            };
            if *index == 0 || at < 0 || at as usize >= length {
                Err(ErrorCode::Value)
            } else {
                Ok(at as usize)
            }
        })
        .collect()
}

fn index_list(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Vec<i64>> {
    let mut out = Vec::new();
    for arg in args.iter().skip(1) {
        for value in ctx.to_matrix(arg)?.flat()? {
            out.push(num(ctx, &Value::Scalar(value))?.trunc() as i64);
        }
    }
    Ok(out)
}

fn fn_choose_rows(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let matrix = ctx.to_matrix(&args[0])?;
    let body = matrix.data()?;
    let rows = choose_indexes(&index_list(ctx, args)?, matrix.rows())?;
    Ok(Value::Matrix(Matrix::from_rows(
        rows.into_iter().map(|row| body[row].clone()).collect(),
    )))
}

fn fn_choose_cols(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let matrix = ctx.to_matrix(&args[0])?;
    let cols = choose_indexes(&index_list(ctx, args)?, matrix.cols())?;
    let body = matrix.data()?;
    Ok(Value::Matrix(Matrix::from_rows(
        body.into_iter()
            .map(|row| cols.iter().map(|col| row[*col].clone()).collect())
            .collect(),
    )))
}

fn stack(ctx: &CallCtx<'_>, args: &[Value], vertical: bool) -> Fx<Value> {
    let parts: Vec<Matrix> = args
        .iter()
        .map(|arg| ctx.to_matrix(arg))
        .collect::<Fx<_>>()?;
    if vertical {
        let width = parts.iter().map(Matrix::cols).max().unwrap_or(0);
        let mut rows = Vec::new();
        for part in &parts {
            for row in part.data()? {
                let mut line = Vec::with_capacity(width);
                for col in 0..width {
                    line.push(if col < row.len() {
                        row[col].clone()
                    } else {
                        Scalar::Error(ErrorCode::Na)
                    });
                }
                rows.push(line);
            }
        }
        return Ok(Value::Matrix(Matrix::from_rows(rows)));
    }
    let height = parts.iter().map(Matrix::rows).max().unwrap_or(0);
    let width: usize = parts.iter().map(Matrix::cols).sum();
    let matrix = Matrix::build(height, width, |row, mut col| {
        for part in &parts {
            if col < part.cols() {
                return Ok(if row < part.rows() {
                    part.get(row, col)
                } else {
                    Scalar::Error(ErrorCode::Na)
                });
            }
            col -= part.cols();
        }
        Ok(Scalar::Error(ErrorCode::Na))
    })?;
    Ok(Value::Matrix(matrix))
}

fn fn_vstack(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    stack(ctx, args, true)
}

fn fn_hstack(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    stack(ctx, args, false)
}

fn flatten(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Vec<Scalar>> {
    let matrix = ctx.to_matrix(&args[0])?;
    let ignore = int_at(ctx, args, 1)?.unwrap_or(0);
    let by_col = int_at(ctx, args, 2)?.unwrap_or(0) != 0;
    let mut values = Vec::new();
    let major = if by_col { matrix.cols() } else { matrix.rows() };
    let minor = if by_col { matrix.rows() } else { matrix.cols() };
    for outer in 0..major {
        for inner in 0..minor {
            let value = if by_col {
                matrix.get(inner, outer)
            } else {
                matrix.get(outer, inner)
            };
            if (ignore == 1 || ignore == 3) && matches!(value, Scalar::Blank) {
                continue;
            }
            if (ignore == 2 || ignore == 3) && value.is_error() {
                continue;
            }
            values.push(value);
        }
    }
    Ok(values)
}

fn fn_tocol(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = flatten(ctx, args)?;
    if values.is_empty() {
        return Err(ErrorCode::Calc);
    }
    Ok(Value::Matrix(Matrix::from_rows(
        values.into_iter().map(|value| vec![value]).collect(),
    )))
}

fn fn_torow(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let values = flatten(ctx, args)?;
    if values.is_empty() {
        return Err(ErrorCode::Calc);
    }
    Ok(Value::Matrix(Matrix::from_rows(vec![values])))
}

fn wrap(ctx: &CallCtx<'_>, args: &[Value], by_rows: bool) -> Fx<Value> {
    let values = ctx.to_matrix(&args[0])?;
    if values.rows() != 1 && values.cols() != 1 {
        return Err(ErrorCode::Value);
    }
    let flat = values.flat()?;
    let count = int_at(ctx, args, 1)?.unwrap_or(0);
    if count < 1 {
        return Err(ErrorCode::Num);
    }
    let pad = if args.len() > 2 {
        ctx.to_scalar(&args[2])
    } else {
        Scalar::Error(ErrorCode::Na)
    };
    let count = count as usize;
    let other = flat.len().div_ceil(count);
    let matrix = if by_rows {
        Matrix::build(other, count, |row, col| {
            Ok(flat
                .get(row * count + col)
                .cloned()
                .unwrap_or_else(|| pad.clone()))
        })?
    } else {
        Matrix::build(count, other, |row, col| {
            Ok(flat
                .get(col * count + row)
                .cloned()
                .unwrap_or_else(|| pad.clone()))
        })?
    };
    Ok(Value::Matrix(matrix))
}

fn fn_wrap_rows(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    wrap(ctx, args, true)
}

fn fn_wrap_cols(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    wrap(ctx, args, false)
}

fn fn_expand(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let matrix = ctx.to_matrix(&args[0])?;
    let rows = int_at(ctx, args, 1)?.unwrap_or(matrix.rows() as i64);
    let cols = int_at(ctx, args, 2)?.unwrap_or(matrix.cols() as i64);
    if rows < matrix.rows() as i64 || cols < matrix.cols() as i64 {
        return Err(ErrorCode::Value);
    }
    if rows > MAX_ARRAY_SIDE as i64 || cols > MAX_ARRAY_SIDE as i64 {
        return Err(ErrorCode::Num);
    }
    let pad = if args.len() > 3 {
        ctx.to_scalar(&args[3])
    } else {
        Scalar::Error(ErrorCode::Na)
    };
    let out = Matrix::build(rows as usize, cols as usize, |row, col| {
        Ok(if row < matrix.rows() && col < matrix.cols() {
            matrix.get(row, col)
        } else {
            pad.clone()
        })
    })?;
    Ok(Value::Matrix(out))
}

pub(super) fn specs() -> Vec<FunctionSpec> {
    vec![
        gen_spec(
            "TAKE",
            C,
            "TAKE(array, rows, [columns])",
            "Rows or columns from the start or end of an array.",
            2,
            3,
            ANY,
            false,
            fn_take,
        ),
        gen_spec(
            "DROP",
            C,
            "DROP(array, rows, [columns])",
            "An array without rows or columns at the start or end.",
            2,
            3,
            ANY,
            false,
            fn_drop,
        ),
        gen_spec(
            "CHOOSEROWS",
            C,
            "CHOOSEROWS(array, row_num1, ...)",
            "The given rows of an array.",
            2,
            255,
            ANY,
            false,
            fn_choose_rows,
        ),
        gen_spec(
            "CHOOSECOLS",
            C,
            "CHOOSECOLS(array, col_num1, ...)",
            "The given columns of an array.",
            2,
            255,
            ANY,
            false,
            fn_choose_cols,
        ),
        gen_spec(
            "VSTACK",
            C,
            "VSTACK(array1, [array2], ...)",
            "Stacks arrays vertically.",
            1,
            254,
            ANY,
            false,
            fn_vstack,
        ),
        gen_spec(
            "HSTACK",
            C,
            "HSTACK(array1, [array2], ...)",
            "Stacks arrays horizontally.",
            1,
            254,
            ANY,
            false,
            fn_hstack,
        ),
        gen_spec(
            "TOCOL",
            C,
            "TOCOL(array, [ignore], [scan_by_column])",
            "An array as one column.",
            1,
            3,
            ANY,
            false,
            fn_tocol,
        ),
        gen_spec(
            "TOROW",
            C,
            "TOROW(array, [ignore], [scan_by_column])",
            "An array as one row.",
            1,
            3,
            ANY,
            false,
            fn_torow,
        ),
        gen_spec(
            "WRAPROWS",
            C,
            "WRAPROWS(vector, wrap_count, [pad_with])",
            "Wraps a vector into rows.",
            2,
            3,
            ANY,
            false,
            fn_wrap_rows,
        ),
        gen_spec(
            "WRAPCOLS",
            C,
            "WRAPCOLS(vector, wrap_count, [pad_with])",
            "Wraps a vector into columns.",
            2,
            3,
            ANY,
            false,
            fn_wrap_cols,
        ),
        gen_spec(
            "EXPAND",
            C,
            "EXPAND(array, rows, [columns], [pad_with])",
            "Expands an array to given dimensions.",
            2,
            4,
            ANY,
            false,
            fn_expand,
        ),
        gen_spec(
            "FILTER",
            C,
            "FILTER(array, include, [if_empty])",
            "Filters an array by a Boolean array.",
            2,
            3,
            ANY,
            false,
            fn_filter,
        ),
        gen_spec(
            "SORT",
            C,
            "SORT(array, [sort_index], [sort_order], [by_col])",
            "Sorts the contents of an array.",
            1,
            4,
            ANY,
            false,
            fn_sort,
        ),
        gen_spec(
            "SORTBY",
            C,
            "SORTBY(array, by_array1, [sort_order1], ...)",
            "Sorts an array by other arrays.",
            2,
            255,
            ANY,
            false,
            fn_sort_by,
        ),
        gen_spec(
            "UNIQUE",
            C,
            "UNIQUE(array, [by_col], [exactly_once])",
            "The unique rows or columns of an array.",
            1,
            3,
            ANY,
            false,
            fn_unique,
        ),
        gen_spec(
            "TRANSPOSE",
            C,
            "TRANSPOSE(array)",
            "Swaps the rows and columns of an array.",
            1,
            1,
            ANY,
            false,
            fn_transpose,
        ),
        gen_spec(
            "SEQUENCE",
            C,
            "SEQUENCE(rows, [columns], [start], [step])",
            "An array of sequential numbers.",
            1,
            4,
            ANY,
            false,
            fn_sequence,
        ),
        gen_spec(
            "RANDARRAY",
            C,
            "RANDARRAY([rows], [columns], [min], [max], [integer])",
            "An array of random numbers.",
            0,
            5,
            ANY,
            true,
            fn_rand_array,
        ),
    ]
}
