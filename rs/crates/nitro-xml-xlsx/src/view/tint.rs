// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/tint.ts; changes listed in PROVENANCE.md.

const HLSMAX: i32 = 240;
const RGBMAX: i32 = 255;

fn idiv(a: i32, b: i32) -> i32 {
    a / b
}

struct Hls {
    h: i32,
    l: i32,
    s: i32,
}

fn rgb_to_hls240(r: i32, g: i32, b: i32) -> Hls {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = idiv((max + min) * HLSMAX + RGBMAX, 2 * RGBMAX);
    if max == min {
        return Hls {
            h: idiv(HLSMAX * 2, 3),
            l,
            s: 0,
        };
    }
    let sum = max + min;
    let s = if l <= HLSMAX / 2 {
        idiv((max - min) * HLSMAX + idiv(sum, 2), sum)
    } else {
        idiv(
            (max - min) * HLSMAX + idiv(2 * RGBMAX - sum, 2),
            2 * RGBMAX - sum,
        )
    };
    let delta = |c: i32| idiv((max - c) * idiv(HLSMAX, 6) + idiv(max - min, 2), max - min);
    let rd = delta(r);
    let gd = delta(g);
    let bd = delta(b);
    let h = if r == max {
        bd - gd
    } else if g == max {
        idiv(HLSMAX, 3) + rd - bd
    } else {
        idiv(2 * HLSMAX, 3) + gd - rd
    };
    let h = if h < 0 { h + HLSMAX } else { h };
    let h = if h > HLSMAX { h - HLSMAX } else { h };
    Hls { h, l, s }
}

fn hue_to_rgb(n1: i32, n2: i32, hue: i32) -> i32 {
    let mut h = hue;
    if h < 0 {
        h += HLSMAX;
    }
    if h > HLSMAX {
        h -= HLSMAX;
    }
    let sixth = idiv(HLSMAX, 6);
    if h < sixth {
        return n1 + idiv((n2 - n1) * h + idiv(HLSMAX, 12), sixth);
    }
    if h < idiv(HLSMAX, 2) {
        return n2;
    }
    if h < idiv(HLSMAX * 2, 3) {
        return n1
            + idiv(
                (n2 - n1) * (idiv(HLSMAX * 2, 3) - h) + idiv(HLSMAX, 12),
                sixth,
            );
    }
    n1
}

fn clamp255(v: i32) -> i32 {
    v.clamp(0, 255)
}

fn hls240_to_rgb(hls: &Hls) -> [i32; 3] {
    if hls.s == 0 {
        let v = clamp255(((hls.l * RGBMAX) as f64 / HLSMAX as f64).round() as i32);
        return [v, v, v];
    }
    let m2 = if hls.l <= HLSMAX / 2 {
        idiv(hls.l * (HLSMAX + hls.s) + idiv(HLSMAX, 2), HLSMAX)
    } else {
        hls.l + hls.s - idiv(hls.l * hls.s + idiv(HLSMAX, 2), HLSMAX)
    };
    let m1 = 2 * hls.l - m2;
    let channel = |hue: i32| {
        clamp255(idiv(
            hue_to_rgb(m1, m2, hue) * RGBMAX + idiv(HLSMAX, 2),
            HLSMAX,
        ))
    };
    [
        channel(hls.h + idiv(HLSMAX, 3)),
        channel(hls.h),
        channel(hls.h - idiv(HLSMAX, 3)),
    ]
}

pub fn parse_hex(hex: &str) -> Option<[i32; 3]> {
    let s = hex.trim_start_matches('#');
    let bytes = if s.len() == 8 {
        s.get(2..)?
    } else if s.len() == 6 {
        s
    } else {
        return None;
    };
    if bytes.len() != 6 || !bytes.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = i32::from_str_radix(&bytes[0..2], 16).ok()?;
    let g = i32::from_str_radix(&bytes[2..4], 16).ok()?;
    let b = i32::from_str_radix(&bytes[4..6], 16).ok()?;
    Some([r, g, b])
}

pub fn to_hex_color(rgb: [i32; 3]) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        rgb[0].clamp(0, 255),
        rgb[1].clamp(0, 255),
        rgb[2].clamp(0, 255)
    )
}

pub fn apply_tint(hex: &str, tint: Option<f64>) -> String {
    let rgb = parse_hex(hex).unwrap_or([0, 0, 0]);
    let Some(t) = tint else {
        return to_hex_color(rgb);
    };
    let t = t.clamp(-1.0, 1.0);
    let hls = rgb_to_hls240(rgb[0], rgb[1], rgb[2]);
    let l = if t < 0.0 {
        (hls.l as f64 * (1.0 + t)).trunc() as i32
    } else {
        (hls.l as f64 * (1.0 - t) + HLSMAX as f64 * t).round() as i32
    };
    let l = l.clamp(0, HLSMAX);
    to_hex_color(hls240_to_rgb(&Hls {
        h: hls.h,
        l,
        s: hls.s,
    }))
}
