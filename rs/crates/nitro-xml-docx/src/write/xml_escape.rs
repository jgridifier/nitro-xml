// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/write/xml-out.ts (escape helpers); changes listed in PROVENANCE.md.

/// SpreadsheetML / WordprocessingML XML declaration used on regenerated parts.
pub const XML_HEADER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>";

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
            out.push_str(&format!("_x{:04X}_", ch as u32));
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

pub fn escape_attr(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#xA;"),
            '\r' => out.push_str("&#xD;"),
            '\t' => out.push_str("&#x9;"),
            _ => out.push(ch),
        }
    }
    out
}
