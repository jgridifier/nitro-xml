// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/text.ts, text-case.ts,
// text-convert.ts, text-extra.ts and text-regex.ts; changes listed in PROVENANCE.md.
//! Text functions, from `ARRAYTOTEXT` through `VALUETOTEXT`.
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]

use std::sync::OnceLock;

use regex::{Captures, Regex, RegexBuilder};

use crate::numfmt::{format_value, CellValue, FormatOptions};

use super::coerce::{to_bool, to_text};
use super::helpers::{int_of, scalar_of, str_of};
use super::host::CallCtx;
use super::spec::{gen_spec, FunctionSpec, ParamKind};
use super::text_num::parse_number_text;
use super::value::{ErrorCode, Fx, Matrix, Scalar, Value};

const C: &str = "Text";
const MAX_TEXT: usize = 32_767;
const ANY: &[ParamKind] = &[ParamKind::Any];

pub fn specs() -> Vec<FunctionSpec> {
    let mut out = Vec::new();
    out.extend(convert_specs());
    out.extend(core_specs());
    out.extend(extra_specs());
    out
}

fn convert_specs() -> Vec<FunctionSpec> {
    vec![
        gen_spec(
            "TEXT",
            C,
            "TEXT(value, format_text)",
            "Formats a value with a number format.",
            2,
            2,
            &[],
            false,
            text_fn,
        ),
        gen_spec(
            "VALUE",
            C,
            "VALUE(text)",
            "Converts text to a number.",
            1,
            1,
            &[],
            false,
            value_fn,
        ),
        gen_spec(
            "NUMBERVALUE",
            C,
            "NUMBERVALUE(text, [decimal_separator], [group_separator])",
            "Converts text to a number with given separators.",
            1,
            3,
            &[],
            false,
            number_value,
        ),
        gen_spec(
            "FIXED",
            C,
            "FIXED(number, [decimals], [no_commas])",
            "Formats a number as text with fixed decimals.",
            1,
            3,
            &[],
            false,
            fixed_fn,
        ),
        gen_spec(
            "DOLLAR",
            C,
            "DOLLAR(number, [decimals])",
            "Formats a number as currency text.",
            1,
            2,
            &[],
            false,
            dollar_fn,
        ),
        gen_spec(
            "CHAR",
            C,
            "CHAR(number)",
            "The character for a code (Windows-1252).",
            1,
            1,
            &[],
            false,
            char_fn,
        ),
        gen_spec(
            "CODE",
            C,
            "CODE(text)",
            "The code of the first character (Windows-1252).",
            1,
            1,
            &[],
            false,
            code_fn,
        ),
        gen_spec(
            "UNICHAR",
            C,
            "UNICHAR(number)",
            "The Unicode character for a code point.",
            1,
            1,
            &[],
            false,
            unichar_fn,
        ),
        gen_spec(
            "UNICODE",
            C,
            "UNICODE(text)",
            "The code point of the first character.",
            1,
            1,
            &[],
            false,
            unicode_fn,
        ),
        gen_spec(
            "T",
            C,
            "T(value)",
            "The text of a value, or empty text.",
            1,
            1,
            &[],
            false,
            t_fn,
        ),
    ]
}

fn core_specs() -> Vec<FunctionSpec> {
    let mut out = vec![
        gen_spec(
            "CONCATENATE",
            C,
            "CONCATENATE(text1, [text2], ...)",
            "Joins text items.",
            1,
            255,
            &[],
            false,
            concatenate_fn,
        ),
        gen_spec(
            "CONCAT",
            C,
            "CONCAT(text1, [text2], ...)",
            "Joins text from items and ranges.",
            1,
            255,
            ANY,
            false,
            concat_fn,
        ),
        gen_spec(
            "TEXTJOIN",
            C,
            "TEXTJOIN(delimiter, ignore_empty, text1, ...)",
            "Joins text with a delimiter.",
            3,
            255,
            ANY,
            false,
            text_join,
        ),
    ];
    for name in ["LEFT", "LEFTB"] {
        out.push(gen_spec(
            name,
            C,
            "LEFT(text, [num_chars])",
            "The first characters of a text.",
            1,
            2,
            &[],
            false,
            left_fn,
        ));
    }
    for name in ["RIGHT", "RIGHTB"] {
        out.push(gen_spec(
            name,
            C,
            "RIGHT(text, [num_chars])",
            "The last characters of a text.",
            1,
            2,
            &[],
            false,
            right_fn,
        ));
    }
    for name in ["MID", "MIDB"] {
        out.push(gen_spec(
            name,
            C,
            "MID(text, start_num, num_chars)",
            "Characters from the middle of a text.",
            3,
            3,
            &[],
            false,
            mid_fn,
        ));
    }
    for name in ["LEN", "LENB"] {
        out.push(gen_spec(
            name,
            C,
            "LEN(text)",
            "The number of characters in a text.",
            1,
            1,
            &[],
            false,
            len_fn,
        ));
    }
    out.push(gen_spec(
        "LOWER",
        C,
        "LOWER(text)",
        "Converts text to lower case.",
        1,
        1,
        &[],
        false,
        |ctx, args| Ok(Value::Scalar(Scalar::Text(lower(&str_arg(ctx, args)?)))),
    ));
    out.push(gen_spec(
        "UPPER",
        C,
        "UPPER(text)",
        "Converts text to upper case.",
        1,
        1,
        &[],
        false,
        |ctx, args| Ok(Value::Scalar(Scalar::Text(upper(&str_arg(ctx, args)?)))),
    ));
    out.push(gen_spec(
        "PROPER",
        C,
        "PROPER(text)",
        "Capitalizes the first letter of each word.",
        1,
        1,
        &[],
        false,
        |ctx, args| Ok(Value::Scalar(Scalar::Text(proper(&str_arg(ctx, args)?)))),
    ));
    out.push(gen_spec(
        "TRIM",
        C,
        "TRIM(text)",
        "Removes extra spaces.",
        1,
        1,
        &[],
        false,
        |ctx, args| {
            Ok(Value::Scalar(Scalar::Text(trim_excel(&str_arg(
                ctx, args,
            )?))))
        },
    ));
    out.push(gen_spec(
        "CLEAN",
        C,
        "CLEAN(text)",
        "Removes non-printable characters.",
        1,
        1,
        &[],
        false,
        |ctx, args| Ok(Value::Scalar(Scalar::Text(clean(&str_arg(ctx, args)?)))),
    ));
    out.push(gen_spec(
        "SUBSTITUTE",
        C,
        "SUBSTITUTE(text, old_text, new_text, [instance_num])",
        "Replaces occurrences of a text.",
        3,
        4,
        &[],
        false,
        substitute_fn,
    ));
    for name in ["REPLACE", "REPLACEB"] {
        out.push(gen_spec(
            name,
            C,
            "REPLACE(old_text, start_num, num_chars, new_text)",
            "Replaces part of a text.",
            4,
            4,
            &[],
            false,
            replace_fn,
        ));
    }
    for name in ["FIND", "FINDB"] {
        out.push(gen_spec(
            name,
            C,
            "FIND(find_text, within_text, [start_num])",
            "The position of a text (case-sensitive).",
            2,
            3,
            &[],
            false,
            |ctx, args| find_fn(ctx, args, false),
        ));
    }
    for name in ["SEARCH", "SEARCHB"] {
        out.push(gen_spec(
            name,
            C,
            "SEARCH(find_text, within_text, [start_num])",
            "The position of a text (wildcards, any case).",
            2,
            3,
            &[],
            false,
            |ctx, args| find_fn(ctx, args, true),
        ));
    }
    out.push(gen_spec(
        "REPT",
        C,
        "REPT(text, number_times)",
        "Repeats a text.",
        2,
        2,
        &[],
        false,
        rept_fn,
    ));
    out.push(gen_spec(
        "EXACT",
        C,
        "EXACT(text1, text2)",
        "Whether two texts are identical (case-sensitive).",
        2,
        2,
        &[],
        false,
        |ctx, args| {
            Ok(Value::Scalar(Scalar::Bool(
                str_of(ctx, &args[0])? == str_of(ctx, &args[1])?,
            )))
        },
    ));
    // The B-variants share implementations; syntax strings above are representative.
    let _ = C;
    out
}

fn extra_specs() -> Vec<FunctionSpec> {
    vec![
        gen_spec("ENCODEURL", C, "ENCODEURL(text)", "Percent-encodes text for use in a URL.", 1, 1, &[], false, |ctx, args| {
            Ok(Value::Scalar(Scalar::Text(encode_url(&str_of(ctx, &args[0])?))))
        }),
        gen_spec(
            "REGEXTEST",
            C,
            "REGEXTEST(text, pattern, [case_sensitivity])",
            "Whether any part of the text matches the pattern.",
            2,
            3,
            &[],
            false,
            regex_test,
        ),
        gen_spec(
            "REGEXEXTRACT",
            C,
            "REGEXEXTRACT(text, pattern, [return_mode], [case_sensitivity])",
            "The first part of the text that matches the pattern.",
            2,
            4,
            &[],
            false,
            regex_extract,
        ),
        gen_spec(
            "REGEXREPLACE",
            C,
            "REGEXREPLACE(text, pattern, replacement, [occurrence], [case_sensitivity])",
            "Replaces the parts of the text that match the pattern.",
            3,
            5,
            &[],
            false,
            regex_replace,
        ),
        gen_spec(
            "TEXTBEFORE",
            C,
            "TEXTBEFORE(text, delimiter, [instance_num], [match_mode], [match_end], [if_not_found])",
            "The text before a delimiter.",
            2,
            6,
            &[ParamKind::Value, ParamKind::Any, ParamKind::Value],
            false,
            |ctx, args| text_around(ctx, args, true),
        ),
        gen_spec(
            "TEXTAFTER",
            C,
            "TEXTAFTER(text, delimiter, [instance_num], [match_mode], [match_end], [if_not_found])",
            "The text after a delimiter.",
            2,
            6,
            &[ParamKind::Value, ParamKind::Any, ParamKind::Value],
            false,
            |ctx, args| text_around(ctx, args, false),
        ),
        gen_spec(
            "TEXTSPLIT",
            C,
            "TEXTSPLIT(text, col_delimiter, [row_delimiter], [ignore_empty], [match_mode], [pad_with])",
            "Splits text into an array.",
            2,
            6,
            ANY,
            false,
            text_split,
        ),
        gen_spec("VALUETOTEXT", C, "VALUETOTEXT(value, [format])", "Text for any value.", 1, 2, &[], false, value_to_text_fn),
        gen_spec("ARRAYTOTEXT", C, "ARRAYTOTEXT(array, [format])", "Text for an array.", 1, 2, ANY, false, array_to_text),
    ]
}

fn str_arg(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<String> {
    str_of(ctx, args.first().unwrap_or(&Value::blank()))
}

fn opt_num(ctx: &CallCtx<'_>, args: &[Value], index: usize, fallback: f64) -> Fx<f64> {
    if index < args.len() {
        super::helpers::num(ctx, &args[index])
    } else {
        Ok(fallback)
    }
}

fn limit(text: String) -> Fx<Value> {
    if utf16_len(&text) > MAX_TEXT {
        Err(ErrorCode::Value)
    } else {
        Ok(Value::Scalar(Scalar::Text(text)))
    }
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

fn utf16_units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn from_units(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

fn is_high(unit: u16) -> bool {
    (0xD800..=0xDBFF).contains(&unit)
}

fn is_low(unit: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&unit)
}

fn left_units(text: &str, n: usize) -> String {
    let units = utf16_units(text);
    let mut end = n.min(units.len());
    if end > 0 && end < units.len() && is_high(units[end - 1]) {
        end += 1;
    }
    from_units(&units[..end])
}

fn right_units(text: &str, n: usize) -> String {
    if n == 0 {
        return String::new();
    }
    let units = utf16_units(text);
    let mut start = units.len().saturating_sub(n);
    if start > 0 && is_low(units[start]) {
        start -= 1;
    }
    from_units(&units[start..])
}

fn mid_units(text: &str, start: usize, n: usize) -> String {
    let units = utf16_units(text);
    if start >= units.len() {
        return String::new();
    }
    let end = (start + n).min(units.len());
    from_units(&units[start..end])
}

fn index_of(hay: &str, needle: &str, from: usize) -> Option<usize> {
    let h = utf16_units(hay);
    let n = utf16_units(needle);
    if n.is_empty() {
        return Some(from);
    }
    if from > h.len() || n.len() > h.len().saturating_sub(from) {
        return None;
    }
    h[from..]
        .windows(n.len())
        .position(|window| window == n.as_slice())
        .map(|pos| pos + from)
}

fn is_letter(ch: char) -> bool {
    let low: String = ch.to_lowercase().collect();
    let up: String = ch.to_uppercase().collect();
    low != up
}

fn upper_char(ch: char) -> String {
    let up: String = ch.to_uppercase().collect();
    if up.encode_utf16().count() == ch.len_utf16() || up.chars().count() == 1 {
        up
    } else {
        ch.to_string()
    }
}

fn lower_char(ch: char, before: Option<char>, after: Option<char>) -> String {
    if ch == 'Σ' {
        let final_sigma = before.is_some_and(is_letter) && !after.is_some_and(is_letter);
        return if final_sigma { "ς" } else { "σ" }.to_owned();
    }
    let low: String = ch.to_lowercase().collect();
    let mut chars = low.chars();
    match (chars.next(), chars.next()) {
        (Some(one), None) => one.to_string(),
        (Some(one), Some(_)) => one.to_string(),
        _ => ch.to_string(),
    }
}

fn map_chars(
    text: &str,
    mut func: impl FnMut(char, Option<char>, Option<char>) -> String,
) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    for (index, ch) in chars.iter().copied().enumerate() {
        let before = if index > 0 {
            chars.get(index - 1).copied()
        } else {
            None
        };
        let after = chars.get(index + 1).copied();
        out.push_str(&func(ch, before, after));
    }
    out
}

fn upper(text: &str) -> String {
    map_chars(text, |ch, _, _| upper_char(ch))
}

fn lower(text: &str) -> String {
    map_chars(text, lower_char)
}

fn proper(text: &str) -> String {
    map_chars(text, |ch, before, after| {
        if !is_letter(ch) {
            return ch.to_string();
        }
        if before.is_some_and(is_letter) {
            lower_char(ch, before, after)
        } else {
            upper_char(ch)
        }
    })
}

fn trim_excel(text: &str) -> String {
    let collapsed = space_run().replace_all(text, " ");
    let mut out = collapsed.into_owned();
    if out.starts_with(' ') {
        out.remove(0);
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

fn space_run() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(" +").expect("spaces"))
}

fn clean(text: &str) -> String {
    text.chars().filter(|ch| (*ch as u32) > 0x1f).collect()
}

fn text_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let mut scalar = scalar_of(&args[0])?;
    if let Scalar::Error(code) = scalar {
        return Ok(Value::error(code));
    }
    if let Scalar::Text(text) = &scalar {
        if let Some(n) = parse_number_text(text, ctx.date1904) {
            scalar = Scalar::Number(n);
        }
    }
    let cell = match scalar {
        Scalar::Bool(true) => CellValue::Text("TRUE".to_owned()),
        Scalar::Bool(false) => CellValue::Text("FALSE".to_owned()),
        Scalar::Number(n) => CellValue::Number(n),
        Scalar::Text(text) => CellValue::Text(text),
        Scalar::Blank => CellValue::Blank,
        Scalar::Error(code) => return Ok(Value::error(code)),
    };
    let format = str_of(ctx, &args[1])?;
    let formatted = format_value(
        &cell,
        &format,
        &FormatOptions {
            date1904: ctx.date1904,
            ..FormatOptions::default()
        },
    );
    Ok(Value::Scalar(Scalar::Text(formatted.text)))
}

fn value_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let scalar = scalar_of(&args[0])?;
    match scalar {
        Scalar::Number(n) => Ok(Value::number(n)),
        Scalar::Blank => Ok(Value::number(0.0)),
        Scalar::Bool(_) => Err(ErrorCode::Value),
        Scalar::Error(code) => Ok(Value::error(code)),
        Scalar::Text(text) => match parse_number_text(&text, ctx.date1904) {
            Some(n) => Ok(Value::number(n)),
            None => Err(ErrorCode::Value),
        },
    }
}

fn number_value(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let decimal = if args.len() > 1 {
        str_of(ctx, &args[1])?.chars().next().unwrap_or('.')
    } else {
        '.'
    };
    let group = if args.len() > 2 {
        str_of(ctx, &args[2])?.chars().next().unwrap_or(',')
    } else {
        ','
    };
    let mut text: String = str_of(ctx, &args[0])?
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    if text.is_empty() {
        return Ok(Value::number(0.0));
    }
    let mut percent = 0i32;
    while text.ends_with('%') {
        percent += 1;
        text.pop();
    }
    if let Some(at) = text.find(decimal) {
        if text[at + decimal.len_utf8()..].contains(group) {
            return Err(ErrorCode::Value);
        }
    }
    text = text.replace(group, "");
    if decimal != '.' {
        text = text.replace(decimal, ".");
    }
    let value: f64 = text.parse().unwrap_or(f64::NAN);
    if !number_value_re().is_match(&text) || !value.is_finite() {
        return Err(ErrorCode::Value);
    }
    Ok(Value::number(value / 100f64.powi(percent)))
}

fn number_value_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^[+-]?(\d+\.?\d*|\.\d+)(e[+-]?\d+)?$").expect("numbervalue"))
}

fn fixed_text(value: f64, decimals: f64, commas: bool) -> String {
    let d = decimals.trunc();
    let rounded = super::math::round_half_digits(value, d);
    let places = d.clamp(0.0, 99.0) as usize;
    let abs = rounded.abs();
    let mut text = format!("{abs:.places$}");
    if commas {
        let (int_part, frac) = match text.split_once('.') {
            Some((int_part, frac)) => (int_part.to_owned(), Some(frac.to_owned())),
            None => (text.clone(), None),
        };
        let mut grouped = String::new();
        for (index, ch) in int_part.chars().rev().enumerate() {
            if index > 0 && index.is_multiple_of(3) {
                grouped.push(',');
            }
            grouped.push(ch);
        }
        let int_part: String = grouped.chars().rev().collect();
        text = match frac {
            Some(frac) => format!("{int_part}.{frac}"),
            None => int_part,
        };
    }
    if rounded < 0.0 {
        format!("-{text}")
    } else {
        text
    }
}

fn fixed_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let commas = !(args.len() > 2 && to_bool(&scalar_of(&args[2])?)?);
    Ok(Value::Scalar(Scalar::Text(fixed_text(
        super::helpers::num(ctx, &args[0])?,
        opt_num(ctx, args, 1, 2.0)?,
        commas,
    ))))
}

fn dollar_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let value = super::helpers::num(ctx, &args[0])?;
    let text = fixed_text(value.abs(), opt_num(ctx, args, 1, 2.0)?, true);
    let stripped: String = text
        .chars()
        .filter(|ch| !matches!(ch, '0' | '.' | ','))
        .collect();
    let shown = if value < 0.0 && !stripped.is_empty() {
        format!("(${text})")
    } else {
        format!("${text}")
    };
    Ok(Value::Scalar(Scalar::Text(shown)))
}

fn cp1252(code: u32) -> u32 {
    match code {
        128 => 0x20AC,
        130 => 0x201A,
        131 => 0x0192,
        132 => 0x201E,
        133 => 0x2026,
        134 => 0x2020,
        135 => 0x2021,
        136 => 0x02C6,
        137 => 0x2030,
        138 => 0x0160,
        139 => 0x2039,
        140 => 0x0152,
        142 => 0x017D,
        145 => 0x2018,
        146 => 0x2019,
        147 => 0x201C,
        148 => 0x201D,
        149 => 0x2022,
        150 => 0x2013,
        151 => 0x2014,
        152 => 0x02DC,
        153 => 0x2122,
        154 => 0x0161,
        155 => 0x203A,
        156 => 0x0153,
        158 => 0x017E,
        159 => 0x0178,
        other => other,
    }
}

fn cp1252_reverse(code: u32) -> Option<u32> {
    Some(match code {
        0x20AC => 128,
        0x201A => 130,
        0x0192 => 131,
        0x201E => 132,
        0x2026 => 133,
        0x2020 => 134,
        0x2021 => 135,
        0x02C6 => 136,
        0x2030 => 137,
        0x0160 => 138,
        0x2039 => 139,
        0x0152 => 140,
        0x017D => 142,
        0x2018 => 145,
        0x2019 => 146,
        0x201C => 147,
        0x201D => 148,
        0x2022 => 149,
        0x2013 => 150,
        0x2014 => 151,
        0x02DC => 152,
        0x2122 => 153,
        0x0161 => 154,
        0x203A => 155,
        0x0153 => 156,
        0x017E => 158,
        0x0178 => 159,
        _ => return None,
    })
}

fn char_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let n = int_of(ctx, &args[0])?;
    if !(1.0..=255.0).contains(&n) {
        return Err(ErrorCode::Value);
    }
    let code = cp1252(n as u32);
    let ch = char::from_u32(code).unwrap_or('\u{3F}');
    Ok(Value::Scalar(Scalar::Text(ch.to_string())))
}

fn code_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let text = str_of(ctx, &args[0])?;
    let Some(unit) = text.encode_utf16().next() else {
        return Err(ErrorCode::Value);
    };
    let code = u32::from(unit);
    let mapped = cp1252_reverse(code).unwrap_or(if code <= 255 { code } else { 63 });
    Ok(Value::number(f64::from(mapped)))
}

fn unichar_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let n = int_of(ctx, &args[0])?;
    if n < 1.0 || n > f64::from(0x10_FFFF) {
        return Err(ErrorCode::Value);
    }
    let Some(ch) = char::from_u32(n as u32) else {
        return Err(ErrorCode::Value);
    };
    Ok(Value::Scalar(Scalar::Text(ch.to_string())))
}

fn unicode_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let text = str_of(ctx, &args[0])?;
    let Some(ch) = text.chars().next() else {
        return Err(ErrorCode::Value);
    };
    Ok(Value::number(f64::from(ch as u32)))
}

fn t_fn(_ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let scalar = scalar_of(args.first().unwrap_or(&Value::blank()))?;
    match scalar {
        Scalar::Error(code) => Ok(Value::error(code)),
        Scalar::Text(text) => Ok(Value::Scalar(Scalar::Text(text))),
        _ => Ok(Value::Scalar(Scalar::Text(String::new()))),
    }
}

fn concatenate_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let mut out = String::new();
    for arg in args {
        out.push_str(&str_of(ctx, arg)?);
    }
    limit(out)
}

fn join_values(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Vec<String>> {
    let mut out = Vec::new();
    for arg in args {
        for value in ctx.to_matrix(arg)?.flat()? {
            if let Scalar::Error(code) = value {
                return Err(code);
            }
            out.push(to_text(&value)?);
        }
    }
    Ok(out)
}

fn concat_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    limit(join_values(ctx, args)?.concat())
}

fn text_join(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let delimiters = ctx
        .to_matrix(&args[0])?
        .flat()?
        .into_iter()
        .map(|value| {
            if let Scalar::Error(code) = value {
                Err(code)
            } else {
                to_text(&value)
            }
        })
        .collect::<Fx<Vec<_>>>()?;
    let ignore = to_bool(&ctx.to_scalar(&args[1]))?;
    let parts: Vec<String> = join_values(ctx, &args[2..])?
        .into_iter()
        .filter(|part| !ignore || !part.is_empty())
        .collect();
    let mut out = String::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 && !delimiters.is_empty() {
            out.push_str(&delimiters[(index - 1) % delimiters.len()]);
        }
        out.push_str(part);
    }
    limit(out)
}

fn left_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let n = opt_num(ctx, args, 1, 1.0)?;
    if n < 0.0 {
        return Err(ErrorCode::Value);
    }
    Ok(Value::Scalar(Scalar::Text(left_units(
        &str_of(ctx, &args[0])?,
        n.trunc() as usize,
    ))))
}

fn right_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let n = opt_num(ctx, args, 1, 1.0)?.trunc();
    if n < 0.0 {
        return Err(ErrorCode::Value);
    }
    Ok(Value::Scalar(Scalar::Text(right_units(
        &str_of(ctx, &args[0])?,
        n as usize,
    ))))
}

fn mid_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let start = super::helpers::num(ctx, &args[1])?.trunc();
    let n = super::helpers::num(ctx, &args[2])?.trunc();
    if start < 1.0 || n < 0.0 {
        return Err(ErrorCode::Value);
    }
    Ok(Value::Scalar(Scalar::Text(mid_units(
        &str_of(ctx, &args[0])?,
        (start as usize).saturating_sub(1),
        n as usize,
    ))))
}

fn len_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    Ok(Value::number(utf16_len(&str_of(ctx, &args[0])?) as f64))
}

fn substitute_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let text = str_of(ctx, &args[0])?;
    let old = str_of(ctx, &args[1])?;
    let new = str_of(ctx, &args[2])?;
    if old.is_empty() {
        return Ok(Value::Scalar(Scalar::Text(text)));
    }
    if args.len() < 4 {
        return limit(text.split(&old).collect::<Vec<_>>().join(&new));
    }
    let instance = super::helpers::num(ctx, &args[3])?.trunc();
    if instance < 1.0 {
        return Err(ErrorCode::Value);
    }
    let Some(at) = nth_index(&text, &old, instance as usize) else {
        return Ok(Value::Scalar(Scalar::Text(text)));
    };
    let units = utf16_units(&text);
    let old_units = utf16_len(&old);
    let prefix = from_units(&units[..at]);
    let suffix = from_units(&units[at + old_units..]);
    limit(format!("{prefix}{new}{suffix}"))
}

fn nth_index(text: &str, needle: &str, instance: usize) -> Option<usize> {
    let mut from = 0;
    let mut found = 0;
    for _ in 0..instance {
        found = index_of(text, needle, from)?;
        from = found + utf16_len(needle).max(1);
    }
    Some(found)
}

fn replace_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let text = str_of(ctx, &args[0])?;
    let start = super::helpers::num(ctx, &args[1])?.trunc();
    let n = super::helpers::num(ctx, &args[2])?.trunc();
    if start < 1.0 || n < 0.0 {
        return Err(ErrorCode::Value);
    }
    let units = utf16_units(&text);
    let start = (start as usize).saturating_sub(1);
    let end = (start + n as usize).min(units.len());
    let prefix = if start >= units.len() {
        text.clone()
    } else {
        from_units(&units[..start.min(units.len())])
    };
    let suffix = if start >= units.len() {
        String::new()
    } else {
        from_units(&units[end..])
    };
    limit(format!("{prefix}{}{suffix}", str_of(ctx, &args[3])?))
}

fn find_fn(ctx: &CallCtx<'_>, args: &[Value], insensitive: bool) -> Fx<Value> {
    let needle = str_of(ctx, &args[0])?;
    let hay = str_of(ctx, &args[1])?;
    let start = opt_num(ctx, args, 2, 1.0)?;
    let hay_len = utf16_len(&hay);
    let extra = if insensitive { 0.0 } else { 1.0 };
    if start < 1.0 || start > hay_len as f64 + extra {
        return Err(ErrorCode::Value);
    }
    let from = start.trunc() as usize - 1;
    if needle.is_empty() {
        return Ok(Value::number(from as f64 + 1.0));
    }
    let at = if insensitive {
        search_wildcard(&hay, &needle, from)?
    } else {
        index_of(&hay, &needle, from)
    };
    match at {
        Some(index) => Ok(Value::number(index as f64 + 1.0)),
        None => Err(ErrorCode::Value),
    }
}

fn search_wildcard(hay: &str, needle: &str, from: usize) -> Fx<Option<usize>> {
    let units = utf16_units(hay);
    if from > units.len() {
        return Ok(None);
    }
    let slice = from_units(&units[from..]);
    let regex = wildcard_regex(needle)?;
    Ok(regex
        .find(&slice)
        .map(|found| from + utf16_len(&slice[..found.start()])))
}

fn wildcard_regex(pattern: &str) -> Fx<Regex> {
    let mut source = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '~' && index + 1 < chars.len() {
            source.push_str(&regex::escape(&chars[index + 1].to_string()));
            index += 2;
            continue;
        } else if ch == '*' {
            source.push_str(r"[\s\S]*");
        } else if ch == '?' {
            source.push_str(r"[\s\S]");
        } else {
            source.push_str(&regex::escape(&ch.to_string()));
        }
        index += 1;
    }
    RegexBuilder::new(&source)
        .case_insensitive(true)
        .build()
        .map_err(|_| ErrorCode::Value)
}

fn rept_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let text = str_of(ctx, &args[0])?;
    let n = super::helpers::num(ctx, &args[1])?.trunc();
    if n < 0.0 || utf16_len(&text) as f64 * n > MAX_TEXT as f64 {
        return Err(ErrorCode::Value);
    }
    if text.is_empty() || n == 0.0 {
        return Ok(Value::Scalar(Scalar::Text(String::new())));
    }
    Ok(Value::Scalar(Scalar::Text(text.repeat(n as usize))))
}

fn compile_regex(pattern: &str, case_mode: f64) -> Fx<Regex> {
    if case_mode != 0.0 && case_mode != 1.0 {
        return Err(ErrorCode::Value);
    }
    RegexBuilder::new(pattern)
        .case_insensitive(case_mode == 1.0)
        .build()
        .map_err(|_| ErrorCode::Value)
}

fn regex_test(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let regex = compile_regex(&str_of(ctx, &args[1])?, opt_num(ctx, args, 2, 0.0)?)?;
    Ok(Value::Scalar(Scalar::Bool(
        regex.is_match(&str_of(ctx, &args[0])?),
    )))
}

fn regex_extract(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    if opt_num(ctx, args, 2, 0.0)? != 0.0 {
        return Err(ErrorCode::Value);
    }
    let regex = compile_regex(&str_of(ctx, &args[1])?, opt_num(ctx, args, 3, 0.0)?)?;
    match regex.find(&str_of(ctx, &args[0])?) {
        Some(found) => Ok(Value::Scalar(Scalar::Text(found.as_str().to_owned()))),
        None => Err(ErrorCode::Na),
    }
}

fn regex_replace(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let text = str_of(ctx, &args[0])?;
    let regex = compile_regex(&str_of(ctx, &args[1])?, opt_num(ctx, args, 4, 0.0)?)?;
    let replacement = str_of(ctx, &args[2])?;
    let occurrence = if args.len() > 3 && !matches!(args[3], Value::Scalar(Scalar::Blank)) {
        int_of(ctx, &args[3])?
    } else {
        0.0
    };
    if occurrence == 0.0 {
        let replaced = regex.replace_all(&text, |caps: &Captures| {
            expand_caps(&text, caps, &replacement)
        });
        return Ok(Value::Scalar(Scalar::Text(replaced.into_owned())));
    }
    let hits: Vec<(usize, usize)> = regex
        .find_iter(&text)
        .map(|m| (m.start(), m.end()))
        .collect();
    let index = if occurrence > 0.0 {
        occurrence as isize - 1
    } else {
        hits.len() as isize + occurrence as isize
    };
    let Some(&(start, end)) = (index >= 0)
        .then_some(index as usize)
        .and_then(|i| hits.get(i))
    else {
        return Ok(Value::Scalar(Scalar::Text(text)));
    };
    let matched = &text[start..end];
    let inner = regex.replace(matched, |caps: &Captures| {
        expand_caps(matched, caps, &replacement)
    });
    Ok(Value::Scalar(Scalar::Text(format!(
        "{}{inner}{}",
        &text[..start],
        &text[end..]
    ))))
}

fn expand_caps(whole: &str, caps: &Captures<'_>, replacement: &str) -> String {
    let chars: Vec<char> = replacement.chars().collect();
    let mut out = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '$' && index + 1 < chars.len() {
            match chars[index + 1] {
                '$' => {
                    out.push('$');
                    index += 2;
                }
                '&' => {
                    out.push_str(caps.get(0).map(|m| m.as_str()).unwrap_or(""));
                    index += 2;
                }
                '`' => {
                    let start = caps.get(0).map(|m| m.start()).unwrap_or(0);
                    out.push_str(&whole[..start]);
                    index += 2;
                }
                '\'' => {
                    let end = caps.get(0).map(|m| m.end()).unwrap_or(0);
                    out.push_str(&whole[end..]);
                    index += 2;
                }
                digit if digit.is_ascii_digit() => {
                    let mut n = usize::from(digit as u8 - b'0');
                    let mut consumed = 2;
                    if index + 2 < chars.len() && chars[index + 2].is_ascii_digit() {
                        let n2 = n * 10 + usize::from(chars[index + 2] as u8 - b'0');
                        if caps.get(n2).is_some() {
                            n = n2;
                            consumed = 3;
                        }
                    }
                    if let Some(group) = caps.get(n) {
                        out.push_str(group.as_str());
                    }
                    index += consumed;
                }
                _ => {
                    out.push('$');
                    index += 1;
                }
            }
        } else {
            out.push(chars[index]);
            index += 1;
        }
    }
    out
}

fn encode_url(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~') {
            out.push(ch);
        } else {
            let mut buf = [0u8; 4];
            for byte in ch.encode_utf8(&mut buf).as_bytes() {
                out.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    out
}

struct Hit {
    at: usize,
    length: usize,
}

fn occurrences(text: &str, delimiters: &[String], insensitive: bool) -> Vec<Hit> {
    let hay = if insensitive {
        text.to_lowercase()
    } else {
        text.to_owned()
    };
    let needles: Vec<String> = delimiters
        .iter()
        .map(|item| {
            if insensitive {
                item.to_lowercase()
            } else {
                item.clone()
            }
        })
        .collect();
    let hay_units = utf16_units(&hay);
    let mut out = Vec::new();
    let mut index = 0usize;
    while index <= hay_units.len() {
        let mut best: Option<Hit> = None;
        for needle in &needles {
            if needle.is_empty() {
                continue;
            }
            if let Some(at) = index_of(&from_units(&hay_units), needle, index) {
                let length = utf16_len(needle);
                let better = best
                    .as_ref()
                    .is_none_or(|hit| at < hit.at || (at == hit.at && length > hit.length));
                if better {
                    best = Some(Hit { at, length });
                }
            }
        }
        let Some(hit) = best else {
            break;
        };
        index = hit.at + hit.length;
        out.push(hit);
    }
    out
}

fn text_around(ctx: &CallCtx<'_>, args: &[Value], before: bool) -> Fx<Value> {
    let text = str_of(ctx, &args[0])?;
    let delimiters = delimiters_of(ctx, args.get(1))?;
    let instance = opt_num(ctx, args, 2, 1.0)?.trunc();
    let insensitive = opt_num(ctx, args, 3, 0.0)? == 1.0;
    let match_end = if args.len() > 4 {
        to_bool(&scalar_of(&args[4])?)?
    } else {
        false
    };
    let not_found = if args.len() > 5 {
        let scalar = scalar_of(&args[5])?;
        if matches!(scalar, Scalar::Blank) {
            Scalar::Error(ErrorCode::Na)
        } else {
            scalar
        }
    } else {
        Scalar::Error(ErrorCode::Na)
    };
    if instance == 0.0 || instance.abs() > utf16_len(&text) as f64 + 1.0 {
        return Err(ErrorCode::Value);
    }
    if delimiters.is_empty() || delimiters.iter().all(|item| item.is_empty()) {
        let shown = if before {
            if instance > 0.0 {
                String::new()
            } else {
                text
            }
        } else if instance > 0.0 {
            text
        } else {
            String::new()
        };
        return Ok(Value::Scalar(Scalar::Text(shown)));
    }
    let mut found = occurrences(&text, &delimiters, insensitive);
    if match_end {
        if instance > 0.0 {
            found.push(Hit {
                at: utf16_len(&text),
                length: 0,
            });
        } else {
            found.insert(0, Hit { at: 0, length: 0 });
        }
    }
    let index = if instance > 0.0 {
        instance as isize - 1
    } else {
        found.len() as isize + instance as isize
    };
    if index < 0 {
        return Ok(Value::Scalar(not_found));
    }
    let Some(hit) = found.get(index as usize) else {
        return Ok(Value::Scalar(not_found));
    };
    let units = utf16_units(&text);
    let shown = if before {
        from_units(&units[..hit.at.min(units.len())])
    } else {
        from_units(&units[(hit.at + hit.length).min(units.len())..])
    };
    Ok(Value::Scalar(Scalar::Text(shown)))
}

fn delimiters_of(ctx: &CallCtx<'_>, value: Option<&Value>) -> Fx<Vec<String>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if matches!(value, Value::Scalar(Scalar::Blank)) {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for item in ctx.to_matrix(value)?.flat()? {
        if let Scalar::Error(code) = item {
            return Err(code);
        }
        out.push(to_text(&item)?);
    }
    Ok(out)
}

fn split_text(
    text: &str,
    delimiters: &[String],
    insensitive: bool,
    ignore_empty: bool,
) -> Vec<String> {
    if delimiters.is_empty() {
        return vec![text.to_owned()];
    }
    let units = utf16_units(text);
    let mut parts = Vec::new();
    let mut last = 0usize;
    for hit in occurrences(text, delimiters, insensitive) {
        parts.push(from_units(&units[last..hit.at.min(units.len())]));
        last = hit.at + hit.length;
    }
    parts.push(from_units(&units[last.min(units.len())..]));
    if ignore_empty {
        parts.retain(|part| !part.is_empty());
    }
    parts
}

fn js_bool(scalar: &Scalar) -> bool {
    match scalar {
        Scalar::Blank => false,
        Scalar::Bool(bit) => *bit,
        Scalar::Number(n) => *n != 0.0 && !n.is_nan(),
        Scalar::Text(text) => !text.is_empty(),
        Scalar::Error(_) => true,
    }
}

fn text_split(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let text = str_of(ctx, &args[0])?;
    let col_delims = delimiters_of(ctx, args.get(1))?;
    let row_delims = delimiters_of(ctx, args.get(2))?;
    let ignore_empty = args.len() > 3
        && !matches!(args[3], Value::Scalar(Scalar::Blank))
        && js_bool(&scalar_of(&args[3])?);
    let insensitive = opt_num(ctx, args, 4, 0.0)? == 1.0;
    let pad = if args.len() > 5 {
        ctx.to_scalar(&args[5])
    } else {
        Scalar::Error(ErrorCode::Na)
    };
    if col_delims.is_empty() && row_delims.is_empty() {
        return Err(ErrorCode::Value);
    }
    let rows = split_text(&text, &row_delims, insensitive, ignore_empty);
    let split_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|line| split_text(line, &col_delims, insensitive, ignore_empty))
        .collect();
    let width = split_rows.iter().map(Vec::len).max().unwrap_or(0);
    let matrix = Matrix::from_rows(
        split_rows
            .into_iter()
            .map(|row| {
                (0..width)
                    .map(|index| {
                        row.get(index)
                            .cloned()
                            .map(Scalar::Text)
                            .unwrap_or_else(|| pad.clone())
                    })
                    .collect()
            })
            .collect(),
    );
    Ok(Value::Matrix(matrix))
}

fn scalar_to_text(value: &Scalar, strict: bool) -> String {
    if let Scalar::Error(code) = value {
        return code.as_excel().to_owned();
    }
    if let Scalar::Text(text) = value {
        if strict {
            return format!("\"{}\"", text.replace('"', "\"\""));
        }
        return text.clone();
    }
    to_text(value).unwrap_or_default()
}

fn value_to_text_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let _ = ctx;
    let strict = opt_num(ctx, args, 1, 0.0)? == 1.0;
    let scalar = scalar_of(args.first().unwrap_or(&Value::blank()))?;
    Ok(Value::Scalar(Scalar::Text(scalar_to_text(&scalar, strict))))
}

fn array_to_text(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let strict = opt_num(ctx, args, 1, 0.0)? == 1.0;
    let matrix = ctx.to_matrix(args.first().unwrap_or(&Value::blank()))?;
    if !strict {
        let text = matrix
            .flat()?
            .iter()
            .map(|value| scalar_to_text(value, false))
            .collect::<Vec<_>>()
            .join(", ");
        return Ok(Value::Scalar(Scalar::Text(text)));
    }
    let data = matrix.data()?;
    let body = data
        .iter()
        .map(|row| {
            row.iter()
                .map(|value| scalar_to_text(value, true))
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join(";");
    Ok(Value::Scalar(Scalar::Text(format!("{{{body}}}"))))
}
