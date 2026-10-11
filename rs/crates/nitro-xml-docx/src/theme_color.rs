// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/theme-color.ts; changes listed in PROVENANCE.md.

use serde_json::Value;

pub fn resolve_theme_color_reference(color_theme: &Value, theme: &Value) -> Option<String> {
    let token = color_theme.get("token").and_then(|v| v.as_str())?;
    let colors = theme.get("colors").and_then(|c| c.as_object())?;
    let mapping = theme.get("colorMapping").and_then(|m| m.as_object());
    let slot = match token {
        "background1" | "text1" | "background2" | "text2" => {
            let key = match token {
                "background1" => "bg1",
                "text1" => "tx1",
                "background2" => "bg2",
                "text2" => "tx2",
                _ => return None,
            };
            let mapped = mapping
                .and_then(|m| m.get(key))
                .and_then(|v| v.as_str())
                .unwrap_or(match key {
                    "bg1" => "lt1",
                    "tx1" => "dk1",
                    "bg2" => "lt2",
                    "tx2" => "dk2",
                    _ => "dk1",
                });
            mapped
        }
        "dark1" => "dk1",
        "light1" => "lt1",
        "dark2" => "dk2",
        "light2" => "lt2",
        other => other,
    };
    let hex = colors.get(slot).and_then(|v| v.as_str())?;
    let mut out = format!("#{}", hex.trim_start_matches('#'));
    if let Some(shade) = color_theme.get("shade").and_then(|v| v.as_f64()) {
        out = apply_shade(&out, shade);
    }
    if let Some(tint) = color_theme.get("tint").and_then(|v| v.as_f64()) {
        out = apply_tint(&out, tint);
    }
    Some(out)
}

fn apply_shade(hex: &str, fraction: f64) -> String {
    mix(hex, fraction)
}

fn apply_tint(hex: &str, fraction: f64) -> String {
    mix_toward_white(hex, fraction)
}

fn mix(hex: &str, fraction: f64) -> String {
    let rgb = match parse_hex(hex) {
        Some(rgb) => rgb,
        None => return hex.to_string(),
    };
    format!(
        "#{:02x}{:02x}{:02x}",
        (rgb.0 as f64 * fraction) as u8,
        (rgb.1 as f64 * fraction) as u8,
        (rgb.2 as f64 * fraction) as u8
    )
}

fn mix_toward_white(hex: &str, fraction: f64) -> String {
    let rgb = match parse_hex(hex) {
        Some(rgb) => rgb,
        None => return hex.to_string(),
    };
    format!(
        "#{:02x}{:02x}{:02x}",
        (255.0 - (255.0 - rgb.0 as f64) * fraction) as u8,
        (255.0 - (255.0 - rgb.1 as f64) * fraction) as u8,
        (255.0 - (255.0 - rgb.2 as f64) * fraction) as u8
    )
}

fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
    let normalized = hex.trim_start_matches('#');
    if normalized.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&normalized[0..2], 16).ok()?;
    let g = u8::from_str_radix(&normalized[2..4], 16).ok()?;
    let b = u8::from_str_radix(&normalized[4..6], 16).ok()?;
    Some((r, g, b))
}
