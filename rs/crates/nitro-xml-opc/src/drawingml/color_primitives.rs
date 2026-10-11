// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/color/color-primitives.ts; changes listed in PROVENANCE.md.

#[derive(Debug, Clone, Copy)]
pub struct HslColor {
    pub h: f64,
    pub s: f64,
    pub l: f64,
}

pub fn clamp_unit_interval(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

pub fn hex_to_rgb_channels(color: &str) -> Option<(f64, f64, f64)> {
    let normalized = color.trim_start_matches('#');
    if normalized.len() != 6 || !normalized.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = i64::from_str_radix(&normalized[0..2], 16).ok()? as f64;
    let g = i64::from_str_radix(&normalized[2..4], 16).ok()? as f64;
    let b = i64::from_str_radix(&normalized[4..6], 16).ok()? as f64;
    Some((r, g, b))
}

fn percent_divisor_for(text: &str) -> f64 {
    if text.ends_with('%') {
        100.0
    } else {
        100_000.0
    }
}

pub fn parse_ooxml_percent(value: &str) -> Option<f64> {
    let text = value.trim();
    let divisor = percent_divisor_for(text);
    let magnitude = if divisor == 100.0 {
        text.strip_suffix('%').and_then(|s| s.parse::<f64>().ok())
    } else {
        text.parse::<f64>().ok()
    };
    magnitude.map(|m| m / divisor)
}

pub fn parse_drawing_percent(value: &str) -> Option<f64> {
    parse_ooxml_percent(value).map(clamp_unit_interval)
}

pub fn parse_drawing_fraction(value: &str) -> Option<f64> {
    parse_ooxml_percent(value)
}

pub fn parse_drawing_hue_degrees(value: &str) -> Option<f64> {
    let parsed = value.trim().parse::<f64>().ok()?;
    Some(parsed / 60_000.0)
}

pub fn scrgb_linear_to_srgb8(linear: f64) -> i32 {
    let l = linear.clamp(0.0, 1.0);
    let companded = if l <= 0.0031308 {
        12.92 * l
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (companded * 255.0).round().clamp(0.0, 255.0) as i32
}

pub fn to_hex(value: f64) -> String {
    format!("{:02X}", value.round().clamp(0.0, 255.0) as i32)
}

pub fn rgb_to_hsl(r: f64, g: f64, b: f64) -> HslColor {
    let r_n = r / 255.0;
    let g_n = g / 255.0;
    let b_n = b / 255.0;
    let c_max = r_n.max(g_n).max(b_n);
    let c_min = r_n.min(g_n).min(b_n);
    let delta = c_max - c_min;
    let l = (c_max + c_min) / 2.0;
    let s = if delta == 0.0 {
        0.0
    } else {
        delta / (1.0 - (2.0 * l - 1.0).abs())
    };
    let h = if delta == 0.0 {
        0.0
    } else if c_max == r_n {
        60.0 * (((g_n - b_n) / delta) % 6.0)
    } else if c_max == g_n {
        60.0 * ((b_n - r_n) / delta + 2.0)
    } else {
        60.0 * ((r_n - g_n) / delta + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    HslColor {
        h,
        s: clamp_unit_interval(s),
        l: clamp_unit_interval(l),
    }
}

pub fn hsl_to_rgb(
    h: f64,
    s: f64,
    l: f64,
    round_channel: fn(f64) -> f64,
    clamp_saturation: bool,
) -> (f64, f64, f64) {
    let s_c = if clamp_saturation {
        clamp_unit_interval(s)
    } else {
        s
    };
    let l_c = clamp_unit_interval(l);
    let h_n = ((h % 360.0) + 360.0) % 360.0;
    let c = (1.0 - (2.0 * l_c - 1.0).abs()) * s_c;
    let x = c * (1.0 - ((h_n / 60.0) % 2.0 - 1.0).abs());
    let m = l_c - c / 2.0;
    let (r_p, g_p, b_p) = if h_n < 60.0 {
        (c, x, 0.0)
    } else if h_n < 120.0 {
        (x, c, 0.0)
    } else if h_n < 180.0 {
        (0.0, c, x)
    } else if h_n < 240.0 {
        (0.0, x, c)
    } else if h_n < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (
        round_channel((r_p + m) * 255.0),
        round_channel((g_p + m) * 255.0),
        round_channel((b_p + m) * 255.0),
    )
}
