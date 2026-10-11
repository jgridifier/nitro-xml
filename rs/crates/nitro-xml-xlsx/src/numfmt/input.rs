// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/numfmt/input.ts; changes listed in PROVENANCE.md.

use std::sync::LazyLock;

use regex::Regex;

use crate::numfmt::input_date::parse_date_time_input;
use crate::numfmt::types::{CellValue, FormatOptions};

/// What typing `text` into a cell produces.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedInput {
    pub value: CellValue,
    /// Number format Excel applies automatically (`0%`, `#,##0.00`, `m/d/yyyy`, ...).
    pub num_fmt: Option<String>,
    /// The formula without its leading `=`.
    pub formula: Option<String>,
}

const ERROR_CODES: &[&str] = &[
    "#NULL!",
    "#DIV/0!",
    "#VALUE!",
    "#REF!",
    "#NAME?",
    "#NUM!",
    "#N/A",
    "#GETTING_DATA",
    "#SPILL!",
    "#CALC!",
];

fn is_error_code(value: &str) -> bool {
    ERROR_CODES.contains(&value)
}

static NUMBER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:(\d{1,3}(?:,\d{3})+)|(\d*))(?:\.(\d*))?$").unwrap());
static LEAD_CURRENCY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([$€£])\s*").unwrap());
static EXP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[eE]([-+]?\d+)$").unwrap());
static FRACTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([-+]?)(\d+)\s+(\d+)/(\d+)$").unwrap());

pub(crate) struct NumberInput {
    pub value: f64,
    pub num_fmt: Option<String>,
}

fn currency_format(symbol: &str, decimals: bool) -> String {
    let body = if decimals { "#,##0.00" } else { "#,##0" };
    if symbol == "$" {
        format!("${body}_);[Red](${body})")
    } else {
        format!("{symbol}{body}")
    }
}

/// Strip a leading `+`/`-`. Returns `false` when parentheses already carry the sign (stays text).
fn take_sign(s: &mut String, negative: &mut bool, parenthesized: bool) -> bool {
    if s.starts_with('-') || s.starts_with('+') {
        // Excel keeps `(-5)`, `(+5)` and `(-$5)` as text: parentheses already carry the sign.
        if parenthesized {
            return false;
        }
        if s.starts_with('-') {
            *negative = !*negative;
        }
        *s = s[1..].trim().to_string();
    }
    true
}

/// Numbers with sign, parentheses, thousands separators, currency, percent or exponent.
pub(crate) fn parse_number_input(text: &str) -> Option<NumberInput> {
    let mut s = text.trim().to_string();
    let mut negative = false;
    let mut parenthesized = false;
    if s.starts_with('(') && s.ends_with(')') {
        negative = true;
        parenthesized = true;
        s = s[1..s.len() - 1].trim().to_string();
    }
    if !take_sign(&mut s, &mut negative, parenthesized) {
        return None;
    }
    let mut currency = String::new();
    if let Some(lead) = LEAD_CURRENCY.captures(&s) {
        currency = lead
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        s = s[lead.get(0).unwrap().end()..].to_string();
        if !take_sign(&mut s, &mut negative, parenthesized) {
            return None;
        }
    } else if let Some(last) = s.chars().last() {
        if last == '€' || last == '£' {
            currency = last.to_string();
            s = s[..s.len() - last.len_utf8()].trim_end().to_string();
        }
    }
    let mut percent = false;
    if currency.is_empty() && s.ends_with('%') {
        percent = true;
        s = s[..s.len() - 1].trim().to_string();
    }
    let mut exponent = String::new();
    if let Some(exp) = EXP_RE.captures(&s) {
        if currency.is_empty() && !percent {
            exponent = exp
                .get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            let full = exp.get(0).unwrap();
            s = s[..full.start()].to_string();
        }
    }
    let m = NUMBER_RE.captures(&s)?;
    if !s.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let grouped = m.get(1).is_some();
    if grouped && !exponent.is_empty() {
        return None;
    }
    let dec_group = m.get(3).map(|g| g.as_str());
    let decimals = dec_group.is_some_and(|d| !d.is_empty());
    let int_part = m
        .get(1)
        .or_else(|| m.get(2))
        .map(|g| g.as_str())
        .unwrap_or("")
        .replace(',', "");
    let int_part = if int_part.is_empty() {
        "0".to_string()
    } else {
        int_part
    };
    let frac = dec_group.unwrap_or("");
    let core = format!("{int_part}.{frac}0");
    let mut value = if exponent.is_empty() {
        js_number(&format!("{core}e0"))?
    } else {
        let exp_n: i32 = exponent.parse().ok()?;
        js_number(&format!("{core}e{exp_n}"))?
    };
    if percent {
        value = js_number(&format!("{core}e-2"))?;
    }
    if !value.is_finite() {
        return None;
    }
    if negative && value != 0.0 {
        value = -value;
    }
    if !exponent.is_empty() {
        return Some(NumberInput {
            value,
            num_fmt: Some("0.00E+00".into()),
        });
    }
    if percent {
        return Some(NumberInput {
            value,
            num_fmt: Some(if decimals { "0.00%" } else { "0%" }.into()),
        });
    }
    if !currency.is_empty() {
        return Some(NumberInput {
            value,
            num_fmt: Some(currency_format(&currency, decimals)),
        });
    }
    if grouped {
        return Some(NumberInput {
            value,
            num_fmt: Some(if decimals { "#,##0.00" } else { "#,##0" }.into()),
        });
    }
    Some(NumberInput {
        value,
        num_fmt: None,
    })
}

/// Parse a decimal literal the way JavaScript `Number(s)` does for our constructed strings.
fn js_number(s: &str) -> Option<f64> {
    s.parse::<f64>().ok()
}

fn parse_fraction_input(text: &str) -> Option<NumberInput> {
    let m = FRACTION_RE.captures(text.trim())?;
    let den: f64 = m[4].parse().ok()?;
    let num: f64 = m[3].parse().ok()?;
    if den == 0.0 || num >= den {
        return None;
    }
    let whole: f64 = m[2].parse().ok()?;
    let magnitude = whole + num / den;
    let value = if &m[1] == "-" { -magnitude } else { magnitude };
    Some(NumberInput {
        value,
        num_fmt: Some(if den < 10.0 { "# ?/?" } else { "# ??/??" }.into()),
    })
}

/// Interprets text typed into a cell like Excel's en-US entry: `=` starts a formula, a leading
/// apostrophe forces text, then booleans, error literals, numbers, fractions, dates and times.
pub fn parse_cell_input(text: &str, options: &FormatOptions<'_>) -> ParsedInput {
    if text.is_empty() {
        return ParsedInput {
            value: CellValue::Blank,
            num_fmt: None,
            formula: None,
        };
    }
    if let Some(formula) = text.strip_prefix('=') {
        if !formula.is_empty() {
            return ParsedInput {
                value: CellValue::Blank,
                num_fmt: None,
                formula: Some(formula.to_string()),
            };
        }
    }
    if let Some(forced) = text.strip_prefix('\'') {
        return ParsedInput {
            value: CellValue::Text(forced.to_string()),
            num_fmt: None,
            formula: None,
        };
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return ParsedInput {
            value: CellValue::Text(text.to_string()),
            num_fmt: None,
            formula: None,
        };
    }
    let upper = trimmed.to_ascii_uppercase();
    // Booleans and error literals must be typed without surrounding spaces (`TRUE ` stays text).
    if trimmed == text {
        if upper == "TRUE" || upper == "FALSE" {
            return ParsedInput {
                value: CellValue::Bool(upper == "TRUE"),
                num_fmt: None,
                formula: None,
            };
        }
        if is_error_code(&upper) {
            return ParsedInput {
                value: CellValue::Error(upper),
                num_fmt: None,
                formula: None,
            };
        }
    }
    let parsed = parse_number_input(trimmed)
        .or_else(|| parse_fraction_input(trimmed))
        .map(|n| (n.value, n.num_fmt))
        .or_else(|| {
            parse_date_time_input(trimmed, options.date1904).map(|d| (d.value, Some(d.num_fmt)))
        });
    match parsed {
        None => ParsedInput {
            value: CellValue::Text(text.to_string()),
            num_fmt: None,
            formula: None,
        },
        Some((value, num_fmt)) => ParsedInput {
            value: CellValue::Number(value),
            num_fmt,
            formula: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parenthesized_signed_stays_text() {
        // Catches: Excel keeps `(-5)` as text because parentheses already carry the sign.
        let got = parse_cell_input("(-5)", &FormatOptions::default());
        assert_eq!(got.value, CellValue::Text("(-5)".into()));
    }

    #[test]
    fn true_with_trailing_space_stays_text() {
        // Catches: booleans require an exact trim match; `TRUE ` is not boolean.
        let got = parse_cell_input("TRUE ", &FormatOptions::default());
        assert_eq!(got.value, CellValue::Text("TRUE ".into()));
    }
}
