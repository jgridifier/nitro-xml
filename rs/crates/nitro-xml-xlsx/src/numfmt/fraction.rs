// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/numfmt/fraction.ts; changes listed in PROVENANCE.md.

use crate::numfmt::decimal::js_int_string;
use crate::numfmt::types::Token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FracToken {
    Lit(String),
    Int { index: usize },
    Num { index: usize },
    Slash,
    Den { index: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FractionPlan {
    pub tokens: Vec<FracToken>,
    pub int_digits: Vec<char>,
    pub num_digits: Vec<char>,
    pub den_digits: Vec<char>,
    pub fixed_denominator: Option<f64>,
    /// Characters the denominator occupies (used to blank a zero fraction).
    pub den_width: usize,
}

fn is_digit(t: &Token) -> bool {
    matches!(t, Token::Digit(_))
}

/// Index of the fraction slash (a `/` between digit placeholders), or `None`.
pub(crate) fn fraction_slash(tokens: &[Token]) -> Option<usize> {
    tokens.iter().enumerate().position(|(i, t)| {
        matches!(t, Token::Slash)
            && i > 0
            && is_digit(&tokens[i - 1])
            && matches!(tokens.get(i + 1), Some(Token::Digit(_) | Token::Num(_)))
    })
}

pub(crate) fn compile_fraction(tokens: &[Token], slash: usize) -> FractionPlan {
    let mut num_start = slash;
    while num_start > 0 && is_digit(&tokens[num_start - 1]) {
        num_start -= 1;
    }
    let mut den_end = slash + 1;
    while den_end < tokens.len()
        && (is_digit(&tokens[den_end]) || matches!(tokens[den_end], Token::Num(_)))
    {
        den_end += 1;
    }
    let den_tokens = &tokens[slash + 1..den_end];
    let fixed = den_tokens.iter().any(|t| matches!(t, Token::Num(_)))
        && den_tokens
            .iter()
            .all(|t| matches!(t, Token::Num(_)) || matches!(t, Token::Digit('0')));
    let mut plan = FractionPlan {
        tokens: Vec::new(),
        int_digits: Vec::new(),
        num_digits: Vec::new(),
        den_digits: Vec::new(),
        fixed_denominator: None,
        den_width: den_tokens.len(),
    };
    if fixed {
        let mut joined = String::new();
        for t in den_tokens {
            match t {
                Token::Num(v) => joined.push_str(v),
                Token::Digit('0') => joined.push('0'),
                _ => {}
            }
        }
        plan.fixed_denominator = joined.parse().ok();
    }
    for (i, tok) in tokens.iter().enumerate() {
        if i == slash {
            plan.tokens.push(FracToken::Slash);
        } else if i > slash && i < den_end {
            if fixed {
                if i == slash + 1 {
                    plan.tokens.push(FracToken::Den { index: 0 });
                }
            } else if let Token::Digit(c) = tok {
                plan.tokens.push(FracToken::Den {
                    index: plan.den_digits.len(),
                });
                plan.den_digits.push(*c);
            }
        } else if is_digit(tok) && i >= num_start && i < slash {
            if let Token::Digit(c) = tok {
                plan.tokens.push(FracToken::Num {
                    index: plan.num_digits.len(),
                });
                plan.num_digits.push(*c);
            }
        } else if is_digit(tok) && i < num_start {
            if let Token::Digit(c) = tok {
                plan.tokens.push(FracToken::Int {
                    index: plan.int_digits.len(),
                });
                plan.int_digits.push(*c);
            }
        } else if let Token::Lit(v) = tok {
            plan.tokens.push(FracToken::Lit(v.clone()));
        } else if let Token::Num(v) = tok {
            plan.tokens.push(FracToken::Lit(v.clone()));
        } else if let Token::Digit(c) = tok {
            plan.tokens.push(FracToken::Lit(c.to_string()));
        } else if matches!(tok, Token::Point) {
            plan.tokens.push(FracToken::Lit(".".into()));
        } else if matches!(tok, Token::Slash) {
            plan.tokens.push(FracToken::Lit("/".into()));
        } else if matches!(tok, Token::Percent) {
            plan.tokens.push(FracToken::Lit("%".into()));
        }
    }
    plan
}

/// Continued-fraction approximation, last convergent whose denominator fits.
fn approximate(x: f64, max_den: f64) -> (f64, f64) {
    if x.is_finite() && x.fract() == 0.0 {
        return (x, 1.0);
    }
    let mut p0 = 0.0;
    let mut q0 = 1.0;
    let mut p1 = 1.0;
    let mut q1 = 0.0;
    let mut y = x;
    for _ in 0..64 {
        let a = y.floor();
        let p2 = a * p1 + p0;
        let q2 = a * q1 + q0;
        if q2 > max_den {
            break;
        }
        p0 = p1;
        q0 = q1;
        p1 = p2;
        q1 = q2;
        let rest = y - a;
        if rest < 1e-10 {
            break;
        }
        y = 1.0 / rest;
    }
    (p1, q1)
}

fn pad_digit(c: char) -> &'static str {
    match c {
        '0' => "0",
        '?' => " ",
        _ => "",
    }
}

fn pad_left(s: &str, slots: &[char]) -> String {
    let mut prefix = String::new();
    let slen = s.chars().count();
    for slot in slots.iter().take(slots.len().saturating_sub(slen)) {
        prefix.push_str(pad_digit(*slot));
    }
    prefix.push_str(s);
    prefix
}

fn pad_right(s: &str, slots: &[char]) -> String {
    let mut suffix = String::new();
    let slen = s.chars().count();
    for slot in slots.iter().skip(slen) {
        suffix.push_str(pad_digit(*slot));
    }
    let mut out = String::new();
    out.push_str(s);
    out.push_str(&suffix);
    out
}

fn all_hash(slots: &[char]) -> bool {
    slots.iter().all(|c| *c == '#')
}

/// Formats `|value|` as a fraction.
pub(crate) fn render_fraction(value: f64, plan: &FractionPlan) -> (String, bool) {
    let x = value.abs();
    let has_int = !plan.int_digits.is_empty();
    let mut ip = if has_int { x.floor() } else { 0.0 };
    let f = x - ip;
    let max_den = 10f64.powi(plan.den_digits.len().max(1) as i32) - 1.0;
    let (mut n, d) = if let Some(den) = plan.fixed_denominator {
        ((f * den).round(), den)
    } else {
        approximate(f, max_den)
    };
    if has_int && n >= d {
        ip += 1.0;
        n = 0.0;
    }
    let zero = !has_int && n == 0.0;
    let blank = has_int && n == 0.0;
    let int_body = if ip > 0.0 {
        js_int_string(ip)
    } else if n == 0.0 {
        "0".into()
    } else {
        String::new()
    };
    let int_text = if has_int {
        pad_left(&int_body, &plan.int_digits)
    } else {
        String::new()
    };
    let num_hash = all_hash(&plan.num_digits);
    let hide_fraction =
        blank && num_hash && (plan.fixed_denominator.is_some() || all_hash(&plan.den_digits));
    let drop_separator = hide_fraction || (has_int && int_text.is_empty() && num_hash);
    let mut last_int: isize = -1;
    for (i, t) in plan.tokens.iter().enumerate() {
        if matches!(t, FracToken::Int { .. }) {
            last_int = i as isize;
        }
    }
    let first_num = plan
        .tokens
        .iter()
        .position(|t| matches!(t, FracToken::Num { .. }))
        .map(|i| i as isize)
        .unwrap_or(-1);
    let mut text = String::new();
    let mut int_done = false;
    for (i, tok) in plan.tokens.iter().enumerate() {
        let i = i as isize;
        match tok {
            FracToken::Lit(v) => {
                if !(drop_separator && i > last_int && i < first_num) {
                    text.push_str(v);
                }
            }
            FracToken::Int { .. } => {
                if !int_done {
                    text.push_str(&int_text);
                }
                int_done = true;
            }
            _ if hide_fraction => {}
            _ if blank => match tok {
                FracToken::Num { .. } | FracToken::Slash => text.push(' '),
                _ => {
                    let n = if plan.fixed_denominator.is_some() {
                        plan.den_width
                    } else {
                        1
                    };
                    for _ in 0..n {
                        text.push(' ');
                    }
                }
            },
            FracToken::Num { index } if *index == 0 => {
                text.push_str(&pad_left(&js_int_string(n), &plan.num_digits));
            }
            FracToken::Num { .. } => {}
            FracToken::Slash => text.push('/'),
            FracToken::Den { .. } if plan.fixed_denominator.is_some() => {
                text.push_str(&js_int_string(d));
            }
            FracToken::Den { index } if *index == 0 => {
                text.push_str(&pad_right(&js_int_string(d), &plan.den_digits));
            }
            FracToken::Den { .. } => {}
        }
    }
    (text, zero)
}
