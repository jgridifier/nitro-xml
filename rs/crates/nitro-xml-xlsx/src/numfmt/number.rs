// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/numfmt/number.ts; changes listed in PROVENANCE.md.

use crate::numfmt::decimal::{
    fraction_part, integer_part, is_zero, js_int_string, pad_start, round_decimals, shift,
    to_digits, Digits,
};
use crate::numfmt::types::Token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PlanToken {
    Lit(String),
    Int { index: usize },
    Point,
    Frac { index: usize },
    Exp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Exponent {
    pub plus: bool,
    pub digits: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NumberPlan {
    pub tokens: Vec<PlanToken>,
    pub int_digits: Vec<char>,
    pub frac_digits: Vec<char>,
    pub grouping: bool,
    /// Power of ten applied before formatting (`%` adds 2, a scaling comma subtracts 3).
    pub shift: i32,
    pub exponent: Option<Exponent>,
}

fn is_digit(t: &Token) -> bool {
    matches!(t, Token::Digit(_))
}

/// Resolves roles of a plain (non-fraction) number section.
pub(crate) fn compile_number(tokens: &[Token]) -> NumberPlan {
    let exp_at = tokens.iter().position(|t| matches!(t, Token::Exp { .. }));
    let end = exp_at.unwrap_or(tokens.len());
    let point_at = tokens
        .iter()
        .enumerate()
        .find(|(i, t)| *i < end && matches!(t, Token::Point))
        .map(|(i, _)| i)
        .unwrap_or(end);
    let mut int_digits = Vec::new();
    let mut frac_digits = Vec::new();
    let mut out = Vec::new();
    let mut grouping = false;
    let mut scale_commas = 0i32;
    let mut percent = 0i32;
    let mut exponent = None;
    let mut seen_digit = false;
    let has_int_after = |k: usize| {
        tokens[k + 1..point_at]
            .iter()
            .any(|t| matches!(t, Token::Digit(_)))
    };
    let mut i = 0;
    while i < tokens.len() {
        if exp_at == Some(i) {
            if let Token::Exp { plus } = tokens[i] {
                let mut n = 0;
                while i + 1 + n < tokens.len() && is_digit(&tokens[i + 1 + n]) {
                    n += 1;
                }
                exponent = Some(Exponent { plus, digits: n });
                out.push(PlanToken::Exp);
                i += n + 1;
                continue;
            }
        }
        match &tokens[i] {
            Token::Digit(c) => {
                seen_digit = true;
                if i < point_at {
                    out.push(PlanToken::Int {
                        index: int_digits.len(),
                    });
                    int_digits.push(*c);
                } else if i < end {
                    out.push(PlanToken::Frac {
                        index: frac_digits.len(),
                    });
                    frac_digits.push(*c);
                } else {
                    out.push(PlanToken::Lit(c.to_string()));
                }
            }
            Token::Point => {
                if i == point_at {
                    out.push(PlanToken::Point);
                } else {
                    out.push(PlanToken::Lit(".".into()));
                }
            }
            Token::Comma => {
                if i < point_at && seen_digit && has_int_after(i) {
                    grouping = true;
                } else if seen_digit && i < end {
                    scale_commas += 1;
                } else {
                    out.push(PlanToken::Lit(",".into()));
                }
            }
            Token::Percent => {
                percent += 1;
                out.push(PlanToken::Lit("%".into()));
            }
            Token::Lit(v) => out.push(PlanToken::Lit(v.clone())),
            Token::Num(v) => out.push(PlanToken::Lit(v.clone())),
            Token::Slash => out.push(PlanToken::Lit("/".into())),
            _ => {}
        }
        i += 1;
    }
    if int_digits.is_empty() && !frac_digits.is_empty() {
        let at = out
            .iter()
            .position(|t| matches!(t, PlanToken::Point))
            .unwrap_or(0);
        out.insert(at, PlanToken::Int { index: 0 });
        int_digits.insert(0, '#');
    }
    NumberPlan {
        tokens: out,
        int_digits,
        frac_digits,
        grouping,
        shift: 2 * percent - 3 * scale_commas,
        exponent,
    }
}

fn pad_digit(c: char) -> &'static str {
    match c {
        '0' => "0",
        '?' => " ",
        _ => "",
    }
}

/// Distributes integer digits over the placeholders (extra digits go to the leftmost one).
fn place_integers(digits: &str, slots: &[char], grouping: bool) -> Vec<String> {
    let n = slots.len();
    let mut out: Vec<String> = slots
        .iter()
        .enumerate()
        .map(|(j, c)| {
            let p = n - 1 - j;
            if p < digits.len() {
                let idx = digits.len() - 1 - p;
                digits[idx..idx + 1].to_string()
            } else {
                pad_digit(*c).to_string()
            }
        })
        .collect();
    if n > 0 && digits.len() > n {
        let extra = &digits[..digits.len() - n];
        out[0] = format!("{extra}{}", out[0]);
    }
    if !grouping {
        return out;
    }
    let mut remaining = out
        .iter()
        .flat_map(|s| s.chars())
        .filter(|ch| ch.is_ascii_digit())
        .count();
    out.into_iter()
        .map(|s| {
            let mut res = String::new();
            for ch in s.chars() {
                res.push(ch);
                if ch.is_ascii_digit() {
                    remaining -= 1;
                    if remaining > 0 && remaining % 3 == 0 {
                        res.push(',');
                    }
                }
            }
            res
        })
        .collect()
}

fn place_fraction(digits: &str, slots: &[char]) -> Vec<String> {
    let mut out: Vec<String> = (0..slots.len())
        .map(|i| {
            digits
                .chars()
                .nth(i)
                .map(|c| c.to_string())
                .unwrap_or_default()
        })
        .collect();
    for i in (0..slots.len()).rev() {
        let c = slots[i];
        let ch = digits.chars().nth(i);
        if ch != Some('0') || c == '0' {
            break;
        }
        out[i] = pad_digit(c).to_string();
    }
    out
}

struct Mantissa {
    value: Digits,
    exp: i32,
}

fn floor_div(a: i32, b: i32) -> i32 {
    let mut q = a / b;
    let r = a % b;
    if r != 0 && (a < 0) != (b < 0) {
        q -= 1;
    }
    q
}

fn scientific(v: &Digits, plan: &NumberPlan) -> Mantissa {
    if is_zero(v) {
        return Mantissa {
            value: v.clone(),
            exp: 0,
        };
    }
    let n = plan.int_digits.len() as i32;
    let engineering = n > 1 && plan.int_digits.iter().any(|c| *c != '0');
    let period = if engineering { n } else { 1 };
    let e = v.point - 1;
    let mut exp = floor_div(e, period) * period;
    let mut m = round_decimals(&shift(v, -exp), plan.frac_digits.len() as i32);
    if m.point > period {
        exp += period;
        m = round_decimals(&shift(v, -exp), plan.frac_digits.len() as i32);
    }
    Mantissa { value: m, exp }
}

/// Formats `|value|`; `zero` reports whether the displayed number rounded to zero.
pub(crate) fn render_number(value: f64, plan: &NumberPlan) -> (String, bool) {
    let v = shift(&to_digits(value), plan.shift);
    let (rounded, exp) = if plan.exponent.is_some() {
        let m = scientific(&v, plan);
        (m.value, m.exp)
    } else {
        (round_decimals(&v, plan.frac_digits.len() as i32), 0)
    };
    let ints = if plan.exponent.is_some() && is_zero(&rounded) {
        plan.int_digits.iter().map(|_| "0".to_string()).collect()
    } else {
        place_integers(&integer_part(&rounded), &plan.int_digits, plan.grouping)
    };
    let fracs = place_fraction(
        &fraction_part(&rounded, plan.frac_digits.len() as i32),
        &plan.frac_digits,
    );
    let mut text = String::new();
    for tok in &plan.tokens {
        match tok {
            PlanToken::Lit(v) => text.push_str(v),
            PlanToken::Int { index } => {
                if let Some(s) = ints.get(*index) {
                    text.push_str(s);
                }
            }
            PlanToken::Frac { index } => {
                if let Some(s) = fracs.get(*index) {
                    text.push_str(s);
                }
            }
            PlanToken::Point => text.push('.'),
            PlanToken::Exp => {
                if let Some(exponent) = &plan.exponent {
                    let sign = if exp < 0 {
                        "-"
                    } else if exponent.plus {
                        "+"
                    } else {
                        ""
                    };
                    let body = pad_start(
                        &js_int_string(f64::from(exp.unsigned_abs())),
                        exponent.digits,
                        '0',
                    );
                    text.push('E');
                    text.push_str(sign);
                    text.push_str(&body);
                }
            }
        }
    }
    (text, is_zero(&rounded))
}
