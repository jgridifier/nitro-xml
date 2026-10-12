// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/chart-style-palette.ts (`shade`); changes listed in PROVENANCE.md.

/// Lighten a `#rrggbb` colour toward white. `amount` in `[0, 1]`.
pub fn tint_hex(hex: &str, amount: f64) -> String {
    let amount = amount.clamp(0.0, 1.0);
    let (r, g, b) = hex_to_rgb(hex);
    rgb_to_hex(
        r + (255.0 - r) * amount,
        g + (255.0 - g) * amount,
        b + (255.0 - b) * amount,
    )
}

/// Darken a `#rrggbb` colour toward black. `amount` in `[0, 1]`.
pub fn shade_hex(hex: &str, amount: f64) -> String {
    let amount = amount.clamp(0.0, 1.0);
    let (r, g, b) = hex_to_rgb(hex);
    rgb_to_hex(r * (1.0 - amount), g * (1.0 - amount), b * (1.0 - amount))
}

fn hex_to_rgb(hex: &str) -> (f64, f64, f64) {
    let h = hex.trim_start_matches('#');
    if h.len() < 6 {
        return (136.0, 136.0, 136.0);
    }
    let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(136) as f64;
    let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(136) as f64;
    let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(136) as f64;
    (r, g, b)
}

fn rgb_to_hex(r: f64, g: f64, b: f64) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        r.round().clamp(0.0, 255.0) as u8,
        g.round().clamp(0.0, 255.0) as u8,
        b.round().clamp(0.0, 255.0) as u8
    )
}
