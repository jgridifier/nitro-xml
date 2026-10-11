// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/simple-types.ts; changes listed in PROVENANCE.md.

use nitro_xml_opc::xml::Element;

use crate::xml_util::ElementExt;

pub fn parse_on_off(value: Option<&str>) -> Option<bool> {
    match value {
        Some("true") | Some("on") | Some("1") => Some(true),
        Some("false") | Some("off") | Some("0") => Some(false),
        _ => None,
    }
}

pub fn on_off_element(element: Option<&Element>) -> Option<bool> {
    let element = element?;
    let value = element.attr_w("val");
    if value.is_none() {
        Some(true)
    } else {
        parse_on_off(value.as_deref())
    }
}

const SAFE_INTEGER: &str = r"^-?\d+$";

pub fn parse_integer(value: Option<&str>) -> Option<i64> {
    let value = value?;
    if !regex_is_match(SAFE_INTEGER, value) {
        return None;
    }
    value.parse().ok().filter(|n: &i64| n.abs() <= (1i64 << 53))
}

pub fn parse_unsigned_integer(value: Option<&str>) -> Option<u64> {
    parse_integer(value).filter(|&n| n >= 0).map(|n| n as u64)
}

pub fn parse_signed_twips(value: Option<&str>) -> Option<i64> {
    let value = value?;
    let text = value.trim();
    if regex_is_match(SAFE_INTEGER, text) {
        return parse_integer(Some(text));
    }
    universal_measure_to_twips(text)
}

pub fn parse_twips(value: Option<&str>) -> Option<u64> {
    parse_signed_twips(value)
        .filter(|&n| n >= 0)
        .map(|n| n as u64)
}

pub fn parse_half_points(value: Option<&str>) -> Option<i64> {
    let value = value?;
    let text = value.trim();
    if text.chars().all(|c| c.is_ascii_digit()) {
        return parse_integer(Some(text));
    }
    let twips = universal_measure_to_twips(text)?;
    if twips >= 0 {
        Some((twips + 5) / 10)
    } else {
        None
    }
}

pub fn parse_signed_half_points(value: Option<&str>) -> Option<i64> {
    let value = value?;
    if let Some(n) = parse_integer(Some(value.trim())) {
        return Some(n);
    }
    let twips = universal_measure_to_twips(value.trim())?;
    Some((twips + 5) / 10)
}

pub fn parse_rgb_color(value: Option<&str>) -> Option<String> {
    let value = value?;
    if value == "auto" {
        return None;
    }
    if value.len() == 6 && value.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(format!("#{}", value))
    } else {
        None
    }
}

pub fn parse_tint_shade(value: Option<&str>) -> Option<f64> {
    let value = value?;
    if value.len() != 2 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let n = i64::from_str_radix(value, 16).unwrap_or(0);
    Some(n as f64 / 255.0)
}

pub fn parse_eighth_points(value: Option<&str>) -> Option<u64> {
    parse_unsigned_integer(value)
}

pub fn parse_text_scale(value: Option<&str>) -> Option<i64> {
    parse_integer(value)
}

fn universal_measure_to_twips(value: &str) -> Option<i64> {
    let units: &[(&str, f64)] = &[
        ("in", 1440.0),
        ("cm", 1440.0 / 2.54),
        ("mm", 144.0 / 2.54),
        ("pt", 20.0),
        ("pc", 240.0),
        ("pi", 240.0),
    ];
    for (suffix, per) in units {
        if let Some(num) = value.strip_suffix(suffix) {
            let total: f64 = num.parse::<f64>().ok()? * per;
            let rounded = total.round() as i64;
            return Some(rounded);
        }
    }
    None
}

fn regex_is_match(pattern: &str, text: &str) -> bool {
    // tiny subset for ^-?\d+$
    if pattern == SAFE_INTEGER {
        let t = text.trim();
        if t.is_empty() {
            return false;
        }
        let mut i = 0;
        if t.starts_with('-') {
            i = 1;
        }
        return t[i..].chars().all(|c| c.is_ascii_digit()) && (t.len() > i);
    }
    false
}
