// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/visual-3d.ts and visual-3d-bevel-light.ts; changes listed in PROVENANCE.md.

use nitro_xml_opc::drawingml::{ThemeSp3dBevel, EMU_PER_PIXEL};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BevelLightVector {
    dx: i32,
    dy: i32,
}

fn get_bevel_highlight_direction(light_rig_direction: Option<&str>) -> BevelLightVector {
    let angle = match light_rig_direction {
        Some("t") => Some(0),
        Some("tr") => Some(45),
        Some("r") => Some(90),
        Some("br") => Some(135),
        Some("b") => Some(180),
        Some("bl") => Some(225),
        Some("l") => Some(270),
        Some("tl") => Some(315),
        _ => None,
    };
    match angle {
        None => BevelLightVector { dx: -1, dy: -1 },
        Some(a) => {
            let bucket = (a / 90) * 90;
            match bucket {
                0 => BevelLightVector { dx: 0, dy: -1 },
                90 => BevelLightVector { dx: 1, dy: 0 },
                180 => BevelLightVector { dx: 0, dy: 1 },
                270 => BevelLightVector { dx: -1, dy: 0 },
                _ => BevelLightVector { dx: -1, dy: -1 },
            }
        }
    }
}

fn is_bevel_profile_inverted(bevel_type: &str) -> bool {
    bevel_type == "softRound"
}

fn emu_to_bevel_px(emu: Option<i32>) -> i32 {
    match emu {
        Some(v) if v > 0 => {
            let px = (v as f64 / EMU_PER_PIXEL as f64).round();
            px.max(1.0) as i32
        }
        _ => 3,
    }
}

fn fmt_opacity(v: f64) -> String {
    let mut s = format!("{:.6}", v);
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

fn get_bevel_shadow(
    bevel_type: &str,
    b_w: i32,
    b_h: i32,
    is_bottom: bool,
    light_vector: BevelLightVector,
) -> String {
    let profile_vector = if is_bevel_profile_inverted(bevel_type) {
        BevelLightVector {
            dx: -light_vector.dx,
            dy: -light_vector.dy,
        }
    } else {
        light_vector
    };
    let hl_dir_x = if is_bottom {
        -profile_vector.dx
    } else {
        profile_vector.dx
    };
    let hl_dir_y = if is_bottom {
        -profile_vector.dy
    } else {
        profile_vector.dy
    };
    let sh_dir_x = -hl_dir_x;
    let sh_dir_y = -hl_dir_y;
    let hl_opacity = if is_bottom { 0.2 } else { 0.3 };
    let sh_opacity = if is_bottom { 0.3 } else { 0.2 };
    let max_dim = b_w.max(b_h);

    match bevel_type {
        "circle" => [
            format!(
                "inset {}px {}px {}px rgba(255,255,255,{})",
                hl_dir_x * b_w,
                hl_dir_y * b_h,
                max_dim + 2,
                fmt_opacity(hl_opacity + 0.12)
            ),
            format!(
                "inset {}px {}px {}px rgba(255,255,255,{})",
                hl_dir_x * ((b_w as f64) * 0.5).round() as i32,
                hl_dir_y * ((b_h as f64) * 0.5).round() as i32,
                max_dim + 4,
                fmt_opacity(hl_opacity * 0.4)
            ),
            format!(
                "inset {}px {}px {}px rgba(0,0,0,{})",
                sh_dir_x * b_w,
                sh_dir_y * b_h,
                max_dim + 2,
                fmt_opacity(sh_opacity + 0.06)
            ),
            format!(
                "inset {}px {}px {}px rgba(0,0,0,{})",
                sh_dir_x * ((b_w as f64) * 0.5).round() as i32,
                sh_dir_y * ((b_h as f64) * 0.5).round() as i32,
                max_dim + 4,
                fmt_opacity(sh_opacity * 0.3)
            ),
        ]
        .join(", "),
        "relaxedInset" => [
            format!(
                "inset {}px {}px {}px rgba(255,255,255,{})",
                hl_dir_x * b_w,
                hl_dir_y * b_h,
                max_dim + 5,
                fmt_opacity(hl_opacity - 0.04)
            ),
            format!(
                "inset {}px {}px {}px rgba(0,0,0,{})",
                sh_dir_x * b_w,
                sh_dir_y * b_h,
                max_dim + 5,
                fmt_opacity(sh_opacity - 0.04)
            ),
            format!(
                "inset 0 0 {}px rgba(0,0,0,{})",
                max_dim + 8,
                fmt_opacity(sh_opacity * 0.15)
            ),
        ]
        .join(", "),
        "hardEdge" => [
            format!(
                "inset {}px {}px 0 rgba(255,255,255,{})",
                hl_dir_x * b_w,
                hl_dir_y * b_h,
                fmt_opacity(hl_opacity + 0.18)
            ),
            format!(
                "inset {}px {}px 0 rgba(0,0,0,{})",
                sh_dir_x * b_w,
                sh_dir_y * b_h,
                fmt_opacity(sh_opacity + 0.18)
            ),
            format!(
                "inset {}px {}px 0 rgba(255,255,255,{})",
                hl_dir_x * ((b_w as f64) * 0.4).round() as i32,
                hl_dir_y * ((b_h as f64) * 0.4).round() as i32,
                fmt_opacity(hl_opacity * 0.3)
            ),
        ]
        .join(", "),
        "cross" => [
            format!(
                "inset {}px 0 {}px rgba(255,255,255,{})",
                hl_dir_x * b_w,
                b_w,
                fmt_opacity(hl_opacity)
            ),
            format!(
                "inset 0 {}px {}px rgba(255,255,255,{})",
                hl_dir_y * b_h,
                b_h,
                fmt_opacity(hl_opacity)
            ),
            format!(
                "inset {}px 0 {}px rgba(0,0,0,{})",
                sh_dir_x * b_w,
                b_w,
                fmt_opacity(sh_opacity)
            ),
            format!(
                "inset 0 {}px {}px rgba(0,0,0,{})",
                sh_dir_y * b_h,
                b_h,
                fmt_opacity(sh_opacity)
            ),
            format!(
                "inset 0 0 {}px rgba(0,0,0,{})",
                ((max_dim as f64) * 0.5).round(),
                fmt_opacity(sh_opacity * 0.2)
            ),
        ]
        .join(", "),
        "coolSlant" => [
            format!(
                "inset {}px {}px {}px rgba(255,255,255,{})",
                hl_dir_x * b_w,
                hl_dir_y * ((b_h as f64) * 0.4).round() as i32,
                max_dim,
                fmt_opacity(hl_opacity + 0.12)
            ),
            format!(
                "inset {}px 0 {}px rgba(255,255,255,{})",
                hl_dir_x * ((b_w as f64) * 0.6).round() as i32,
                ((max_dim as f64) * 0.6).round(),
                fmt_opacity(hl_opacity * 0.4)
            ),
            format!(
                "inset {}px {}px {}px rgba(0,0,0,{})",
                sh_dir_x * ((b_w as f64) * 0.4).round() as i32,
                sh_dir_y * b_h,
                max_dim,
                fmt_opacity(sh_opacity + 0.1)
            ),
        ]
        .join(", "),
        "angle" => [
            format!(
                "inset {}px {}px {}px rgba(255,255,255,{})",
                hl_dir_x * b_w,
                hl_dir_y * b_h,
                ((max_dim as f64) * 0.4).round(),
                fmt_opacity(hl_opacity + 0.16)
            ),
            format!(
                "inset {}px {}px 0 rgba(255,255,255,{})",
                hl_dir_x * ((b_w as f64) * 0.5).round() as i32,
                hl_dir_y * ((b_h as f64) * 0.5).round() as i32,
                fmt_opacity(hl_opacity * 0.5)
            ),
            format!(
                "inset {}px {}px {}px rgba(0,0,0,{})",
                sh_dir_x * b_w,
                sh_dir_y * b_h,
                ((max_dim as f64) * 0.4).round(),
                fmt_opacity(sh_opacity + 0.12)
            ),
        ]
        .join(", "),
        "softRound" => [
            format!(
                "inset {}px {}px {}px rgba(255,255,255,{})",
                hl_dir_x * b_w,
                hl_dir_y * b_h,
                max_dim + 7,
                fmt_opacity(hl_opacity + 0.02)
            ),
            format!(
                "inset {}px {}px {}px rgba(255,255,255,{})",
                hl_dir_x * ((b_w as f64) * 0.3).round() as i32,
                hl_dir_y * ((b_h as f64) * 0.3).round() as i32,
                max_dim + 10,
                fmt_opacity(hl_opacity * 0.3)
            ),
            format!(
                "inset {}px {}px {}px rgba(0,0,0,{})",
                sh_dir_x * b_w,
                sh_dir_y * b_h,
                max_dim + 7,
                fmt_opacity(sh_opacity - 0.04)
            ),
        ]
        .join(", "),
        "convex" => [
            format!(
                "inset 0 0 {}px rgba(255,255,255,{})",
                max_dim + 4,
                fmt_opacity(hl_opacity + 0.06)
            ),
            format!(
                "inset {}px {}px {}px rgba(255,255,255,{})",
                hl_dir_x * b_w,
                hl_dir_y * b_h,
                max_dim,
                fmt_opacity(hl_opacity + 0.02)
            ),
            format!(
                "inset {}px {}px {}px rgba(0,0,0,{})",
                sh_dir_x * b_w,
                sh_dir_y * b_h,
                max_dim,
                fmt_opacity(sh_opacity)
            ),
            format!(
                "inset {}px {}px {}px rgba(0,0,0,{})",
                sh_dir_x * ((b_w as f64) * 1.5).round() as i32,
                sh_dir_y * ((b_h as f64) * 1.5).round() as i32,
                max_dim + 2,
                fmt_opacity(sh_opacity * 0.3)
            ),
        ]
        .join(", "),
        "slope" => [
            format!(
                "inset {}px {}px {}px rgba(255,255,255,{})",
                hl_dir_x * b_w,
                hl_dir_y * b_h,
                max_dim + 4,
                fmt_opacity(hl_opacity + 0.06)
            ),
            format!(
                "inset {}px {}px {}px rgba(255,255,255,{})",
                hl_dir_x * ((b_w as f64) * 0.5).round() as i32,
                hl_dir_y * ((b_h as f64) * 0.5).round() as i32,
                max_dim + 6,
                fmt_opacity(hl_opacity * 0.35)
            ),
            format!(
                "inset {}px {}px {}px rgba(0,0,0,{})",
                sh_dir_x * ((b_w as f64) * 0.7).round() as i32,
                sh_dir_y * ((b_h as f64) * 0.7).round() as i32,
                max_dim,
                fmt_opacity(sh_opacity)
            ),
        ]
        .join(", "),
        "divot" => [
            format!(
                "inset {}px {}px {}px rgba(255,255,255,{})",
                sh_dir_x * ((b_w as f64) * 0.5).round() as i32,
                sh_dir_y * ((b_h as f64) * 0.5).round() as i32,
                ((max_dim as f64) * 0.5).round(),
                fmt_opacity(hl_opacity + 0.06)
            ),
            format!(
                "inset {}px {}px {}px rgba(0,0,0,{})",
                hl_dir_x * ((b_w as f64) * 0.5).round() as i32,
                hl_dir_y * ((b_h as f64) * 0.5).round() as i32,
                ((max_dim as f64) * 0.5).round(),
                fmt_opacity(sh_opacity + 0.12)
            ),
            format!(
                "inset 0 0 {}px rgba(0,0,0,{})",
                ((max_dim as f64) * 0.3).round(),
                fmt_opacity(sh_opacity * 0.3)
            ),
        ]
        .join(", "),
        "riblet" => [
            format!(
                "inset 0 {}px {}px rgba(255,255,255,{})",
                hl_dir_y * b_h,
                ((b_h as f64) * 0.4).round(),
                fmt_opacity(hl_opacity + 0.02)
            ),
            format!(
                "inset 0 {}px {}px rgba(0,0,0,{})",
                sh_dir_y * b_h,
                ((b_h as f64) * 0.4).round(),
                fmt_opacity(sh_opacity)
            ),
            format!(
                "inset 0 {}px {}px rgba(255,255,255,{})",
                hl_dir_y * (b_h * 2),
                b_h,
                fmt_opacity(hl_opacity * 0.45)
            ),
            format!(
                "inset 0 {}px {}px rgba(0,0,0,{})",
                sh_dir_y * (b_h * 2),
                b_h,
                fmt_opacity(sh_opacity * 0.25)
            ),
        ]
        .join(", "),
        "artDeco" => [
            format!(
                "inset {}px {}px 0 rgba(255,255,255,{})",
                hl_dir_x * b_w,
                hl_dir_y * b_h,
                fmt_opacity(hl_opacity + 0.12)
            ),
            format!(
                "inset {}px {}px 0 rgba(255,255,255,{})",
                hl_dir_x * (b_w * 2),
                hl_dir_y * (b_h * 2),
                fmt_opacity(hl_opacity * 0.45)
            ),
            format!(
                "inset {}px {}px 0 rgba(255,255,255,{})",
                hl_dir_x * (b_w * 3),
                hl_dir_y * (b_h * 3),
                fmt_opacity(hl_opacity * 0.2)
            ),
            format!(
                "inset {}px {}px 0 rgba(0,0,0,{})",
                sh_dir_x * b_w,
                sh_dir_y * b_h,
                fmt_opacity(sh_opacity + 0.12)
            ),
            format!(
                "inset {}px {}px 0 rgba(0,0,0,{})",
                sh_dir_x * (b_w * 2),
                sh_dir_y * (b_h * 2),
                fmt_opacity(sh_opacity * 0.4)
            ),
        ]
        .join(", "),
        _ => [
            format!(
                "inset {}px {}px {}px rgba(255,255,255,{})",
                hl_dir_x * b_w,
                hl_dir_y * b_h,
                max_dim,
                fmt_opacity(hl_opacity)
            ),
            format!(
                "inset {}px {}px {}px rgba(0,0,0,{})",
                sh_dir_x * b_w,
                sh_dir_y * b_h,
                max_dim,
                fmt_opacity(sh_opacity)
            ),
        ]
        .join(", "),
    }
}

/// Bevel preset → inset `box-shadow` (top + bottom combined), matching upstream `get3DBevelShadow`.
pub fn get_3d_bevel_shadow(
    bevel: &ThemeSp3dBevel,
    light_rig_direction: Option<&str>,
) -> Option<String> {
    let light_vector = get_bevel_highlight_direction(light_rig_direction);
    let mut parts: Vec<String> = Vec::new();

    if bevel.top_type != "none" {
        let b_w = emu_to_bevel_px(bevel.top_width_emu);
        let b_h = emu_to_bevel_px(bevel.top_height_emu);
        parts.push(get_bevel_shadow(
            &bevel.top_type,
            b_w,
            b_h,
            false,
            light_vector,
        ));
    }

    if let Some(bottom_type) = bevel.bottom_type.as_deref() {
        if bottom_type != "none" {
            let b_w = emu_to_bevel_px(bevel.bottom_width_emu);
            let b_h = emu_to_bevel_px(bevel.bottom_height_emu);
            parts.push(get_bevel_shadow(bottom_type, b_w, b_h, true, light_vector));
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nitro_xml_opc::drawingml::ThemeSp3dBevel;

    // Catches: Profil-like theme metal+bevelT never emits circle inset bevel layers before material specular.
    #[test]
    fn profil_like_metal_bevel_circle_inset_shadow() {
        let bevel = ThemeSp3dBevel {
            top_type: "circle".to_owned(),
            top_width_emu: Some(28_575),
            top_height_emu: Some(28_575),
            bottom_type: None,
            bottom_width_emu: None,
            bottom_height_emu: None,
        };
        let shadow = get_3d_bevel_shadow(&bevel, None).expect("bevel shadow");
        assert!(
            shadow.contains("inset -3px -3px 5px rgba(255,255,255,0.42)"),
            "circle 3px default light, got {shadow}"
        );
        assert!(
            shadow.contains("rgba(0,0,0,"),
            "bevel shadow needs dark inset layer, got {shadow}"
        );
    }

    // Catches: hardEdge bevel preset diverges from upstream get3DBevelShadow at default size.
    #[test]
    fn hard_edge_default_size_matches_upstream_layers() {
        let bevel = ThemeSp3dBevel {
            top_type: "hardEdge".to_owned(),
            top_width_emu: None,
            top_height_emu: None,
            bottom_type: None,
            bottom_width_emu: None,
            bottom_height_emu: None,
        };
        let shadow = get_3d_bevel_shadow(&bevel, None).expect("hardEdge");
        assert!(
            shadow.contains("inset -3px -3px 0 rgba(255,255,255,0.48)"),
            "hardEdge highlight, got {shadow}"
        );
        assert!(
            shadow.contains("inset 3px 3px 0 rgba(0,0,0,0.38)"),
            "hardEdge shadow, got {shadow}"
        );
    }
}
