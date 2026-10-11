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

fn font_size_px(font: &str) -> f64 {
    font.split_whitespace()
        .find_map(|tok| tok.strip_suffix("px"))
        .and_then(|n| n.parse().ok())
        .unwrap_or(14.67)
}

/// Port of upstream `estimateWidth` (`src/ui/src/xlsx/grid/measure.ts`) for DOM grid paint.
pub fn estimate_width(text: &str, font: &str) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    let size = font_size_px(font);
    let bold = if font.contains("bold") { 1.07 } else { 1.0 };
    let mut units = 0.0;
    for ch in text.chars() {
        units += if "ilI.,:;'|!".contains(ch) {
            0.28
        } else if "mwMW@".contains(ch) {
            0.85
        } else if ch as u32 >= 0x3000 && ch as u32 <= 0x9FFF {
            1.0
        } else {
            0.52
        };
    }
    units * size * bold
}
