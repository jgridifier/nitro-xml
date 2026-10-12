// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/visual-effects.ts,
// src/core/pptx/core/core/builders/PptxShapeEffectStyleExtractor.ts; changes in PROVENANCE.md.

use crate::inheritance::child_local;
use nitro_xml_opc::drawingml::{
    emu_to_pixels, hex_to_rgb_channels, parse_drawing_color_in, resolve_drawing_color,
    DrawingColorTheme,
};
use nitro_xml_opc::xml::{ns, Element};

/// Build CSS `box-shadow` from `a:spPr/a:effectLst/a:outerShdw`.
///
/// Offset: `dir` is in 60000ths of a degree; `angle_deg = dir / 60000`.
/// `dx = round(cos(angle_rad) * dist_px)`, `dy = round(sin(angle_rad) * dist_px)` with
/// `dist_px = emu_to_pixels(dist)`. OOXML `blurRad` is halved for CSS blur (radius → σ).
pub(crate) fn box_shadow_from_sp_pr(
    sp_pr: &Element,
    theme: &dyn DrawingColorTheme,
    rotation_deg: Option<f64>,
) -> Option<String> {
    let effect_lst = child_local(sp_pr, "effectLst", ns::A)?;
    box_shadow_from_effect_lst(effect_lst, theme, rotation_deg)
}

/// Build CSS `box-shadow` from `a:effectLst/a:outerShdw` (theme `effectStyleLst` or `spPr`).
pub(crate) fn box_shadow_from_effect_lst(
    effect_lst: &Element,
    theme: &dyn DrawingColorTheme,
    rotation_deg: Option<f64>,
) -> Option<String> {
    let outer = child_local(effect_lst, "outerShdw", ns::A)?;
    box_shadow_from_outer_shdw(outer, theme, rotation_deg)
}

fn box_shadow_from_outer_shdw(
    outer: &Element,
    theme: &dyn DrawingColorTheme,
    rotation_deg: Option<f64>,
) -> Option<String> {
    let color = parse_drawing_color_in(Some(outer))?;
    let resolved = resolve_drawing_color(&color, Some(theme), None)?;
    if resolved.hex.is_empty() {
        return None;
    }

    let blur_emu = attr_i64(outer, "blurRad").unwrap_or(0).max(0);
    let dist_emu = attr_i64(outer, "dist").unwrap_or(0).max(0);
    let dir_units = attr_i64(outer, "dir").unwrap_or(0);

    let mut angle_deg = dir_units as f64 / 60_000.0;
    if shadow_rotate_with_shape(outer) == Some(false) {
        if let Some(rot) = rotation_deg {
            angle_deg -= rot;
        }
    }
    let angle_rad = angle_deg.to_radians();
    let distance_px = emu_to_pixels(dist_emu);
    let offset_x = (angle_rad.cos() * distance_px).round();
    let offset_y = (angle_rad.sin() * distance_px).round();

    let blur_px = (emu_to_pixels(blur_emu) / 2.0).max(0.0).round();

    let color_css = shadow_color_css(&resolved.hex, resolved.alpha)?;
    Some(format!(
        "{}px {}px {}px {}",
        format_px(offset_x),
        format_px(offset_y),
        format_px(blur_px),
        color_css
    ))
}

fn shadow_color_css(hex: &str, alpha: f64) -> Option<String> {
    if alpha >= 0.999 {
        return Some(hex.to_owned());
    }
    let (r, g, b) = hex_to_rgb_channels(hex)?;
    let alpha_s = if (alpha * 100.0).round() / 100.0 == alpha {
        format!("{}", alpha)
    } else {
        format!("{:.2}", alpha)
    };
    Some(format!(
        "rgba({}, {}, {}, {})",
        r.round() as u8,
        g.round() as u8,
        b.round() as u8,
        alpha_s
    ))
}

fn attr_i64(el: &Element, local: &str) -> Option<i64> {
    el.attrs
        .iter()
        .find(|a| a.local == local)
        .and_then(|a| a.value.parse().ok())
}

fn shadow_rotate_with_shape(el: &Element) -> Option<bool> {
    let raw = el
        .attrs
        .iter()
        .find(|a| a.local == "rotWithShape")
        .map(|a| a.value.as_str());
    match raw {
        None => None,
        Some("0" | "false") => Some(false),
        Some("1" | "true") => Some(true),
        Some(other) => other.parse().ok(),
    }
}

fn format_px(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nitro_xml_opc::xml::parse_xml_str;

    struct EmptyTheme;

    impl DrawingColorTheme for EmptyTheme {
        fn scheme(&self, _name: &str) -> Option<String> {
            None
        }
    }

    const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

    // Catches: outerShdw dir treated as radians so shadow offsets collapse toward zero.
    #[test]
    fn dir_dist_map_to_box_shadow_offset() {
        let xml = format!(
            r#"<a:outerShdw xmlns:a="{A_NS}" blurRad="50800" dist="38100" dir="2700000" rotWithShape="0">
  <a:srgbClr val="000000"><a:alpha val="40000"/></a:srgbClr>
</a:outerShdw>"#
        );
        let sp_pr_xml =
            format!(r#"<a:spPr xmlns:a="{A_NS}"><a:effectLst>{xml}</a:effectLst></a:spPr>"#);
        let sp_doc = parse_xml_str(&sp_pr_xml).expect("spPr");
        let shadow = box_shadow_from_sp_pr(&sp_doc.root, &EmptyTheme, None).expect("shadow");
        assert!(
            shadow.contains("3px 3px"),
            "45° dist≈4px → ~3px offsets, got {shadow}"
        );
        assert!(
            shadow.contains("3px"),
            "blurRad 50800 → ~3px blur, got {shadow}"
        );
        assert!(shadow.contains("rgba("), "alpha 40000 → rgba, got {shadow}");
    }

    // Catches: rotWithShape=false shadow not counter-rotated with the shape.
    #[test]
    fn rot_with_shape_false_counter_rotates_dir() {
        let xml = format!(
            r#"<a:outerShdw xmlns:a="{A_NS}" dist="38100" dir="2700000" rotWithShape="0">
  <a:srgbClr val="000000"/>
</a:outerShdw>"#
        );
        let sp_pr_xml =
            format!(r#"<a:spPr xmlns:a="{A_NS}"><a:effectLst>{xml}</a:effectLst></a:spPr>"#);
        let sp_doc = parse_xml_str(&sp_pr_xml).expect("spPr");
        let without = box_shadow_from_sp_pr(&sp_doc.root, &EmptyTheme, None).expect("shadow");
        let with_rot =
            box_shadow_from_sp_pr(&sp_doc.root, &EmptyTheme, Some(45.0)).expect("shadow");
        assert_ne!(without, with_rot);
    }
}
