// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/theme-palette.ts

use nitro_xml_opc::drawingml::hex_to_rgb_channels;

use super::summary::ChartThemePalette;

pub const THEME_SLOTS: [&str; 12] = [
    "lt1", "dk1", "lt2", "dk2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6",
    "hlink", "folHlink",
];

pub const DEFAULT_THEME_COLORS: [&str; 12] = [
    "FFFFFF", "000000", "E7E6E6", "44546A", "4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5",
    "70AD47", "0563C1", "954F72",
];

pub fn theme_color(theme: &ChartThemePalette, index: usize) -> Option<String> {
    let hex = theme
        .colors
        .get(index)
        .map(|s| s.as_str())
        .or_else(|| DEFAULT_THEME_COLORS.get(index).copied());
    hex.and_then(|h| {
        if hex_to_rgb_channels(h).is_some() {
            Some(h.trim_start_matches('#').to_uppercase())
        } else {
            None
        }
    })
}

pub fn chart_theme_palette(
    theme_colors: Option<Vec<String>>,
    major_font: Option<&str>,
    minor_font: Option<&str>,
) -> ChartThemePalette {
    let colors = THEME_SLOTS
        .iter()
        .enumerate()
        .map(|(i, _)| {
            theme_colors
                .as_ref()
                .and_then(|c| c.get(i))
                .cloned()
                .or_else(|| DEFAULT_THEME_COLORS.get(i).map(|s| s.to_string()))
                .unwrap_or_else(|| "000000".to_string())
        })
        .collect();
    ChartThemePalette {
        colors,
        major_font: major_font.unwrap_or("Calibri Light").to_string(),
        minor_font: minor_font.unwrap_or("Calibri").to_string(),
    }
}
