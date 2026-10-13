// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/lookup.ts,
// lookup-core.ts and reference.ts, plus address-r1c1.ts (`parseR1C1Range`) and
// `quoteSheetName` in address.ts; changes listed in PROVENANCE.md.
//! Lookup & Reference functions: MATCH through CHOOSE, and the reference functions
//! ADDRESS through FORMULATEXT.
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]

use std::sync::OnceLock;

use regex::Regex;

use crate::address::{column_label, parse_address, MAX_COL, MAX_ROW};
use crate::formula::{parse_formula, BinaryOp, FormulaAst};

use super::coerce::compare_scalars;
use super::helpers::{bool_of, has_wildcards, int_of, num, scalar_of, str_of, wildcard_regex};
use super::host::CallCtx;
use super::spec::{gen_spec, FunctionSpec, ParamKind};
use super::value::{Area, ErrorCode, Fx, Matrix, RefValue, Scalar, Value};

const C: &str = "Lookup & Reference";
const ANY: &[ParamKind] = &[ParamKind::Any];
const VALUE_ANY: &[ParamKind] = &[ParamKind::Value, ParamKind::Any];
const ANY_VALUE: &[ParamKind] = &[ParamKind::Any, ParamKind::Value];
const LOOKUP_PARAMS: &[ParamKind] = &[
    ParamKind::Value,
    ParamKind::Any,
    ParamKind::Value,
    ParamKind::Value,
];
const LOOKUP3: &[ParamKind] = &[ParamKind::Value, ParamKind::Any, ParamKind::Any];
const INDEX_PARAMS: &[ParamKind] = &[
    ParamKind::Any,
    ParamKind::Value,
    ParamKind::Value,
    ParamKind::Value,
];

struct Vector {
    source: Source,
    length: usize,
    logical: usize,
}

enum Source {
    Sheet {
        sheet: usize,
        origin_row: i32,
        origin_col: i32,
        vertical: bool,
    },
    Matrix {
        matrix: Matrix,
        index: usize,
        vertical: bool,
    },
}

impl Vector {
    fn get(&self, ctx: &CallCtx<'_>, index: usize) -> Scalar {
        if index >= self.length {
            return Scalar::Blank;
        }
        match &self.source {
            Source::Sheet {
                sheet,
                origin_row,
                origin_col,
                vertical,
            } => {
                if *vertical {
                    ctx.read_cell(*sheet, origin_row + index as i32, *origin_col)
                } else {
                    ctx.read_cell(*sheet, *origin_row, origin_col + index as i32)
                }
            }
            Source::Matrix {
                matrix,
                index: at,
                vertical,
            } => {
                if *vertical {
                    matrix.get(index, *at)
                } else {
                    matrix.get(*at, index)
                }
            }
        }
    }
}

fn shape_of(value: &Value) -> Fx<(usize, usize)> {
    match value {
        Value::Ref(reference) => {
            if reference.areas.len() != 1 {
                return Err(ErrorCode::Value);
            }
            let area = reference.areas[0];
            Ok((area.rows(), area.cols()))
        }
        Value::Matrix(matrix) => Ok((matrix.rows(), matrix.cols())),
        Value::Scalar(_) | Value::Lambda(_) => Ok((1, 1)),
    }
}

fn line_of(ctx: &CallCtx<'_>, value: &Value, vertical: bool, index: usize) -> Fx<Vector> {
    match value {
        Value::Ref(reference) => {
            if reference.areas.len() != 1 {
                return Err(ErrorCode::Value);
            }
            let area = reference.areas[0];
            let (bound_rows, bound_cols) = ctx.host.bounds(area.sheet);
            if vertical {
                let col = area.start_col + index as i32;
                let last = area.end_row.min(bound_rows.saturating_sub(1));
                let length = if last >= area.start_row {
                    (last - area.start_row + 1) as usize
                } else {
                    0
                };
                Ok(Vector {
                    source: Source::Sheet {
                        sheet: area.sheet,
                        origin_row: area.start_row,
                        origin_col: col,
                        vertical: true,
                    },
                    length,
                    logical: area.rows(),
                })
            } else {
                let row = area.start_row + index as i32;
                let last = area.end_col.min(bound_cols.saturating_sub(1));
                let length = if last >= area.start_col {
                    (last - area.start_col + 1) as usize
                } else {
                    0
                };
                Ok(Vector {
                    source: Source::Sheet {
                        sheet: area.sheet,
                        origin_row: row,
                        origin_col: area.start_col,
                        vertical: false,
                    },
                    length,
                    logical: area.cols(),
                })
            }
        }
        Value::Matrix(matrix) => {
            let length = if vertical {
                matrix.rows()
            } else {
                matrix.cols()
            };
            Ok(Vector {
                source: Source::Matrix {
                    matrix: matrix.clone(),
                    index,
                    vertical,
                },
                length,
                logical: length,
            })
        }
        Value::Scalar(scalar) => Ok(Vector {
            source: Source::Matrix {
                matrix: Matrix::from_rows(vec![vec![scalar.clone()]]),
                index,
                vertical,
            },
            length: 1,
            logical: 1,
        }),
        Value::Lambda(_) => Err(ErrorCode::Calc),
    }
}

fn vector_of(ctx: &CallCtx<'_>, value: &Value) -> Fx<(Vector, bool)> {
    let (rows, cols) = shape_of(value)?;
    if rows != 1 && cols != 1 {
        return Err(ErrorCode::Na);
    }
    if cols == 1 {
        Ok((line_of(ctx, value, true, 0)?, true))
    } else {
        Ok((line_of(ctx, value, false, 0)?, false))
    }
}

fn same_kind(a: &Scalar, b: &Scalar) -> bool {
    matches!(
        (a, b),
        (Scalar::Number(_), Scalar::Number(_))
            | (Scalar::Text(_), Scalar::Text(_))
            | (Scalar::Bool(_), Scalar::Bool(_))
    )
}

fn compare_lookup(a: &Scalar, b: &Scalar) -> i8 {
    if matches!(a, Scalar::Blank) {
        return if matches!(b, Scalar::Blank) { 0 } else { 1 };
    }
    if matches!(b, Scalar::Blank) {
        return -1;
    }
    compare_scalars(a, b).unwrap_or(0)
}

fn matches_exact(
    lookup: &Scalar,
    value: &Scalar,
    wildcards: bool,
    pattern: Option<&Regex>,
) -> bool {
    if let Scalar::Text(_) = lookup {
        if wildcards {
            if let (Some(regex), Scalar::Text(text)) = (pattern, value) {
                return regex.is_match(text);
            }
            if pattern.is_some() {
                return false;
            }
        }
    }
    if matches!(lookup, Scalar::Blank) {
        return matches!(value, Scalar::Blank);
    }
    !value.is_error()
        && !matches!(value, Scalar::Blank)
        && same_kind(value, lookup)
        && compare_scalars(value, lookup).ok() == Some(0)
}

fn exact_pattern(lookup: &Scalar, wildcards: bool) -> Option<Regex> {
    if wildcards {
        if let Scalar::Text(text) = lookup {
            if has_wildcards(text) {
                return Some(wildcard_regex(text));
            }
        }
    }
    None
}

fn find_exact(
    ctx: &CallCtx<'_>,
    vector: &Vector,
    lookup: &Scalar,
    wildcards: bool,
    reverse: bool,
) -> i64 {
    let pattern = exact_pattern(lookup, wildcards);
    let logical = vector.logical;
    if reverse {
        if matches!(lookup, Scalar::Blank) && logical > vector.length {
            return logical as i64 - 1;
        }
        for i in (0..vector.length).rev() {
            if matches_exact(lookup, &vector.get(ctx, i), wildcards, pattern.as_ref()) {
                return i as i64;
            }
        }
        return -1;
    }
    for i in 0..vector.length {
        if matches_exact(lookup, &vector.get(ctx, i), wildcards, pattern.as_ref()) {
            return i as i64;
        }
    }
    if matches!(lookup, Scalar::Blank) && logical > vector.length {
        return vector.length as i64;
    }
    -1
}

/// Last same-kind value that is <= the lookup. A probe on another type steps right.
fn find_ascending(ctx: &CallCtx<'_>, vector: &Vector, lookup: &Scalar) -> Fx<i64> {
    if let Scalar::Error(code) = lookup {
        return Err(*code);
    }
    let mut lo: i64 = 0;
    let mut hi: i64 = vector.length as i64 - 1;
    let mut best: i64 = -1;
    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        let mut at = mid;
        let mut value = if at >= 0 && (at as usize) < vector.length {
            vector.get(ctx, at as usize)
        } else {
            Scalar::Blank
        };
        while at <= hi && !same_kind(&value, lookup) {
            at += 1;
            value = if at <= hi && (at as usize) < vector.length {
                vector.get(ctx, at as usize)
            } else {
                Scalar::Blank
            };
        }
        if at > hi {
            hi = mid - 1;
            continue;
        }
        if compare_scalars(&value, lookup).unwrap_or(1) <= 0 {
            best = at;
            lo = at + 1;
        } else {
            hi = mid - 1;
        }
    }
    Ok(best)
}

fn find_descending(ctx: &CallCtx<'_>, vector: &Vector, lookup: &Scalar) -> Fx<i64> {
    if let Scalar::Error(code) = lookup {
        return Err(*code);
    }
    let mut best = -1i64;
    for i in 0..vector.length {
        let value = vector.get(ctx, i);
        if !same_kind(&value, lookup) {
            continue;
        }
        let cmp = compare_scalars(&value, lookup).unwrap_or(0);
        if cmp == 0 {
            return Ok(i as i64);
        }
        if cmp < 0 {
            break;
        }
        best = i as i64;
    }
    Ok(best)
}

fn find_sorted(ctx: &CallCtx<'_>, vector: &Vector, lookup: &Scalar, descending: bool) -> Fx<i64> {
    if let Scalar::Error(code) = lookup {
        return Err(*code);
    }
    let mut lo: i64 = 0;
    let mut hi: i64 = vector.logical as i64 - 1;
    let mut best: i64 = -1;
    let mut exact: i64 = -1;
    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        let mut probe = mid;
        let mut value = if mid >= 0 && (mid as usize) < vector.length {
            vector.get(ctx, mid as usize)
        } else {
            Scalar::Blank
        };
        while value.is_error() && probe > lo {
            probe -= 1;
            value = if probe >= 0 && (probe as usize) < vector.length {
                vector.get(ctx, probe as usize)
            } else {
                Scalar::Blank
            };
        }
        if value.is_error() {
            lo = mid + 1;
            continue;
        }
        let cmp = compare_lookup(&value, lookup);
        if cmp == 0 && !descending {
            exact = probe;
            hi = probe - 1;
            continue;
        }
        if if descending { cmp >= 0 } else { cmp <= 0 } {
            best = probe;
            lo = mid + 1;
        } else {
            hi = probe - 1;
        }
    }
    Ok(if exact >= 0 { exact } else { best })
}

fn find_nearest(
    ctx: &CallCtx<'_>,
    vector: &Vector,
    lookup: &Scalar,
    mode: i32,
    reverse: bool,
) -> i64 {
    let mut best: i64 = -1;
    let mut best_value = Scalar::Blank;
    let n = vector.length;
    let mut consider = |i: usize, value: &Scalar| -> bool {
        if value.is_error() {
            return false;
        }
        let cmp = compare_lookup(value, lookup);
        if cmp == 0 {
            best = i as i64;
            return true;
        }
        let candidate = if mode == -1 { cmp < 0 } else { cmp > 0 };
        if candidate
            && (best < 0
                || (mode == -1 && compare_lookup(value, &best_value) > 0)
                || (mode == 1 && compare_lookup(value, &best_value) < 0))
        {
            best = i as i64;
            best_value = value.clone();
        }
        false
    };
    let logical = vector.logical;
    if reverse && logical > n && consider(logical - 1, &Scalar::Blank) {
        return best;
    }
    for k in 0..n {
        let i = if reverse { n - 1 - k } else { k };
        if consider(i, &vector.get(ctx, i)) {
            return best;
        }
    }
    if !reverse && logical > n {
        let _ = consider(n, &Scalar::Blank);
    }
    best
}

fn xsearch(
    ctx: &CallCtx<'_>,
    vector: &Vector,
    lookup: &Scalar,
    match_mode: i32,
    search_mode: i32,
) -> Fx<i64> {
    if !matches!(match_mode, -1..=2) || !matches!(search_mode, 1 | -1 | 2 | -2) {
        return Err(ErrorCode::Value);
    }
    if search_mode == 2 || search_mode == -2 {
        if match_mode == 2 {
            return Err(ErrorCode::Value);
        }
        let descending = search_mode == -2;
        let at = find_sorted(ctx, vector, lookup, descending)?;
        let found = at >= 0 && matches_exact(lookup, &vector.get(ctx, at as usize), false, None);
        if match_mode == 0 {
            return Ok(if found { at } else { -1 });
        }
        if found {
            return Ok(at);
        }
        let length = vector.logical as i64;
        if match_mode == -1 {
            return Ok(if descending {
                if at + 1 < length {
                    at + 1
                } else {
                    -1
                }
            } else {
                at
            });
        }
        return Ok(if descending {
            at
        } else if at + 1 < length {
            at + 1
        } else {
            -1
        });
    }
    let reverse = search_mode == -1;
    if match_mode == 0 || match_mode == 2 {
        return Ok(find_exact(ctx, vector, lookup, match_mode == 2, reverse));
    }
    Ok(find_nearest(ctx, vector, lookup, match_mode, reverse))
}

fn is_blank(value: &Value) -> bool {
    matches!(value, Value::Scalar(Scalar::Blank))
}

fn lookup_value(value: &Value) -> Fx<Scalar> {
    let scalar = scalar_of(value)?;
    if let Scalar::Error(code) = scalar {
        return Err(code);
    }
    Ok(scalar)
}

fn blank_as_zero(value: &Value) -> Fx<Scalar> {
    let scalar = lookup_value(value)?;
    if matches!(scalar, Scalar::Blank) {
        Ok(Scalar::Number(0.0))
    } else {
        Ok(scalar)
    }
}

fn lookup_array(ctx: &CallCtx<'_>, value: &Value) -> Fx<Value> {
    match value {
        Value::Ref(_) | Value::Matrix(_) => Ok(value.clone()),
        other => {
            let _ = num(ctx, other)?;
            Err(ErrorCode::Na)
        }
    }
}

fn cell_of(value: &Value, row: usize, col: usize) -> Fx<Value> {
    match value {
        Value::Ref(reference) => {
            let area = reference.areas.first().ok_or(ErrorCode::Ref)?;
            let r = area.start_row + row as i32;
            let c = area.start_col + col as i32;
            Ok(Value::Ref(RefValue {
                areas: vec![Area {
                    sheet: area.sheet,
                    start_row: r,
                    end_row: r,
                    start_col: c,
                    end_col: c,
                }],
            }))
        }
        Value::Matrix(matrix) => Ok(Value::Scalar(matrix.get(row, col))),
        Value::Scalar(scalar) => Ok(Value::Scalar(scalar.clone())),
        Value::Lambda(_) => Err(ErrorCode::Calc),
    }
}

fn slice_of(value: &Value, vertical: bool, index: usize) -> Fx<Value> {
    match value {
        Value::Ref(reference) => {
            let area = reference.areas.first().ok_or(ErrorCode::Ref)?;
            let sliced = if vertical {
                let row = area.start_row + index as i32;
                Area {
                    sheet: area.sheet,
                    start_row: row,
                    end_row: row,
                    start_col: area.start_col,
                    end_col: area.end_col,
                }
            } else {
                let col = area.start_col + index as i32;
                Area {
                    sheet: area.sheet,
                    start_row: area.start_row,
                    end_row: area.end_row,
                    start_col: col,
                    end_col: col,
                }
            };
            Ok(Value::Ref(RefValue {
                areas: vec![sliced],
            }))
        }
        other => {
            let matrix = match other {
                Value::Matrix(matrix) => matrix.clone(),
                Value::Scalar(scalar) => Matrix::from_rows(vec![vec![scalar.clone()]]),
                Value::Lambda(_) => return Err(ErrorCode::Calc),
                Value::Ref(_) => unreachable!(),
            };
            if vertical {
                let row: Vec<Scalar> = (0..matrix.cols()).map(|c| matrix.get(index, c)).collect();
                Ok(Value::Matrix(Matrix::from_rows(vec![row])))
            } else {
                let rows: Vec<Vec<Scalar>> = (0..matrix.rows())
                    .map(|r| vec![matrix.get(r, index)])
                    .collect();
                Ok(Value::Matrix(Matrix::from_rows(rows)))
            }
        }
    }
}

fn vhlookup(ctx: &CallCtx<'_>, args: &[Value], vertical: bool) -> Fx<Value> {
    let lookup = blank_as_zero(&args[0])?;
    let table = lookup_array(ctx, args.get(1).unwrap_or(&Value::blank()))?;
    let index = int_of(ctx, &args[2])?;
    let approximate = if args.len() < 4 {
        true
    } else {
        bool_of(&args[3])?
    };
    let (rows, cols) = shape_of(&table)?;
    if index < 1.0 {
        return Err(ErrorCode::Value);
    }
    if index > if vertical { cols as f64 } else { rows as f64 } {
        return Err(ErrorCode::Ref);
    }
    let keys = line_of(ctx, &table, vertical, 0)?;
    let at = if approximate {
        find_ascending(ctx, &keys, &lookup)?
    } else {
        find_exact(ctx, &keys, &lookup, true, false)
    };
    if at < 0 {
        return Err(ErrorCode::Na);
    }
    if vertical {
        cell_of(&table, at as usize, index as usize - 1)
    } else {
        cell_of(&table, index as usize - 1, at as usize)
    }
}

fn fn_vlookup(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    vhlookup(ctx, args, true)
}

fn fn_hlookup(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    vhlookup(ctx, args, false)
}

fn fn_lookup(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let value = blank_as_zero(&args[0])?;
    let source = args.get(1).cloned().unwrap_or_else(Value::blank);
    let (rows, cols) = shape_of(&source)?;
    if args.len() > 2 {
        let keys = vector_of(ctx, &source)?.0;
        let at = find_ascending(ctx, &keys, &value)?;
        if at < 0 {
            return Err(ErrorCode::Na);
        }
        let result = args.get(2).cloned().unwrap_or_else(Value::blank);
        let (_, result_cols) = shape_of(&result)?;
        return if result_cols == 1 {
            cell_of(&result, at as usize, 0)
        } else {
            cell_of(&result, 0, at as usize)
        };
    }
    let vertical = rows >= cols;
    let keys = line_of(ctx, &source, vertical, 0)?;
    let at = find_ascending(ctx, &keys, &value)?;
    if at < 0 {
        return Err(ErrorCode::Na);
    }
    if vertical {
        cell_of(&source, at as usize, cols.saturating_sub(1))
    } else {
        cell_of(&source, rows.saturating_sub(1), at as usize)
    }
}

fn fn_match(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let lookup = blank_as_zero(&args[0])?;
    let kind = if args.len() > 2 {
        int_of(ctx, &args[2])?
    } else {
        1.0
    };
    let kind = kind.signum();
    let vector = vector_of(
        ctx,
        &lookup_array(ctx, args.get(1).unwrap_or(&Value::blank()))?,
    )?
    .0;
    let at = if kind == 0.0 {
        find_exact(ctx, &vector, &lookup, true, false)
    } else if kind > 0.0 {
        find_ascending(ctx, &vector, &lookup)?
    } else {
        find_descending(ctx, &vector, &lookup)?
    };
    if at < 0 {
        Err(ErrorCode::Na)
    } else {
        Ok(Value::number(at as f64 + 1.0))
    }
}

fn fn_xmatch(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let value = lookup_value(&args[0])?;
    let vector = vector_of(ctx, args.get(1).unwrap_or(&Value::blank()))?.0;
    let match_mode = if args.len() > 2 {
        int_of(ctx, &args[2])?
    } else {
        0.0
    };
    // A missing search mode is 1 (`missingDefaults`). An empty cell is blank here too.
    let search_mode = if args.len() > 3 && !is_blank(&args[3]) {
        int_of(ctx, &args[3])?
    } else {
        1.0
    };
    let at = xsearch(
        ctx,
        &vector,
        &value,
        match_mode.trunc() as i32,
        search_mode.trunc() as i32,
    )?;
    if at < 0 {
        Err(ErrorCode::Na)
    } else {
        Ok(Value::number(at as f64 + 1.0))
    }
}

fn fn_xlookup(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let value = ctx.to_scalar(&args[0]);
    if let Scalar::Error(code) = value {
        return Err(code);
    }
    let lookup_arg = args.get(1).cloned().unwrap_or_else(Value::blank);
    let (vector, vertical) = vector_of(ctx, &lookup_arg)?;
    let result = args.get(2).cloned().unwrap_or_else(Value::blank);
    let (result_rows, result_cols) = shape_of(&result)?;
    let match_mode = if args.len() > 4 && !is_blank(&args[4]) {
        num(ctx, &Value::Scalar(ctx.to_scalar(&args[4])))?
    } else {
        0.0
    };
    let search_mode = if args.len() > 5 && !is_blank(&args[5]) {
        num(ctx, &Value::Scalar(ctx.to_scalar(&args[5])))?
    } else {
        1.0
    };
    let (lookup_rows, lookup_cols) = shape_of(&lookup_arg)?;
    let lookup_len = if vertical { lookup_rows } else { lookup_cols };
    let result_len = if vertical { result_rows } else { result_cols };
    if result_len != lookup_len {
        return Err(ErrorCode::Value);
    }
    let at = xsearch(
        ctx,
        &vector,
        &value,
        match_mode.trunc() as i32,
        search_mode.trunc() as i32,
    )?;
    if at < 0 {
        if args.len() > 3 && !is_blank(&args[3]) {
            return Ok(args[3].clone());
        }
        return Err(ErrorCode::Na);
    }
    if vertical {
        if result_cols == 1 {
            cell_of(&result, at as usize, 0)
        } else {
            slice_of(&result, true, at as usize)
        }
    } else if result_rows == 1 {
        cell_of(&result, 0, at as usize)
    } else {
        slice_of(&result, false, at as usize)
    }
}

fn fn_index(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let source = args.first().cloned().unwrap_or_else(Value::blank);
    if let Value::Ref(reference) = &source {
        if reference.areas.len() > 1 {
            let area = if args.len() > 3 {
                int_of(ctx, &args[3])?
            } else {
                1.0
            };
            let chosen = reference
                .areas
                .get((area.trunc() as isize - 1) as usize)
                .copied()
                .ok_or(ErrorCode::Ref)?;
            let mut next = vec![Value::Ref(RefValue {
                areas: vec![chosen],
            })];
            if args.len() > 1 {
                next.push(args[1].clone());
            }
            if args.len() > 2 {
                next.push(args[2].clone());
            }
            return fn_index(ctx, &next);
        }
    }
    let (rows, cols) = shape_of(&source)?;
    let mut r = if args.len() > 1 && !is_blank(&args[1]) {
        int_of(ctx, &args[1])?
    } else {
        0.0
    };
    let mut c = if args.len() > 2 && !is_blank(&args[2]) {
        int_of(ctx, &args[2])?
    } else {
        0.0
    };
    if args.len() <= 2 && rows == 1 && cols > 1 {
        c = r;
        r = 1.0;
    } else if args.len() <= 2 && r > 0.0 && rows > 1 && cols > 1 && matches!(source, Value::Ref(_))
    {
        return Err(ErrorCode::Ref);
    }
    if r < 0.0 || c < 0.0 || r > rows as f64 || c > cols as f64 {
        return Err(ErrorCode::Ref);
    }
    if r == 0.0 && c == 0.0 {
        return Ok(source);
    }
    if r == 0.0 {
        return if cols == 1 {
            Ok(source)
        } else {
            slice_of(&source, false, c as usize - 1)
        };
    }
    if c == 0.0 {
        return if rows == 1 {
            Ok(source)
        } else {
            slice_of(&source, true, r as usize - 1)
        };
    }
    cell_of(&source, r as usize - 1, c as usize - 1)
}

fn fn_choose(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let index = int_of(ctx, &args[0])?;
    if index < 1.0 || index >= args.len() as f64 {
        return Err(ErrorCode::Value);
    }
    Ok(args[index as usize].clone())
}

fn ref_arg(value: &Value) -> Fx<RefValue> {
    match value {
        Value::Ref(reference) => Ok(reference.clone()),
        Value::Scalar(Scalar::Error(code)) => Err(*code),
        _ => Err(ErrorCode::Value),
    }
}

fn fn_offset(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let reference = ref_arg(&args[0])?;
    if reference.areas.len() != 1 {
        return Err(ErrorCode::Value);
    }
    let area = reference.areas[0];
    let height = if args.len() > 3 && !is_blank(&args[3]) {
        int_of(ctx, &args[3])?
    } else {
        f64::from(area.end_row - area.start_row + 1)
    };
    let width = if args.len() > 4 && !is_blank(&args[4]) {
        int_of(ctx, &args[4])?
    } else {
        f64::from(area.end_col - area.start_col + 1)
    };
    if height == 0.0 || width == 0.0 {
        return Err(ErrorCode::Ref);
    }
    let top = f64::from(area.start_row) + int_of(ctx, &args[1])?;
    let left = f64::from(area.start_col) + int_of(ctx, &args[2])?;
    let (r1, r2) = if height > 0.0 {
        (top, top + height - 1.0)
    } else {
        (top + height + 1.0, top)
    };
    let (c1, c2) = if width > 0.0 {
        (left, left + width - 1.0)
    } else {
        (left + width + 1.0, left)
    };
    if r1 < 0.0 || c1 < 0.0 || r2 > f64::from(MAX_ROW) || c2 > f64::from(MAX_COL) {
        return Err(ErrorCode::Ref);
    }
    Ok(Value::Ref(RefValue {
        areas: vec![Area {
            sheet: area.sheet,
            start_row: r1 as i32,
            end_row: r2 as i32,
            start_col: c1 as i32,
            end_col: c2 as i32,
        }],
    }))
}

fn sheets_of(
    ctx: &CallCtx<'_>,
    prefix: &Option<crate::formula::SheetPrefix>,
) -> Option<Vec<usize>> {
    let Some(prefix) = prefix else {
        return Some(vec![ctx.sheet]);
    };
    if prefix.book.is_some() {
        return None;
    }
    let first = ctx.host.find_sheet(&prefix.sheet)?;
    let Some(second) = &prefix.sheet2 else {
        return Some(vec![first]);
    };
    let last = ctx.host.find_sheet(second)?;
    let (lo, hi) = if first <= last {
        (first, last)
    } else {
        (last, first)
    };
    Some((lo..=hi).collect())
}

fn ref_from_ast(ctx: &CallCtx<'_>, ast: &FormulaAst) -> Option<RefValue> {
    match ast {
        FormulaAst::Ref {
            prefix,
            reference,
            spill,
        } => {
            if *spill {
                return None;
            }
            let spec = reference.as_ref()?;
            let sheets = sheets_of(ctx, prefix)?;
            Some(RefValue {
                areas: sheets
                    .into_iter()
                    .map(|sheet| {
                        Area::normalize(
                            sheet,
                            spec.start.row,
                            spec.start.col,
                            spec.end.row,
                            spec.end.col,
                        )
                    })
                    .collect(),
            })
        }
        FormulaAst::Binary {
            op: BinaryOp::Range,
            left,
            right,
        } => {
            let left = ref_from_ast(ctx, left)?;
            let right = ref_from_ast(ctx, right)?;
            if left.areas.len() != 1 || right.areas.len() != 1 {
                return None;
            }
            let x = left.areas[0];
            let y = right.areas[0];
            if x.sheet != y.sheet {
                return None;
            }
            Some(RefValue {
                areas: vec![Area {
                    sheet: x.sheet,
                    start_row: x.start_row.min(y.start_row),
                    start_col: x.start_col.min(y.start_col),
                    end_row: x.end_row.max(y.end_row),
                    end_col: x.end_col.max(y.end_col),
                }],
            })
        }
        _ => None,
    }
}

fn unquote_sheet(name: &str) -> String {
    let inner = name
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .unwrap_or(name);
    inner.replace("''", "'")
}

fn r1c1_dimension(text: Option<&str>, base: i32, size: i32) -> Option<i32> {
    let text = text?;
    if text.is_empty() {
        return Some(base);
    }
    if let Some(inner) = text
        .strip_prefix('[')
        .and_then(|body| body.strip_suffix(']'))
    {
        let offset: i32 = inner.parse().ok()?;
        if offset.abs() >= size {
            return None;
        }
        let size = i64::from(size);
        let wrapped = (i64::from(base) + i64::from(offset)).rem_euclid(size);
        return Some(wrapped as i32);
    }
    let at = text.parse::<i32>().ok()? - 1;
    if at >= 0 && at < size {
        Some(at)
    } else {
        None
    }
}

fn r1c1_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^R(\[[+-]?\d+\]|\d+)?C(\[[+-]?\d+\]|\d+)?$").expect("r1c1"))
}

fn r1c1_row_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^R(\[[+-]?\d+\]|\d+)?$").expect("r1c1 row"))
}

fn r1c1_col_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^C(\[[+-]?\d+\]|\d+)?$").expect("r1c1 col"))
}

#[derive(Clone, Copy)]
struct Corner {
    kind: u8,
    start_row: i32,
    start_col: i32,
    end_row: i32,
    end_col: i32,
}

fn r1c1_corner(text: &str, base_row: i32, base_col: i32) -> Option<Corner> {
    let row_size = MAX_ROW as i32 + 1;
    let col_size = MAX_COL as i32 + 1;
    if let Some(caps) = r1c1_re().captures(text) {
        // An absent group means the formula cell's row or column.
        let row = match caps.get(1) {
            None => base_row,
            Some(group) => r1c1_dimension(Some(group.as_str()), base_row, row_size)?,
        };
        let col = match caps.get(2) {
            None => base_col,
            Some(group) => r1c1_dimension(Some(group.as_str()), base_col, col_size)?,
        };
        return Some(Corner {
            kind: 0,
            start_row: row,
            start_col: col,
            end_row: row,
            end_col: col,
        });
    }
    if let Some(caps) = r1c1_row_re().captures(text) {
        let row = match caps.get(1) {
            None => base_row,
            Some(m) => r1c1_dimension(Some(m.as_str()), base_row, row_size)?,
        };
        return Some(Corner {
            kind: 1,
            start_row: row,
            start_col: 0,
            end_row: row,
            end_col: MAX_COL as i32,
        });
    }
    if let Some(caps) = r1c1_col_re().captures(text) {
        let col = match caps.get(1) {
            None => base_col,
            Some(m) => r1c1_dimension(Some(m.as_str()), base_col, col_size)?,
        };
        return Some(Corner {
            kind: 2,
            start_row: 0,
            start_col: col,
            end_row: MAX_ROW as i32,
            end_col: col,
        });
    }
    None
}

fn parse_r1c1_range(text: &str, base_row: i32, base_col: i32) -> Option<Area> {
    if base_row < 0 || base_col < 0 || base_row > MAX_ROW as i32 || base_col > MAX_COL as i32 {
        return None;
    }
    let parts: Vec<&str> = text.trim().split(':').collect();
    if parts.is_empty() || parts.len() > 2 {
        return None;
    }
    let a = r1c1_corner(parts[0], base_row, base_col)?;
    let b = if parts.len() == 1 {
        a
    } else {
        r1c1_corner(parts[1], base_row, base_col)?
    };
    if a.kind != b.kind {
        return None;
    }
    Some(Area::normalize(
        0,
        a.start_row,
        a.start_col,
        b.end_row,
        b.end_col,
    ))
}

fn parse_r1c1(ctx: &CallCtx<'_>, text: &str) -> Option<RefValue> {
    let (sheet, body) = if let Some(bang) = text.rfind('!') {
        let name = unquote_sheet(&text[..bang]);
        let sheet = ctx.host.find_sheet(&name)?;
        (sheet, &text[bang + 1..])
    } else {
        (ctx.sheet, text)
    };
    let mut area = parse_r1c1_range(body, ctx.row, ctx.col)?;
    area.sheet = sheet;
    Some(RefValue { areas: vec![area] })
}

fn fn_indirect(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let text = str_of(ctx, &args[0])?.trim().to_owned();
    let a1 = if args.len() > 1 {
        bool_of(&args[1])?
    } else {
        true
    };
    let reference = if a1 {
        let ast = parse_formula(&text).ok();
        ast.as_ref().and_then(|ast| ref_from_ast(ctx, ast))
    } else {
        parse_r1c1(ctx, &text)
    };
    reference.map_or(Err(ErrorCode::Ref), |reference| Ok(Value::Ref(reference)))
}

fn rows_or_cols(ctx: &CallCtx<'_>, args: &[Value], vertical: bool) -> Fx<Value> {
    if args.is_empty() {
        let n = if vertical { ctx.row } else { ctx.col };
        return Ok(Value::number(f64::from(n + 1)));
    }
    let reference = ref_arg(&args[0])?;
    let area = reference.areas.first().ok_or(ErrorCode::Ref)?;
    if vertical {
        if area.start_row == area.end_row {
            return Ok(Value::number(f64::from(area.start_row + 1)));
        }
        let matrix = Matrix::build(area.rows(), 1, |r, _| {
            Ok(Scalar::Number(f64::from(area.start_row + r as i32 + 1)))
        })?;
        Ok(Value::Matrix(matrix))
    } else if area.start_col == area.end_col {
        Ok(Value::number(f64::from(area.start_col + 1)))
    } else {
        let matrix = Matrix::build(1, area.cols(), |_, c| {
            Ok(Scalar::Number(f64::from(area.start_col + c as i32 + 1)))
        })?;
        Ok(Value::Matrix(matrix))
    }
}

fn fn_row(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    rows_or_cols(ctx, args, true)
}

fn fn_column(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    rows_or_cols(ctx, args, false)
}

fn axis_count(value: &Value, vertical: bool) -> Fx<Value> {
    if let Value::Scalar(Scalar::Error(code)) = value {
        return Ok(Value::error(*code));
    }
    let (rows, cols) = shape_of(value)?;
    Ok(Value::number(if vertical { rows } else { cols } as f64))
}

fn fn_rows(_ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    axis_count(args.first().unwrap_or(&Value::blank()), true)
}

fn fn_columns(_ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    axis_count(args.first().unwrap_or(&Value::blank()), false)
}

fn fn_areas(_ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(ref_arg(&args[0])?.areas.len() as f64))
}

fn quote_sheet_name(name: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let simple = RE.get_or_init(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_.]*$").expect("sheet name"));
    if simple.is_match(name) && parse_address(name).is_none() {
        name.to_owned()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

fn fn_address(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let row = int_of(ctx, &args[0])?;
    let col = int_of(ctx, &args[1])?;
    // Missing abs / A1 slots use 1 and TRUE. A blank cell takes the same defaults.
    let abs = if args.len() > 2 && !is_blank(&args[2]) {
        int_of(ctx, &args[2])?.trunc()
    } else {
        1.0
    };
    let a1 = if args.len() > 3 && !is_blank(&args[3]) {
        bool_of(&args[3])?
    } else {
        true
    };
    if row < 1.0
        || col < 1.0
        || row > f64::from(MAX_ROW) + 1.0
        || col > f64::from(MAX_COL) + 1.0
        || abs < 1.0
        || abs > 4.0
    {
        return Err(ErrorCode::Value);
    }
    let abs_row = abs == 1.0 || abs == 2.0;
    let abs_col = abs == 1.0 || abs == 3.0;
    let mut text = if a1 {
        format!(
            "{}{}{}{}",
            if abs_col { "$" } else { "" },
            column_label(col as u32 - 1),
            if abs_row { "$" } else { "" },
            row as i64
        )
    } else {
        format!(
            "R{}C{}",
            if abs_row {
                row.to_string()
            } else {
                format!("[{}]", row as i64)
            },
            if abs_col {
                col.to_string()
            } else {
                format!("[{}]", col as i64)
            }
        )
    };
    if args.len() > 4 && !is_blank(&args[4]) {
        let sheet = str_of(ctx, &args[4])?;
        if !sheet.is_empty() {
            text = format!("{}!{text}", quote_sheet_name(&sheet));
        }
    }
    Ok(Value::Scalar(Scalar::Text(text)))
}

fn fn_hyperlink(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    if args.len() > 1 && !is_blank(&args[1]) {
        return Ok(match &args[1] {
            Value::Scalar(scalar) => Value::Scalar(scalar.clone()),
            other => Value::Scalar(ctx.to_scalar(other)),
        });
    }
    Ok(Value::Scalar(Scalar::Text(str_of(ctx, &args[0])?)))
}

fn fn_anchor_array(_ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    // A non-reference is `#VALUE!`. Spill footprints of other anchors live on the
    // calc engine (`TODO(#72)`), so a real reference is `#REF!` here.
    let reference = ref_arg(&args[0])?;
    if reference.areas.is_empty() {
        return Err(ErrorCode::Ref);
    }
    Err(ErrorCode::Ref)
}

fn fn_single(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::Scalar(ctx.to_scalar(&args[0])))
}

fn fn_formulatext(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let area = ref_arg(&args[0])?
        .areas
        .first()
        .copied()
        .ok_or(ErrorCode::Ref)?;
    match ctx
        .host
        .cell_formula(area.sheet, area.start_row, area.start_col)
    {
        Some(formula) => Ok(Value::Scalar(Scalar::Text(format!("={formula}")))),
        None => Err(ErrorCode::Na),
    }
}

pub(super) fn specs() -> Vec<FunctionSpec> {
    vec![
        gen_spec("OFFSET", C, "OFFSET(reference, rows, cols, [height], [width])", "A reference offset from a starting reference.", 3, 5, ANY_VALUE, true, fn_offset),
        gen_spec("INDIRECT", C, "INDIRECT(ref_text, [a1])", "The reference named by a text.", 1, 2, &[], true, fn_indirect),
        gen_spec("ROW", C, "ROW([reference])", "The row number of a reference.", 0, 1, ANY, false, fn_row),
        gen_spec("COLUMN", C, "COLUMN([reference])", "The column number of a reference.", 0, 1, ANY, false, fn_column),
        gen_spec("ROWS", C, "ROWS(array)", "The number of rows in a reference or array.", 1, 1, ANY, false, fn_rows),
        gen_spec("COLUMNS", C, "COLUMNS(array)", "The number of columns in a reference or array.", 1, 1, ANY, false, fn_columns),
        gen_spec("AREAS", C, "AREAS(reference)", "The number of areas in a reference.", 1, 1, ANY, false, fn_areas),
        gen_spec("ADDRESS", C, "ADDRESS(row_num, column_num, [abs_num], [a1], [sheet_text])", "A cell address as text.", 2, 5, &[], false, fn_address),
        gen_spec("HYPERLINK", C, "HYPERLINK(link_location, [friendly_name])", "A link; the cell shows the friendly name.", 1, 2, &[], false, fn_hyperlink),
        gen_spec("ANCHORARRAY", C, "ANCHORARRAY(reference)", "The spill range of a dynamic array formula (A1#).", 1, 1, ANY, false, fn_anchor_array),
        gen_spec("SINGLE", C, "SINGLE(value)", "Implicit intersection (the @ operator).", 1, 1, ANY, false, fn_single),
        gen_spec("FORMULATEXT", C, "FORMULATEXT(reference)", "The formula of a cell as text.", 1, 1, ANY, false, fn_formulatext),
        gen_spec("VLOOKUP", C, "VLOOKUP(lookup_value, table_array, col_index_num, [range_lookup])", "Looks up a value in the first column of a table.", 3, 4, LOOKUP_PARAMS, false, fn_vlookup),
        gen_spec("HLOOKUP", C, "HLOOKUP(lookup_value, table_array, row_index_num, [range_lookup])", "Looks up a value in the first row of a table.", 3, 4, LOOKUP_PARAMS, false, fn_hlookup),
        gen_spec("LOOKUP", C, "LOOKUP(lookup_value, lookup_vector, [result_vector])", "Looks up a value in a sorted vector or array.", 2, 3, LOOKUP3, false, fn_lookup),
        gen_spec("MATCH", C, "MATCH(lookup_value, lookup_array, [match_type])", "The position of a value in a range.", 2, 3, LOOKUP_PARAMS, false, fn_match),
        gen_spec("XMATCH", C, "XMATCH(lookup_value, lookup_array, [match_mode], [search_mode])", "The position of a value with flexible matching.", 2, 4, LOOKUP_PARAMS, false, fn_xmatch),
        gen_spec("XLOOKUP", C, "XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found], [match_mode], [search_mode])", "Searches a range and returns the matching item from another.", 3, 6, VALUE_ANY, false, fn_xlookup),
        gen_spec("INDEX", C, "INDEX(array, row_num, [column_num], [area_num])", "The value or reference at a position.", 1, 4, INDEX_PARAMS, false, fn_index),
        gen_spec("CHOOSE", C, "CHOOSE(index_num, value1, [value2], ...)", "Chooses a value from a list by index.", 2, 255, VALUE_ANY, false, fn_choose),
    ]
}
