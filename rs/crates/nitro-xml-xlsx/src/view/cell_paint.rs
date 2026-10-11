// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/ui/grid/cell-paint.ts (cssFont, hashes, hasBorders, repeatToFill);
// changes listed in PROVENANCE.md.

use crate::view::css_font_family::css_font_family;
use crate::view::types::{BordersView, FontView};

pub fn css_font(font: &FontView, zoom: f64) -> String {
    let size = ((font.size_px * (zoom / 100.0) * 100.0).round() / 100.0).max(1.0);
    format!(
        "{}{} {}px {}",
        if font.italic { "italic " } else { "" },
        if font.bold { "bold " } else { "" },
        size,
        css_font_family(&font.family)
    )
}

pub fn has_borders(b: &BordersView) -> bool {
    b.has_borders()
}

pub fn repeat_to_fill(text: &str, text_width: f64, cell_width: f64) -> String {
    if text.is_empty() || text_width <= 0.0 {
        return text.to_string();
    }
    let times = ((cell_width / text_width).floor() as usize).max(1);
    text.repeat(times.min(500))
}

pub fn hashes(cell_inner_width: f64, hash_width: f64) -> String {
    let n = ((cell_inner_width / hash_width.max(1.0)).floor() as usize).max(1);
    "#".repeat(n.min(200))
}
