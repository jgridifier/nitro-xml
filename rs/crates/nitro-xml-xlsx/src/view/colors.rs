// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/colors.ts and
// src/core/chart/render/theme-palette.ts (themeColor); changes listed in PROVENANCE.md.

use crate::model::{Color, ThemePalette};
use crate::view::tint::{apply_tint, parse_hex, to_hex_color};

pub const INDEXED_COLORS: [&str; 66] = [
    "000000", "FFFFFF", "FF0000", "00FF00", "0000FF", "FFFF00", "FF00FF", "00FFFF", "000000",
    "FFFFFF", "FF0000", "00FF00", "0000FF", "FFFF00", "FF00FF", "00FFFF", "800000", "008000",
    "000080", "808000", "800080", "008080", "C0C0C0", "808080", "9999FF", "993366", "FFFFCC",
    "CCFFFF", "660066", "FF8080", "0066CC", "CCCCFF", "000080", "FF00FF", "FFFF00", "00FFFF",
    "800080", "800000", "008080", "0000FF", "00CCFF", "CCFFFF", "CCFFCC", "FFFF99", "99CCFF",
    "FF99CC", "CC99FF", "FFCC99", "3366FF", "33CCCC", "99CC00", "FFCC00", "FF9900", "FF6600",
    "666699", "969696", "003366", "339966", "003300", "333300", "993300", "993366", "333399",
    "333333", "000000", "FFFFFF",
];

pub fn theme_color(theme: &ThemePalette, index: usize) -> Option<String> {
    theme.colors.get(index).cloned()
}

pub fn resolve_color(
    color: Option<&Color>,
    theme: &ThemePalette,
    _fallback: Option<&str>,
    indexed: &[&str],
) -> Option<String> {
    let color = color?;
    let mut base: Option<String> = None;
    if let Some(rgb) = &color.rgb {
        base = parse_hex(rgb).map(|_| rgb.trim_start_matches('#').to_uppercase());
    } else if let Some(t) = color.theme {
        let idx = t as usize;
        base = theme_color(theme, idx);
    } else if let Some(i) = color.indexed {
        let idx = i as usize;
        base = indexed
            .get(idx)
            .or_else(|| INDEXED_COLORS.get(idx))
            .map(|s| s.to_string());
    }
    let base = base?;
    let hex = if base.starts_with('#') {
        base
    } else {
        format!("#{}", base)
    };
    Some(apply_tint(&hex, color.tint))
}

pub fn mix_colors(a: &str, b: &str, t: f64, toward_start: bool) -> String {
    let ca = parse_hex(a).unwrap_or([0, 0, 0]);
    let cb = parse_hex(b).unwrap_or([0, 0, 0]);
    let k = t.clamp(0.0, 1.0);
    let out: [i32; 3] = ca
        .iter()
        .zip(cb.iter())
        .map(|(v, bv)| {
            let delta = ((*bv - *v) as f64) * k;
            let v = *v as f64 + if toward_start { delta.trunc() } else { delta };
            v.round() as i32
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap_or([0, 0, 0]);
    to_hex_color(out)
}
