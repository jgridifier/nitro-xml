// SPDX-License-Identifier: MIT
//! Text width estimates for layout when canvas measure is unavailable (WASM paint).

/// Deterministic stub for view-IR / oracle dumps (`parity/oracle/xlsx.ts`).
pub fn fake_xlsx_measure(text: &str) -> f64 {
    let mut w = 0.0;
    for ch in text.chars() {
        let cp = ch as u32;
        w += if cp > 255 { 14.0 } else { 7.0 };
    }
    w
}

/// Parse `Npx` from a `css_font` string (`[italic ][bold ]{size}px {family}`).
fn font_size_px(font: &str) -> f64 {
    let Some(px) = font.find("px") else {
        return 14.67;
    };
    let start = font[..px]
        .rfind(|c: char| c.is_whitespace())
        .map(|i| i + 1)
        .unwrap_or(0);
    font[start..px].parse().unwrap_or(14.67)
}

#[inline]
fn char_units(ch: char) -> f64 {
    // Upstream estimateWidth class buckets (measure.ts) — match, not `.contains`.
    match ch {
        'i' | 'l' | 'I' | '.' | ',' | ':' | ';' | '\'' | '|' | '!' => 0.28,
        'm' | 'w' | 'M' | 'W' | '@' => 0.85,
        '\u{3000}'..='\u{9FFF}' => 1.0,
        _ => 0.52,
    }
}

/// Port of upstream `estimateWidth` (`src/ui/src/xlsx/grid/measure.ts`) for DOM grid paint.
pub fn estimate_width(text: &str, font: &str) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    let size = font_size_px(font);
    // Same predicate as upstream estimateWidth (`font.includes("bold")`).
    let bold = if font.contains("bold") { 1.07 } else { 1.0 };
    let mut units = 0.0;
    for ch in text.chars() {
        units += char_units(ch);
    }
    units * size * bold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_width_matches_class_buckets() {
        // Catches: char-class rewrite drifting from upstream estimateWidth buckets (#30).
        let font = "11px Calibri, sans-serif";
        assert!((estimate_width("i", font) - 0.28 * 11.0).abs() < 1e-9);
        assert!((estimate_width("m", font) - 0.85 * 11.0).abs() < 1e-9);
        assert!((estimate_width("a", font) - 0.52 * 11.0).abs() < 1e-9);
        assert!((estimate_width("中", font) - 1.0 * 11.0).abs() < 1e-9);
        let bold = "bold 11px Calibri, sans-serif";
        assert!((estimate_width("a", bold) - 0.52 * 11.0 * 1.07).abs() < 1e-9);
    }

    #[test]
    fn font_size_px_reads_css_font_prefix() {
        // Catches: size parse breaking when italic/bold prefixes are present (#30).
        assert!((font_size_px("11px Arial") - 11.0).abs() < 1e-9);
        assert!((font_size_px("italic bold 12.5px \"Calibri\", sans-serif") - 12.5).abs() < 1e-9);
    }
}
