// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/geometry/adjustment-aware-shapes.ts
// (`buildCircularArrow`); changes listed in PROVENANCE.md.
//! TS SmartArt paint uses the adjustment-aware circularArrow polygon (not the
//! ECMA preset evaluator). Match that silhouette for pixel parity.

use std::collections::HashMap;

const ANGLE_UNITS_PER_DEGREE: f64 = 60_000.0;
const DISTANCE_UNITS_PER_FRACTION: f64 = 100_000.0;
const ARC_VERTEX_COUNT: usize = 32;

/// SVG path `d` matching upstream `buildCircularArrow` → `outlinePathData`.
pub fn circular_arrow_path_d(
    width: f64,
    height: f64,
    adjustments: Option<&HashMap<String, f64>>,
) -> Option<String> {
    let width = if width.is_finite() && width > 0.0 {
        width
    } else {
        1.0
    };
    let height = if height.is_finite() && height > 0.0 {
        height
    } else {
        1.0
    };
    let adj = |name: &str| adjustments.and_then(|m| m.get(name).copied());
    let thickness = distance_adj_to_fraction(adj("adj1"), 12_500.0).clamp(0.02, 0.45);
    let start_angle = angle_adj_to_radians(adj("adj2"), 1_142_319.0);
    let end_angle = angle_adj_to_radians(adj("adj3"), 20_457_681.0);
    let head_size = distance_adj_to_fraction(adj("adj4"), 12_500.0).clamp(0.0, 0.45);
    let sweep = clockwise_sweep(start_angle, end_angle);
    let head_sweep = (sweep * 0.4).min(head_size * std::f64::consts::PI);
    let band_sweep_end = start_angle + sweep - head_sweep;
    let cx = width / 2.0;
    let cy = height / 2.0;
    let rx_outer = width / 2.0;
    let ry_outer = height / 2.0;
    let rx_inner = rx_outer * (1.0 - thickness * 2.0);
    let ry_inner = ry_outer * (1.0 - thickness * 2.0);
    let mut points: Vec<(f64, f64)> = Vec::with_capacity(ARC_VERTEX_COUNT * 2 + 8);
    for i in 0..=ARC_VERTEX_COUNT {
        let t = i as f64 / ARC_VERTEX_COUNT as f64;
        let a = start_angle + (band_sweep_end - start_angle) * t;
        points.push((cx + rx_outer * a.cos(), cy + ry_outer * a.sin()));
    }
    let tip_angle = start_angle + sweep;
    let head_radial_outer = (rx_outer + rx_inner) / 2.0 + (rx_outer - rx_inner) * (0.5 + head_size);
    let head_radial_outer_y =
        (ry_outer + ry_inner) / 2.0 + (ry_outer - ry_inner) * (0.5 + head_size);
    let tip_radial_mid = (rx_outer + rx_inner) / 2.0;
    let tip_radial_mid_y = (ry_outer + ry_inner) / 2.0;
    points.push((
        cx + head_radial_outer * band_sweep_end.cos(),
        cy + head_radial_outer_y * band_sweep_end.sin(),
    ));
    points.push((
        cx + tip_radial_mid * tip_angle.cos(),
        cy + tip_radial_mid_y * tip_angle.sin(),
    ));
    let inner_barb_x = rx_inner - (rx_outer - rx_inner) * head_size;
    let inner_barb_y = ry_inner - (ry_outer - ry_inner) * head_size;
    points.push((
        cx + inner_barb_x * band_sweep_end.cos(),
        cy + inner_barb_y * band_sweep_end.sin(),
    ));
    for i in (0..=ARC_VERTEX_COUNT).rev() {
        let t = i as f64 / ARC_VERTEX_COUNT as f64;
        let a = start_angle + (band_sweep_end - start_angle) * t;
        points.push((cx + rx_inner * a.cos(), cy + ry_inner * a.sin()));
    }
    if points.len() < 3 {
        return None;
    }
    let mut d = String::new();
    for (i, (x, y)) in points.iter().enumerate() {
        if i == 0 {
            d.push_str(&format!("M {} {}", fmt(*x), fmt(*y)));
        } else {
            d.push_str(&format!(" L {} {}", fmt(*x), fmt(*y)));
        }
    }
    d.push_str(" Z");
    Some(d)
}

fn angle_adj_to_radians(value: Option<f64>, default_units: f64) -> f64 {
    let raw = value.filter(|v| v.is_finite()).unwrap_or(default_units);
    let degrees = raw / ANGLE_UNITS_PER_DEGREE;
    degrees * (std::f64::consts::PI / 180.0)
}

fn distance_adj_to_fraction(value: Option<f64>, default_units: f64) -> f64 {
    let raw = value.filter(|v| v.is_finite()).unwrap_or(default_units);
    (raw / DISTANCE_UNITS_PER_FRACTION).clamp(-2.0, 2.0)
}

fn normalise_angle(angle: f64) -> f64 {
    let two_pi = std::f64::consts::PI * 2.0;
    let mut a = angle % two_pi;
    if a < 0.0 {
        a += two_pi;
    }
    a
}

fn clockwise_sweep(start_angle: f64, end_angle: f64) -> f64 {
    let start = normalise_angle(start_angle);
    let mut end = normalise_angle(end_angle);
    if end <= start {
        end += std::f64::consts::PI * 2.0;
    }
    end - start
}

fn fmt(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_owned();
    }
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::circular_arrow_path_d;
    use std::collections::HashMap;

    // Catches: SmartArt circularArrow using ECMA arcTo instead of TS adjustment-aware polygon.
    #[test]
    fn fixture_adj_emits_closed_polygon_path() {
        let mut m = HashMap::new();
        m.insert("adj1".into(), 8249.0);
        m.insert("adj2".into(), 576197.0);
        m.insert("adj3".into(), 2963490.0);
        m.insert("adj4".into(), 51967.0);
        m.insert("adj5".into(), 9624.0);
        let d = circular_arrow_path_d(4525778.0, 4525778.0, Some(&m)).expect("path");
        assert!(d.starts_with('M'), "{d}");
        assert!(d.contains('L'), "{d}");
        assert!(d.ends_with('Z'), "{d}");
        assert!(!d.contains('A'), "TS paint uses polygon samples, not arcTo: {d}");
        assert!(d.len() > 200, "expected dense polygon, got {}", d.len());
    }
}
