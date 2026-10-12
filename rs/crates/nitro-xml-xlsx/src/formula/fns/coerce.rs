// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/coerce.ts; changes listed in PROVENANCE.md.
//! Excel's scalar coercion and comparison rules.

use super::text_num::{number_to_text, parse_number_text, round15};
use super::value::{ErrorCode, Fx, Scalar};

/// A number as arithmetic sees it: blank is 0, TRUE is 1, numeric text converts.
pub fn to_number(value: &Scalar, date1904: bool) -> Fx<f64> {
    match value {
        Scalar::Number(n) => Ok(*n),
        Scalar::Blank => Ok(0.0),
        Scalar::Bool(true) => Ok(1.0),
        Scalar::Bool(false) => Ok(0.0),
        Scalar::Text(text) => parse_number_text(text, date1904).ok_or(ErrorCode::Value),
        Scalar::Error(code) => Err(*code),
    }
}

/// Text as `&` sees it. Numbers use General formatting.
pub fn to_text(value: &Scalar) -> Fx<String> {
    match value {
        Scalar::Text(text) => Ok(text.clone()),
        Scalar::Number(n) => Ok(number_to_text(*n)),
        Scalar::Bool(true) => Ok("TRUE".to_owned()),
        Scalar::Bool(false) => Ok("FALSE".to_owned()),
        Scalar::Blank => Ok(String::new()),
        Scalar::Error(code) => Err(*code),
    }
}

/// A logical as `IF` sees it. Numbers are non-zero; text must be TRUE or FALSE.
pub fn to_bool(value: &Scalar) -> Fx<bool> {
    match value {
        Scalar::Bool(value) => Ok(*value),
        Scalar::Number(n) => Ok(*n != 0.0),
        Scalar::Blank => Ok(false),
        Scalar::Text(text) => {
            let upper = text.to_uppercase();
            if upper == "TRUE" {
                Ok(true)
            } else if upper == "FALSE" {
                Ok(false)
            } else {
                Err(ErrorCode::Value)
            }
        }
        Scalar::Error(code) => Err(*code),
    }
}

/// A finite result, or `#NUM!`.
pub fn finite(value: f64) -> Fx<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ErrorCode::Num)
    }
}

fn type_rank(value: &Scalar) -> u8 {
    match value {
        Scalar::Number(_) => 0,
        Scalar::Text(_) => 1,
        Scalar::Bool(_) => 2,
        _ => 3,
    }
}

/// Compares numbers at Excel's 15-significant-digit precision.
pub fn compare_numbers(a: f64, b: f64) -> i8 {
    if a == b {
        return 0;
    }
    if (a - b).abs() > a.abs().max(b.abs()) * 1e-14 {
        return if a < b { -1 } else { 1 };
    }
    let ra = round15(a);
    let rb = round15(b);
    if ra == rb {
        0
    } else if ra < rb {
        -1
    } else {
        1
    }
}

/// Case-insensitive text comparison.
pub fn compare_text(a: &str, b: &str) -> i8 {
    let la = a.to_lowercase();
    let lb = b.to_lowercase();
    if la == lb {
        0
    } else if la < lb {
        -1
    } else {
        1
    }
}

fn blank_like(other: &Scalar) -> Scalar {
    match other {
        Scalar::Number(_) => Scalar::Number(0.0),
        Scalar::Text(_) => Scalar::Text(String::new()),
        Scalar::Bool(_) => Scalar::Bool(false),
        _ => Scalar::Blank,
    }
}

/// Compares two non-error scalars the way Excel's comparison operators do.
pub fn compare_scalars(a: &Scalar, b: &Scalar) -> Fx<i8> {
    if let Scalar::Error(code) = a {
        return Err(*code);
    }
    if let Scalar::Error(code) = b {
        return Err(*code);
    }
    if matches!(a, Scalar::Blank) && matches!(b, Scalar::Blank) {
        return Ok(0);
    }
    if matches!(a, Scalar::Blank) {
        return compare_scalars(&blank_like(b), b);
    }
    if matches!(b, Scalar::Blank) {
        return compare_scalars(a, &blank_like(a));
    }
    let ta = type_rank(a);
    let tb = type_rank(b);
    if ta != tb {
        return Ok(if ta < tb { -1 } else { 1 });
    }
    match (a, b) {
        (Scalar::Number(x), Scalar::Number(y)) => Ok(compare_numbers(*x, *y)),
        (Scalar::Text(x), Scalar::Text(y)) => Ok(compare_text(x, y)),
        (Scalar::Bool(x), Scalar::Bool(y)) => Ok(if x == y {
            0
        } else if *x {
            1
        } else {
            -1
        }),
        _ => Ok(0),
    }
}
