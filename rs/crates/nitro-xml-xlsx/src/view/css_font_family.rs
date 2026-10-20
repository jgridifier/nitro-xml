// SPDX-License-Identifier: MIT
// Ported from ChristopherVR/ooxml@e66aaec src/core/text/css-font-family.ts; changes listed in PROVENANCE.md.

use std::borrow::Cow;

/// Map an OOXML font name to a CSS `font-family` stack.
///
/// Known names are matched case-insensitively without allocating a lowered copy
/// on the Calibri (xlsx S) hot path (#30).
pub fn css_font_family(name: &str) -> Cow<'static, str> {
    if let Some(k) = known_family(name) {
        return Cow::Borrowed(k);
    }
    let escaped = name.replace(['"', '\\'], "");
    Cow::Owned(format!(
        "\"{}\", \"Calibri\", \"Carlito\", \"Segoe UI\", sans-serif",
        escaped
    ))
}

fn known_family(name: &str) -> Option<&'static str> {
    // Fast path: exact Calibri (cells_1k / default theme).
    if name == "Calibri" || name.eq_ignore_ascii_case("calibri") {
        return Some("\"Calibri\", \"Carlito\", \"Segoe UI\", Arial, sans-serif");
    }
    if name.eq_ignore_ascii_case("calibri light") {
        return Some(
            "\"Calibri Light\", \"Calibri\", \"Carlito\", \"Segoe UI\", Arial, sans-serif",
        );
    }
    if name.eq_ignore_ascii_case("cambria") {
        return Some("\"Cambria\", \"Caladea\", Georgia, serif");
    }
    if name.eq_ignore_ascii_case("arial") {
        return Some("Arial, \"Liberation Sans\", Helvetica, sans-serif");
    }
    if name.eq_ignore_ascii_case("times new roman") {
        return Some("\"Times New Roman\", \"Liberation Serif\", Times, serif");
    }
    if name.eq_ignore_ascii_case("courier new") {
        return Some("\"Courier New\", \"Liberation Mono\", monospace");
    }
    if name.eq_ignore_ascii_case("aptos") {
        return Some("\"Aptos\", \"Calibri\", \"Carlito\", \"Segoe UI\", sans-serif");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibri_fast_path_no_owned() {
        // Catches: Calibri css_font_family regressing to Owned and reallocating (#30).
        match css_font_family("Calibri") {
            Cow::Borrowed(s) => assert!(s.contains("Calibri")),
            Cow::Owned(_) => panic!("Calibri must be Borrowed"),
        }
        match css_font_family("CALIBRI") {
            Cow::Borrowed(_) => {}
            Cow::Owned(_) => panic!("case-insensitive Calibri must be Borrowed"),
        }
    }
}
