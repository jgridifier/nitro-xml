// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-colors.ts

use nitro_xml_opc::drawingml::{hex_to_rgb_channels, hsl_to_rgb, rgb_to_hsl, to_hex};
use std::collections::HashMap;

use super::color_palettes::{chart_palette_series_color, find_chart_color_palette};
use super::summary::ChartThemePalette;
use super::theme_palette::{theme_color, THEME_SLOTS};

pub fn chart_color_scheme(theme: &ChartThemePalette) -> HashMap<String, String> {
    let mut slots = HashMap::new();
    for (i, slot) in THEME_SLOTS.iter().enumerate() {
        let hex = theme_color(theme, i).unwrap_or_else(|| "000000".to_string());
        slots.insert(slot.to_string(), format!("#{}", hex));
    }
    slots.insert("bg1".to_string(), slots["lt1"].clone());
    slots.insert("tx1".to_string(), slots["dk1"].clone());
    slots.insert("bg2".to_string(), slots["lt2"].clone());
    slots.insert("tx2".to_string(), slots["dk2"].clone());
    slots
}

pub fn auto_series_color(theme: &ChartThemePalette, index: usize) -> String {
    let i = index.max(0);
    let scheme = chart_color_scheme(theme);
    let palette = find_chart_color_palette(10).expect("palette 10");
    chart_palette_series_color(palette, i, i + 1, &scheme)
}

pub fn modulate_luminance(hex: &str, lum_mod: f64, lum_off: f64) -> String {
    let rgb = hex_to_rgb_channels(hex);
    if rgb.is_none() {
        return "#000000".to_string();
    }
    let (r, g, b) = rgb.unwrap();
    if lum_mod == 1.0 && lum_off == 0.0 {
        return format!(
            "#{}{}{}",
            to_hex(r.round() as i32),
            to_hex(g.round() as i32),
            to_hex(b.round() as i32)
        );
    }
    let hsl = rgb_to_hsl(r, g, b);
    let l = (hsl.l * lum_mod + lum_off).clamp(0.0, 1.0);
    let out = hsl_to_rgb(hsl.h, hsl.s, l, |c| c.round(), true);
    format!(
        "#{}{}{}",
        to_hex(out.0 as i32),
        to_hex(out.1 as i32),
        to_hex(out.2 as i32)
    )
}
