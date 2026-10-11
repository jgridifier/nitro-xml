// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/numfmt/general.ts; changes listed in PROVENANCE.md.

use crate::numfmt::decimal::{
    pad_start, plain, round_decimals, round_significant, to_digits, Digits,
};

/// Width Excel's General format fits a number into (sign excluded).
const GENERAL_WIDTH: i32 = 11;

fn scientific(v: &Digits, width: i32) -> (String, usize) {
    let mut exp = v.point - 1;
    let mut exp_digits = 2.max(js_len(exp.unsigned_abs()));
    let mut r = round_significant(v, 1.max(width - 3 - exp_digits));
    if r.point - 1 != exp {
        exp = r.point - 1;
        exp_digits = 2.max(js_len(exp.unsigned_abs()));
        r = round_significant(v, 1.max(width - 3 - exp_digits));
    }
    let mantissa = if r.d.len() > 1 {
        format!("{}.{}", &r.d[..1], &r.d[1..])
    } else {
        r.d.clone()
    };
    let exp_text = pad_start(&exp.unsigned_abs().to_string(), 2, '0');
    let sign = if exp < 0 { '-' } else { '+' };
    (format!("{mantissa}E{sign}{exp_text}"), r.d.len())
}

fn js_len(n: u32) -> i32 {
    if n == 0 {
        1
    } else {
        n.to_string().len() as i32
    }
}

fn general_digits(v: &Digits, width: i32) -> String {
    if v.d.is_empty() {
        return "0".into();
    }
    let exp = v.point - 1;
    if exp >= width {
        return scientific(v, width).0;
    }
    if exp >= 0 {
        let r = round_decimals(v, (width - 2 - exp).max(0));
        if r.point > width {
            return scientific(v, width).0;
        }
        return plain(&r);
    }
    let fixed = round_decimals(v, (width - 2).max(0));
    let (text, sig) = scientific(v, width);
    if !fixed.d.is_empty() && fixed.d.len() >= sig {
        plain(&fixed)
    } else {
        text
    }
}

fn clamp_width(width: f64) -> i32 {
    if !width.is_finite() {
        return GENERAL_WIDTH;
    }
    let w = width.floor();
    if !w.is_finite() {
        return GENERAL_WIDTH;
    }
    let w = w.clamp(1.0, GENERAL_WIDTH as f64);
    w as i32
}

/// Excel's General number format.
///
/// `width` is characters, sign excluded, at least 1. `None` uses the 11-character default.
pub fn format_general(value: f64, width: Option<f64>) -> String {
    if !value.is_finite() {
        return "#NUM!".into();
    }
    let text = general_digits(
        &to_digits(value),
        clamp_width(width.unwrap_or(GENERAL_WIDTH as f64)),
    );
    if value < 0.0 && text != "0" {
        format!("-{text}")
    } else {
        text
    }
}
