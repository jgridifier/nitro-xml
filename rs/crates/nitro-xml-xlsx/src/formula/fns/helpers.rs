// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/helpers.ts; changes listed in PROVENANCE.md.
//! Argument coercion shared by math and statistical functions.
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]

use std::sync::OnceLock;

use regex::Regex;

use super::coerce::{compare_scalars, to_bool, to_number, to_text};
use super::host::CallCtx;
use super::text_num::parse_number_text;
use super::value::{ErrorCode, Fx, Scalar, Value, VisitKind};

/// A scalar argument (lifting already happened). A matrix contributes its first cell.
pub fn scalar_of(value: &Value) -> Fx<Scalar> {
    match value {
        Value::Scalar(scalar) => Ok(scalar.clone()),
        Value::Matrix(matrix) => Ok(matrix.get(0, 0)),
        Value::Ref(_) => Err(ErrorCode::Value),
    }
}

pub fn num(ctx: &CallCtx<'_>, value: &Value) -> Fx<f64> {
    to_number(&scalar_of(value)?, ctx.date1904)
}

pub fn str_of(ctx: &CallCtx<'_>, value: &Value) -> Fx<String> {
    let _ = ctx;
    to_text(&scalar_of(value)?)
}

pub fn bool_of(value: &Value) -> Fx<bool> {
    to_bool(&scalar_of(value)?)
}

pub fn int_of(ctx: &CallCtx<'_>, value: &Value) -> Fx<f64> {
    Ok(num(ctx, value)?.trunc())
}

pub fn omitted(args: &[Value], index: usize) -> bool {
    index >= args.len() || matches!(args[index], Value::Scalar(Scalar::Blank))
}

/// Numbers from aggregate arguments. Direct arguments coerce text and logicals; references and
/// arrays contribute numbers only unless `include_all`.
pub fn collect_numbers(
    ctx: &CallCtx<'_>,
    args: &[Value],
    include_all: bool,
    ignore_errors: bool,
) -> Fx<Vec<f64>> {
    let mut out = Vec::new();
    for arg in args {
        let mut error = None;
        ctx.for_each(arg, &mut |value, kind| {
            if error.is_some() {
                return;
            }
            if let Scalar::Number(n) = value {
                out.push(*n);
                return;
            }
            if let Scalar::Error(code) = value {
                if !ignore_errors {
                    error = Some(*code);
                }
                return;
            }
            if kind == VisitKind::Direct {
                if matches!(value, Scalar::Blank) {
                    out.push(0.0);
                } else if let Ok(n) = to_number(value, ctx.date1904) {
                    out.push(n);
                } else if let Err(code) = to_number(value, ctx.date1904) {
                    error = Some(code);
                }
                return;
            }
            if include_all {
                match value {
                    Scalar::Bool(true) => out.push(1.0),
                    Scalar::Bool(false) => out.push(0.0),
                    Scalar::Text(_) => out.push(0.0),
                    _ => {}
                }
            }
        });
        if let Some(code) = error {
            return Err(code);
        }
    }
    Ok(out)
}

pub fn numbers_of(values: &[Scalar]) -> Vec<f64> {
    values
        .iter()
        .filter_map(|value| match value {
            Scalar::Number(n) => Some(*n),
            _ => None,
        })
        .collect()
}

fn escape_regex(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if matches!(
            ch,
            '.' | '*'
                | '+'
                | '?'
                | '^'
                | '$'
                | '{'
                | '}'
                | '('
                | ')'
                | '|'
                | '['
                | ']'
                | '\\'
                | '/'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Case-insensitive regular expression for `*`, `?` and `~`.
pub fn wildcard_regex(pattern: &str) -> Regex {
    let mut source = String::from("(?i)^");
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '~' && i + 1 < chars.len() {
            source.push_str(&escape_regex(&chars[i + 1].to_string()));
            i += 2;
            continue;
        } else if ch == '*' {
            source.push_str(r"[\s\S]*");
        } else if ch == '?' {
            source.push_str(r"[\s\S]");
        } else {
            source.push_str(&escape_regex(&ch.to_string()));
        }
        i += 1;
    }
    source.push('$');
    Regex::new(&source).unwrap_or_else(|_| Regex::new("(?i)^$").expect("empty wildcard"))
}

pub fn has_wildcards(text: &str) -> bool {
    text.chars().any(|ch| matches!(ch, '*' | '?' | '~'))
}

fn criteria_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(<=|>=|<>|<|>|=)?([\s\S]*)$").expect("criteria pattern"))
}

/// A predicate for COUNTIF-style criteria.
pub fn make_criteria(criteria: Scalar, date1904: bool) -> Box<dyn Fn(&Scalar) -> bool> {
    if let Scalar::Error(code) = criteria {
        return Box::new(move |value| matches!(value, Scalar::Error(found) if *found == code));
    }
    if let Scalar::Number(target) = criteria {
        return Box::new(move |value| match value {
            Scalar::Number(n) => {
                compare_scalars(&Scalar::Number(*n), &Scalar::Number(target)).ok() == Some(0)
            }
            Scalar::Text(text) => parse_number_text(text, date1904) == Some(target),
            _ => false,
        });
    }
    if let Scalar::Bool(bit) = criteria {
        return Box::new(move |value| matches!(value, Scalar::Bool(found) if *found == bit));
    }
    if matches!(criteria, Scalar::Blank) {
        return Box::new(|value| match value {
            Scalar::Blank => true,
            Scalar::Text(text) => text.is_empty(),
            _ => false,
        });
    }
    let Scalar::Text(text) = criteria else {
        return Box::new(|_| false);
    };
    let caps = criteria_re().captures(&text);
    let op = caps
        .as_ref()
        .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()))
        .unwrap_or_default();
    let operand = caps
        .as_ref()
        .and_then(|c| c.get(2).map(|m| m.as_str().to_owned()))
        .unwrap_or_default();
    let as_number = parse_number_text(&operand, date1904);
    let upper = operand.to_uppercase();
    let as_bool = if upper == "TRUE" {
        Some(true)
    } else if upper == "FALSE" {
        Some(false)
    } else {
        None
    };
    let error_operand = operand.starts_with('#').then(|| operand.to_uppercase());
    if op.is_empty() || op == "=" || op == "<>" {
        let positive: Box<dyn Fn(&Scalar) -> bool> = if operand.is_empty() {
            if op.is_empty() {
                return Box::new(|value| match value {
                    Scalar::Blank => true,
                    Scalar::Text(text) => text.is_empty(),
                    _ => false,
                });
            }
            if op == "=" {
                return Box::new(|value| matches!(value, Scalar::Blank));
            }
            return Box::new(|value| !matches!(value, Scalar::Blank));
        } else if let Some(target) = as_number {
            if op == "<>" {
                return Box::new(
                    move |value| !matches!(value, Scalar::Number(n) if compare_numbers_ok(*n, target)),
                );
            }
            Box::new(move |value| match value {
                Scalar::Number(n) => compare_numbers_ok(*n, target),
                Scalar::Text(text) => parse_number_text(text, date1904) == Some(target),
                _ => false,
            })
        } else if let Some(bit) = as_bool {
            Box::new(move |value| matches!(value, Scalar::Bool(found) if *found == bit))
        } else if let Some(code) = error_operand.clone() {
            Box::new(
                move |value| matches!(value, Scalar::Error(found) if found.as_excel().eq_ignore_ascii_case(&code)),
            )
        } else if has_wildcards(&operand) {
            let regex = wildcard_regex(&operand);
            Box::new(move |value| matches!(value, Scalar::Text(text) if regex.is_match(text)))
        } else {
            let lower = operand.to_lowercase();
            Box::new(
                move |value| matches!(value, Scalar::Text(text) if text.to_lowercase() == lower),
            )
        };
        return if op == "<>" {
            Box::new(move |value| !positive(value))
        } else {
            positive
        };
    }
    let op_owned = op;
    let sign = move |cmp: i8| -> bool {
        match op_owned.as_str() {
            "<" => cmp < 0,
            ">" => cmp > 0,
            "<=" => cmp <= 0,
            _ => cmp >= 0,
        }
    };
    if let Some(target) = as_number {
        return Box::new(move |value| match value {
            Scalar::Number(n) => sign(super::coerce::compare_numbers(*n, target)),
            _ => false,
        });
    }
    if let Some(bit) = as_bool {
        return Box::new(move |value| match value {
            Scalar::Bool(found) => {
                let cmp = compare_scalars(&Scalar::Bool(*found), &Scalar::Bool(bit)).unwrap_or(0);
                sign(cmp)
            }
            _ => false,
        });
    }
    let operand_text = operand;
    Box::new(move |value| match value {
        Scalar::Text(text) => {
            let cmp = compare_scalars(
                &Scalar::Text(text.clone()),
                &Scalar::Text(operand_text.clone()),
            )
            .unwrap_or(0);
            sign(cmp)
        }
        _ => false,
    })
}

fn compare_numbers_ok(a: f64, b: f64) -> bool {
    super::coerce::compare_numbers(a, b) == 0
}
