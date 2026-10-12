// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/info.ts;
// changes listed in PROVENANCE.md.
//! Information functions (`ISBLANK` through `CELL`).

use std::collections::HashSet;

use super::host::CallCtx;
use super::spec::{gen_spec, FunctionSpec, ParamKind};
use super::value::{ErrorCode, Fx, Scalar, Value};

const C: &str = "Information";
const ANY: &[ParamKind] = &[ParamKind::Any];

macro_rules! is_spec {
    ($name:literal, $syntax:literal, $desc:literal, $test:expr) => {
        gen_spec($name, C, $syntax, $desc, 1, 1, &[], false, |_ctx, args| {
            let scalar = super::helpers::scalar_of(args.first().unwrap_or(&Value::blank()))?;
            let test: fn(&Scalar) -> bool = $test;
            Ok(Value::Scalar(Scalar::Bool(test(&scalar))))
        })
    };
}

pub fn specs() -> Vec<FunctionSpec> {
    vec![
        is_spec!(
            "ISBLANK",
            "ISBLANK(value)",
            "TRUE when a cell is empty.",
            |v| { matches!(v, Scalar::Blank) }
        ),
        is_spec!(
            "ISERROR",
            "ISERROR(value)",
            "TRUE for any error value.",
            |v| matches!(v, Scalar::Error(_))
        ),
        is_spec!(
            "ISERR",
            "ISERR(value)",
            "TRUE for errors other than #N/A.",
            |v| matches!(v, Scalar::Error(code) if *code != ErrorCode::Na)
        ),
        is_spec!("ISNA", "ISNA(value)", "TRUE for #N/A.", |v| {
            matches!(v, Scalar::Error(ErrorCode::Na))
        }),
        is_spec!("ISNUMBER", "ISNUMBER(value)", "TRUE for numbers.", |v| {
            matches!(v, Scalar::Number(_))
        }),
        is_spec!("ISTEXT", "ISTEXT(value)", "TRUE for text.", |v| {
            matches!(v, Scalar::Text(_))
        }),
        is_spec!(
            "ISNONTEXT",
            "ISNONTEXT(value)",
            "TRUE for anything but text.",
            |v| !matches!(v, Scalar::Text(_))
        ),
        is_spec!(
            "ISLOGICAL",
            "ISLOGICAL(value)",
            "TRUE for logical values.",
            |v| matches!(v, Scalar::Bool(_))
        ),
        gen_spec(
            "ISEVEN",
            C,
            "ISEVEN(number)",
            "TRUE for even numbers.",
            1,
            1,
            &[],
            false,
            |ctx, args| parity(ctx, args, false),
        ),
        gen_spec(
            "ISODD",
            C,
            "ISODD(number)",
            "TRUE for odd numbers.",
            1,
            1,
            &[],
            false,
            |ctx, args| parity(ctx, args, true),
        ),
        gen_spec(
            "ISREF",
            C,
            "ISREF(value)",
            "TRUE for references.",
            1,
            1,
            ANY,
            false,
            |_, args| {
                Ok(Value::Scalar(Scalar::Bool(matches!(
                    args.first(),
                    Some(Value::Ref(_))
                ))))
            },
        ),
        gen_spec(
            "ISFORMULA",
            C,
            "ISFORMULA(reference)",
            "TRUE when a cell holds a formula.",
            1,
            1,
            ANY,
            false,
            is_formula,
        ),
        gen_spec(
            "N",
            C,
            "N(value)",
            "A value converted to a number (text is 0).",
            1,
            1,
            &[],
            false,
            n_fn,
        ),
        gen_spec(
            "NA",
            C,
            "NA()",
            "The #N/A error value.",
            0,
            0,
            &[],
            false,
            |_, _| Ok(Value::error(ErrorCode::Na)),
        ),
        gen_spec(
            "TYPE",
            C,
            "TYPE(value)",
            "The type of a value: 1 number, 2 text, 4 logical, 16 error, 64 array.",
            1,
            1,
            ANY,
            false,
            type_fn,
        ),
        gen_spec(
            "ERROR.TYPE",
            C,
            "ERROR.TYPE(error_val)",
            "The number of an error value.",
            1,
            1,
            &[],
            false,
            error_type,
        ),
        gen_spec(
            "SHEET",
            C,
            "SHEET([value])",
            "The sheet number of a reference or sheet name.",
            0,
            1,
            ANY,
            false,
            sheet_fn,
        ),
        gen_spec(
            "SHEETS",
            C,
            "SHEETS([reference])",
            "The number of sheets in a reference (or the workbook).",
            0,
            1,
            ANY,
            false,
            sheets_fn,
        ),
        gen_spec(
            "CELL",
            C,
            "CELL(info_type, [reference])",
            "Information about a cell (address, row, col, contents, type, filename, sheet).",
            1,
            2,
            ANY,
            true,
            cell_fn,
        ),
    ]
}

fn parity(ctx: &CallCtx<'_>, args: &[Value], odd: bool) -> Fx<Value> {
    let scalar = super::helpers::scalar_of(args.first().unwrap_or(&Value::blank()))?;
    if let Scalar::Error(code) = scalar {
        return Ok(Value::error(code));
    }
    if matches!(scalar, Scalar::Bool(_)) {
        return Err(ErrorCode::Value);
    }
    let n = super::coerce::to_number(&scalar, ctx.date1904)?;
    Ok(Value::Scalar(Scalar::Bool(
        n.trunc().abs() as i64 % 2 == i64::from(odd),
    )))
}

fn is_formula(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let Some(value) = args.first() else {
        return Err(ErrorCode::Value);
    };
    let Value::Ref(reference) = value else {
        return match value {
            Value::Scalar(Scalar::Error(code)) => Ok(Value::error(*code)),
            _ => Err(ErrorCode::Value),
        };
    };
    let Some(area) = reference.areas.first() else {
        return Err(ErrorCode::Ref);
    };
    Ok(Value::Scalar(Scalar::Bool(
        ctx.host
            .cell_formula(area.sheet, area.start_row, area.start_col)
            .is_some(),
    )))
}

fn n_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let _ = ctx;
    let scalar = super::helpers::scalar_of(args.first().unwrap_or(&Value::blank()))?;
    let n = match scalar {
        Scalar::Error(code) => return Ok(Value::error(code)),
        Scalar::Number(n) => n,
        Scalar::Bool(true) => 1.0,
        _ => 0.0,
    };
    Ok(Value::number(n))
}

fn type_code(value: &Value) -> f64 {
    match value {
        Value::Ref(reference) if !reference.is_cell() => 64.0,
        Value::Ref(_) => 0.0,
        Value::Matrix(_) => 64.0,
        Value::Lambda(_) => 128.0,
        Value::Scalar(Scalar::Error(_)) => 16.0,
        Value::Scalar(Scalar::Text(_)) => 2.0,
        Value::Scalar(Scalar::Bool(_)) => 4.0,
        Value::Scalar(_) => 1.0,
    }
}

fn type_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let Some(value) = args.first() else {
        return Ok(Value::number(1.0));
    };
    if let Value::Ref(reference) = value {
        if reference.is_cell() {
            return Ok(Value::number(type_code(&Value::Scalar(
                ctx.to_scalar(value),
            ))));
        }
    }
    Ok(Value::number(type_code(value)))
}

fn error_number(code: ErrorCode) -> f64 {
    match code {
        ErrorCode::Null => 1.0,
        ErrorCode::Div0 => 2.0,
        ErrorCode::Value => 3.0,
        ErrorCode::Ref => 4.0,
        ErrorCode::Name => 5.0,
        ErrorCode::Num => 6.0,
        ErrorCode::Na => 7.0,
        ErrorCode::Spill => 9.0,
        ErrorCode::Calc => 14.0,
    }
}

fn error_type(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let _ = ctx;
    let scalar = super::helpers::scalar_of(args.first().unwrap_or(&Value::blank()))?;
    match scalar {
        Scalar::Error(code) => Ok(Value::number(error_number(code))),
        _ => Ok(Value::error(ErrorCode::Na)),
    }
}

fn sheet_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let Some(value) = args.first() else {
        return Ok(Value::number(ctx.sheet as f64 + 1.0));
    };
    if let Value::Ref(reference) = value {
        let sheet = reference
            .areas
            .first()
            .map(|area| area.sheet)
            .unwrap_or(ctx.sheet);
        return Ok(Value::number(sheet as f64 + 1.0));
    }
    let name = super::helpers::str_of(ctx, value)?;
    match ctx.host.find_sheet(&name) {
        Some(index) => Ok(Value::number(index as f64 + 1.0)),
        None => Ok(Value::error(ErrorCode::Na)),
    }
}

fn sheets_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let Some(value) = args.first() else {
        return Ok(Value::number(ctx.host.sheet_count() as f64));
    };
    let Value::Ref(reference) = value else {
        return Err(ErrorCode::Value);
    };
    let sheets: HashSet<usize> = reference.areas.iter().map(|area| area.sheet).collect();
    Ok(Value::number(sheets.len() as f64))
}

fn cell_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let info = super::helpers::str_of(ctx, args.first().unwrap_or(&Value::blank()))?;
    let mut sheet = ctx.sheet;
    let mut row = ctx.row;
    let mut col = ctx.col;
    if let Some(reference) = args.get(1) {
        let Value::Ref(reference) = reference else {
            return Err(ErrorCode::Value);
        };
        let Some(area) = reference.areas.first() else {
            return Err(ErrorCode::Ref);
        };
        sheet = area.sheet;
        row = area.start_row;
        col = area.start_col;
    }
    let value = ctx.read_cell(sheet, row, col);
    match info.to_ascii_lowercase().as_str() {
        "address" => Ok(Value::Scalar(Scalar::Text(format!(
            "${}${}",
            crate::address::column_label(col as u32),
            row + 1
        )))),
        "row" => Ok(Value::number(f64::from(row + 1))),
        "col" => Ok(Value::number(f64::from(col + 1))),
        "contents" => Ok(match value {
            Scalar::Blank => Value::number(0.0),
            other => Value::Scalar(other),
        }),
        "type" => Ok(Value::Scalar(Scalar::Text(
            match value {
                Scalar::Blank => "b",
                Scalar::Text(_) => "l",
                _ => "v",
            }
            .to_owned(),
        ))),
        "filename" | "sheet" => Ok(Value::Scalar(Scalar::Text(
            ctx.host.sheet_name(sheet).unwrap_or("").to_owned(),
        ))),
        _ => Err(ErrorCode::Value),
    }
}
