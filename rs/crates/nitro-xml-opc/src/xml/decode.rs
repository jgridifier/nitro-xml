// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xml/lite.ts (entity decoding, line-ending and
// attribute-value normalisation); changes listed in PROVENANCE.md.
//! Entity decoding and the line-ending / attribute-value normalisation the lite reader and DOM
//! share.

/// XML 1.0 end-of-line normalisation (also the NEL / line-separator forms lite normalises).
pub fn normalize_line_endings(xml: &str) -> String {
    if !xml.contains('\r')
        && !xml.contains('\u{0085}')
        && !xml.contains('\u{2028}')
        && !xml.contains('\u{2029}')
    {
        return xml.to_owned();
    }
    let mut out = String::with_capacity(xml.len());
    let mut chars = xml.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if matches!(chars.peek(), Some('\n' | '\u{0085}')) {
                    chars.next();
                }
                out.push('\n');
            }
            '\u{0085}' | '\u{2028}' | '\u{2029}' => out.push('\n'),
            other => out.push(other),
        }
    }
    out
}

/// Decodes predefined and numeric character references. Returns `None` when an `&` is not a known,
/// well-formed reference (parity with lite `decode`).
pub fn decode_entities(text: &str) -> Option<String> {
    if !text.contains('&') {
        return Some(text.to_owned());
    }
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut ampersands = 0usize;
    let mut found = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'&' {
            let start = i;
            while i < bytes.len() && bytes[i] != b'&' {
                i += 1;
            }
            out.push_str(&text[start..i]);
            continue;
        }
        ampersands += 1;
        let semi = text[i + 1..].find(';').map(|o| i + 1 + o)?;
        let body = &text[i + 1..semi];
        if body.is_empty() {
            return None;
        }
        let replacement =
            if let Some(hex) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
                let code = u32::from_str_radix(hex, 16).ok()?;
                char_from_code(code)?
            } else if let Some(dec) = body.strip_prefix('#') {
                let code = dec.parse::<u32>().ok()?;
                char_from_code(code)?
            } else {
                match body {
                    "amp" => "&".to_owned(),
                    "lt" => "<".to_owned(),
                    "gt" => ">".to_owned(),
                    "quot" => "\"".to_owned(),
                    "apos" => "'".to_owned(),
                    _ => return None,
                }
            };
        found += 1;
        out.push_str(&replacement);
        i = semi + 1;
    }
    if ampersands != found {
        return None;
    }
    Some(out)
}

fn char_from_code(code: u32) -> Option<String> {
    if code == 0 || code > 0x10_ffff {
        return None;
    }
    // Surrogate code points are not Unicode scalar values.
    char::from_u32(code).map(|c| c.to_string())
}

/// Attribute-value normalisation matching lite: literal tab/LF/CR become spaces, then entities
/// decode (so `&#10;` remains a newline).
pub fn normalize_attribute_value(raw: &str) -> Option<String> {
    let spaced = if raw.bytes().any(|b| matches!(b, b'\t' | b'\n' | b'\r')) {
        raw.chars()
            .map(|c| match c {
                '\t' | '\n' | '\r' => ' ',
                other => other,
            })
            .collect::<String>()
    } else {
        raw.to_owned()
    };
    decode_entities(&spaced)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_crlf_normalized_before_numeric_cr_entity() {
        // Catches: XML EOL folding must run before entity expansion so &#13; stays CR while
        // literal CR/LF become LF (lite.test.ts line-ending case).
        let text = normalize_line_endings("l1\r\nl2\rl3");
        assert_eq!(text, "l1\nl2\nl3");
        assert_eq!(decode_entities("l3&#13;&#x41;"), Some("l3\rA".to_owned()));
    }

    #[test]
    fn catches_attr_ws_before_entity_keeps_numeric_lf() {
        // Catches: attribute whitespace folding before entity decode, so &#10; is a real newline
        // while a literal LF in the attribute value becomes a space.
        assert_eq!(
            normalize_attribute_value("x\ty\nz&#10;"),
            Some("x y z\n".to_owned())
        );
    }
}
