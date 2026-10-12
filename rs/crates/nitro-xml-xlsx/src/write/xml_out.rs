// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/write/xml-out.ts; changes listed in PROVENANCE.md.
//! SpreadsheetML text escapes and element helpers.

pub const XML_HEADER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// Characters XML 1.0 cannot carry (`xml-out.ts` `CONTROL`, without CR).
fn is_xml_invalid(ch: char) -> bool {
    let u = ch as u32;
    u <= 0x08 || u == 0x0B || u == 0x0C || (0x0E..=0x1F).contains(&u) || u == 0xFFFE || u == 0xFFFF
}

fn is_control_or_cr(ch: char) -> bool {
    ch == '\r' || is_xml_invalid(ch)
}

fn looks_like_hex_escape(rest: &[char]) -> bool {
    rest.len() >= 6
        && (rest[0] == 'x' || rest[0] == 'X')
        && rest[1..5].iter().all(|c| c.is_ascii_hexdigit())
        && rest[5] == '_'
}

fn needs_escape(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    for (i, ch) in chars.iter().copied().enumerate() {
        if is_control_or_cr(ch) {
            return true;
        }
        if ch == '_' && looks_like_hex_escape(&chars[i + 1..]) {
            return true;
        }
    }
    false
}

/// Encodes control characters (and `\r`) as `_xHHHH_`, and escapes a leading underscore of `_xHHHH_`.
pub fn encode_escapes(text: &str) -> String {
    if !needs_escape(text) {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if is_control_or_cr(ch) {
            out.push_str(&format!("_x{ch:04X}", ch = ch as u32));
            i += 1;
            continue;
        }
        if ch == '_' && looks_like_hex_escape(&chars[i + 1..]) {
            out.push_str("_x005F_");
            i += 1;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

/// Escapes element text; characters XML cannot carry are dropped.
pub fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if is_xml_invalid(ch) {
            continue;
        }
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Escapes an attribute value, including newlines and tabs.
pub fn escape_attr(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if is_xml_invalid(ch) {
            continue;
        }
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\r' => out.push_str("&#13;"),
            '\n' => out.push_str("&#10;"),
            '\t' => out.push_str("&#9;"),
            _ => out.push(ch),
        }
    }
    out
}

/// A number as SpreadsheetML writes it (`String(n)` in JS, `-0` → `0`).
pub fn num(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    crate::number_format::js_number_string(value)
}

pub fn attr_str(name: &str, value: Option<&str>) -> String {
    match value {
        Some(v) => format!(" {name}=\"{}\"", escape_attr(v)),
        None => String::new(),
    }
}

pub fn attr_f64(name: &str, value: Option<f64>) -> String {
    match value {
        Some(v) => format!(" {name}=\"{}\"", num(v)),
        None => String::new(),
    }
}

pub fn attr_u32(name: &str, value: Option<u32>) -> String {
    match value {
        Some(v) => format!(" {name}=\"{v}\""),
        None => String::new(),
    }
}

/// `None` omits the attribute; `Some(false)` writes `0`.
pub fn attr_bool(name: &str, value: Option<bool>) -> String {
    match value {
        Some(true) => format!(" {name}=\"1\""),
        Some(false) => format!(" {name}=\"0\""),
        None => String::new(),
    }
}

/// An element; self-closing when `content` is empty.
pub fn el(name: &str, attrs: &str, content: &str) -> String {
    if content.is_empty() {
        format!("<{name}{attrs}/>")
    } else {
        format!("<{name}{attrs}>{content}</{name}>")
    }
}

/// `<t>` text with `xml:space="preserve"` when whitespace would otherwise be trimmed.
pub fn t_element(text: &str) -> String {
    let encoded = escape_text(&encode_escapes(text));
    let preserve = text.chars().next().is_some_and(char::is_whitespace)
        || text.chars().next_back().is_some_and(char::is_whitespace)
        || text.contains('\n')
        || text.contains('\t');
    if preserve {
        format!("<t xml:space=\"preserve\">{encoded}</t>")
    } else {
        format!("<t>{encoded}</t>")
    }
}

/// `xl/workbook.xml` → `worksheets/sheet1.xml` style relative target (upstream `relativeTarget`).
pub fn relative_target(from_part: &str, to_part: &str) -> String {
    let from_parts: Vec<&str> = from_part.split('/').collect();
    let from = if from_parts.len() > 1 {
        &from_parts[..from_parts.len() - 1]
    } else {
        &[]
    };
    let to: Vec<&str> = to_part.split('/').collect();
    let mut common = 0;
    while common < from.len() && common + 1 < to.len() && from[common] == to[common] {
        common += 1;
    }
    let mut parts = vec![".."; from.len() - common];
    parts.extend(to[common..].iter().copied());
    parts.join("/")
}
