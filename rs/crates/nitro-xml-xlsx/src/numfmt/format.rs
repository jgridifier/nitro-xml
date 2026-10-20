// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/numfmt/format.ts; changes listed in PROVENANCE.md.

use crate::numfmt::compile::{choose_section, compile_format, CompiledSection};
use crate::numfmt::date::{is_elapsed_only, render_date, render_elapsed_only};
use crate::numfmt::fraction::render_fraction;
use crate::numfmt::general::format_general;
use crate::numfmt::number::render_number;
use crate::numfmt::types::{
    AmpmKind, CellValue, DatePart, FormatOptions, FormattedValue, SectionKind, Token,
};

/// What Excel shows when a value does not fit its format (negative date, no matching section).
pub const OVERFLOW_TEXT: &str = "########";

fn result(text: String, section: &CompiledSection) -> FormattedValue {
    FormattedValue::new(text, section.color.clone())
}

fn date_source(part: DatePart) -> &'static str {
    match part {
        DatePart::Min => "m",
        DatePart::Mmin => "mm",
        DatePart::Y => "y",
        DatePart::Yyyy => "yyyy",
        DatePart::E => "e",
        DatePart::G => "g",
        DatePart::B => "b",
        DatePart::Bbbb => "bbbb",
        DatePart::M => "m",
        DatePart::Mm => "mm",
        DatePart::Mmm => "mmm",
        DatePart::Mmmm => "mmmm",
        DatePart::Mmmmm => "mmmmm",
        DatePart::D => "d",
        DatePart::Dd => "dd",
        DatePart::Ddd => "ddd",
        DatePart::Dddd => "dddd",
        DatePart::H => "h",
        DatePart::Hh => "hh",
        DatePart::S => "s",
        DatePart::Ss => "ss",
    }
}

fn token_source(tok: &Token) -> String {
    match tok {
        Token::Lit(v) => v.clone(),
        Token::Digit(c) => c.to_string(),
        Token::Num(v) => v.clone(),
        Token::Point => ".".into(),
        Token::Comma => ",".into(),
        Token::Percent => "%".into(),
        Token::Slash => "/".into(),
        Token::Exp { plus } => {
            if *plus {
                "E+".into()
            } else {
                "E-".into()
            }
        }
        Token::General => "General".into(),
        Token::Date(part) => date_source(*part).into(),
        Token::Ampm(AmpmKind::AmPm) => "AM/PM".into(),
        Token::Ampm(AmpmKind::Upper) => "A/P".into(),
        Token::Ampm(AmpmKind::Lower) => "a/p".into(),
        Token::Subsec { digits } => format!(".{}", "0".repeat(*digits)),
        Token::Elapsed { unit, width } => unit.to_string().repeat(*width),
        Token::Text => String::new(),
    }
}

fn format_text(text: &str, section: Option<&CompiledSection>) -> FormattedValue {
    let Some(section) = section else {
        return FormattedValue::new(text, None);
    };
    let mut out = String::new();
    for tok in &section.tokens {
        if matches!(tok, Token::Text) {
            out.push_str(text);
        } else {
            out.push_str(&token_source(tok));
        }
    }
    result(out, section)
}

fn render_abs(v: f64, section: &CompiledSection, general_width: Option<f64>) -> (String, bool) {
    if let Some(plan) = &section.fraction {
        return render_fraction(v, plan);
    }
    if let Some(plan) = &section.number {
        return render_number(v, plan);
    }
    let general = format_general(v.abs(), general_width);
    let zero = general.as_str() == "0";
    // Hot path: lone General (cells_1k) — return without a second String (#30).
    if section.tokens.len() == 1 && matches!(section.tokens[0], Token::General | Token::Text) {
        return (general, zero);
    }
    let mut text = String::new();
    for tok in &section.tokens {
        if matches!(tok, Token::General | Token::Text) {
            text.push_str(&general);
        } else {
            text.push_str(&token_source(tok));
        }
    }
    (text, zero)
}

fn measured(text: &str, options: &FormatOptions<'_>) -> f64 {
    match options.measure {
        Some(measure) => measure(text),
        None => text.chars().count() as f64,
    }
}

fn hash_fill(options: &FormatOptions<'_>) -> String {
    let width = options.width.unwrap_or(0.0);
    let one = match options.measure {
        Some(measure) => measure("#"),
        None => 1.0,
    };
    let n = if one > 0.0 { width / one } else { width };
    let n = if n.is_finite() { n.floor() } else { 1.0 };
    let count = if n < 1.0 {
        1
    } else if n > usize::MAX as f64 {
        usize::MAX
    } else {
        n as usize
    };
    "#".repeat(count)
}

fn format_number(
    value: f64,
    format: &str,
    date1904: bool,
    general_width: Option<f64>,
) -> FormattedValue {
    if !value.is_finite() {
        return FormattedValue::new("#NUM!", None);
    }
    let compiled = compile_format(format);
    let Some(choice) = choose_section(&compiled, value) else {
        return FormattedValue::new(OVERFLOW_TEXT, None);
    };
    let section = &choice.section;
    if section.kind == SectionKind::Date {
        let serial = if choice.abs { value.abs() } else { value };
        if is_elapsed_only(&section.tokens) {
            let text = render_elapsed_only(serial, &section.tokens);
            let text = if serial < 0.0 {
                format!("-{text}")
            } else {
                text
            };
            return result(text, section);
        }
        return match render_date(serial, &section.tokens, date1904) {
            Some(text) => result(text, section),
            None => FormattedValue::new(OVERFLOW_TEXT, None),
        };
    }
    let (text, zero) = render_abs(value, section, general_width);
    let minus = value < 0.0 && !choice.abs && !zero;
    let text = if minus { format!("-{text}") } else { text };
    result(text, section)
}

fn fit_number(value: f64, format: &str, options: &FormatOptions<'_>) -> FormattedValue {
    let date1904 = options.date1904;
    let width = options.width.unwrap_or(0.0);
    let fits = |text: &str| measured(text, options) <= width;
    let first = format_number(value, format, date1904, None);
    if fits(&first.text) || first.text == OVERFLOW_TEXT {
        return first;
    }
    let compiled = compile_format(format);
    let choice = choose_section(&compiled, value);
    let general = choice.as_ref().is_some_and(|choice| {
        choice.section.number.is_none()
            && choice.section.fraction.is_none()
            && choice
                .section
                .tokens
                .iter()
                .any(|t| matches!(t, Token::General))
    });
    if general {
        for w in (1..=10).rev() {
            let next = format_number(value, format, date1904, Some(f64::from(w)));
            if fits(&next.text) {
                return next;
            }
        }
    }
    let fill = hash_fill(options);
    FormattedValue::new(fill, first.color)
}

/// Formats a cell value with an Excel number format code.
///
/// Fill characters (`*x`) are dropped and padding (`_x`) becomes one space. With `options.width`,
/// numbers are fitted the way Excel fits them.
pub fn format_value(
    value: &CellValue,
    format: &str,
    options: &FormatOptions<'_>,
) -> FormattedValue {
    match value {
        CellValue::Blank => FormattedValue::new("", None),
        CellValue::Bool(true) => FormattedValue::new("TRUE", None),
        CellValue::Bool(false) => FormattedValue::new("FALSE", None),
        CellValue::Text(text) => format_text(text, compile_format(format).text.as_ref()),
        CellValue::Error(error) => FormattedValue::new(error.clone(), None),
        CellValue::Number(n) => {
            if options.width.is_some() {
                fit_number(*n, format, options)
            } else {
                format_number(*n, format, options.date1904, None)
            }
        }
    }
}

/// True when the format shows numbers as dates or times.
pub fn is_date_format(format: &str) -> bool {
    compile_format(format).is_date
}
