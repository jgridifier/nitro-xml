// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/geometry/ooxml-arc.ts; changes listed in PROVENANCE.md.

use super::guide_eval::angle_to_radians;

pub struct ArcToResult {
    pub svg: String,
    pub end_x: f64,
    pub end_y: f64,
}

pub fn ooxml_arc_to_svg(
    w_r: f64,
    h_r: f64,
    st_ang: f64,
    sw_ang: f64,
    pen_x: f64,
    pen_y: f64,
) -> Option<ArcToResult> {
    if w_r <= 0.0 || h_r <= 0.0 || sw_ang == 0.0 {
        return None;
    }
    let start_rad = angle_to_radians(st_ang);
    let sweep_rad = angle_to_radians(sw_ang);
    let end_rad = start_rad + sweep_rad;
    let cx = pen_x - w_r * start_rad.cos();
    let cy = pen_y - h_r * start_rad.sin();
    let end_x = cx + w_r * end_rad.cos();
    let end_y = cy + h_r * end_rad.sin();
    let large_arc = if sweep_rad.abs() > std::f64::consts::PI {
        1
    } else {
        0
    };
    let sweep = if sweep_rad > 0.0 { 1 } else { 0 };
    let rx = (w_r * 1000.0).round() / 1000.0;
    let ry = (h_r * 1000.0).round() / 1000.0;
    let ex = (end_x * 1000.0).round() / 1000.0;
    let ey = (end_y * 1000.0).round() / 1000.0;
    Some(ArcToResult {
        svg: format!("A {rx} {ry} 0 {large_arc} {sweep} {ex} {ey}"),
        end_x,
        end_y,
    })
}
