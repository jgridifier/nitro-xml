// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/drawingml/drawing-color.ts; changes listed in PROVENANCE.md.

use serde::{Deserialize, Serialize};

use super::color_linear::{linear_to_srgb255, srgb255_to_linear};
use super::color_primitives::{
    hex_to_rgb_channels, hsl_to_rgb, parse_drawing_fraction, parse_drawing_hue_degrees,
    parse_drawing_percent, rgb_to_hsl, scrgb_linear_to_srgb8, to_hex,
};
use super::dom::elements;
use super::ordered_color_transforms::ordered_color_transforms;
use super::types::{ColorTransform, DrawingColor, DrawingColorKind};
use crate::xml::{ns, Element};

const APPLIED: &[&str] = &[
    "alpha", "alphaMod", "alphaOff", "comp", "inv", "gray", "shade", "tint", "hue", "hueMod",
    "hueOff", "sat", "satMod", "satOff", "lum", "lumMod", "lumOff",
];

fn color_kind_for_local(local: &str) -> Option<DrawingColorKind> {
    match local {
        "srgbClr" => Some(DrawingColorKind::Srgb),
        "schemeClr" => Some(DrawingColorKind::Scheme),
        "sysClr" => Some(DrawingColorKind::System),
        "prstClr" => Some(DrawingColorKind::Preset),
        "scrgbClr" => Some(DrawingColorKind::Scrgb),
        "hslClr" => Some(DrawingColorKind::Hsl),
        _ => None,
    }
}

fn color_value(kind: DrawingColorKind, element: &Element) -> String {
    let attr = |name: &str| super::dom::attr(element, name).unwrap_or("");
    match kind {
        DrawingColorKind::Scrgb => {
            format!("{},{},{}", attr("r"), attr("g"), attr("b"))
        }
        DrawingColorKind::Hsl => {
            format!("{},{},{}", attr("hue"), attr("sat"), attr("lum"))
        }
        _ => attr("val").to_string(),
    }
}

pub fn parse_drawing_color_in(container: Option<&Element>) -> Option<DrawingColor> {
    let container = container?;
    for element in elements(container) {
        if color_kind_for_local(&element.local).is_none() {
            continue;
        }
        if element.ns.is_some() && element.ns.as_deref() != Some(ns::A) {
            continue;
        }
        return parse_drawing_color(element);
    }
    None
}

pub fn parse_drawing_color(element: &Element) -> Option<DrawingColor> {
    let kind = color_kind_for_local(&element.local)?;
    let transforms = elements(element)
        .into_iter()
        .map(|child| ColorTransform {
            name: child.local.clone(),
            value: super::dom::attr(child, "val").unwrap_or("").to_string(),
        })
        .collect();
    let mut color = DrawingColor {
        kind,
        value: color_value(kind, element),
        transforms,
        fallback: None,
    };
    if kind == DrawingColorKind::System {
        color.fallback = super::dom::attr(element, "lastClr").map(|s| s.to_string());
    }
    Some(color)
}

pub fn parse_drawing_color_list(container: Option<&Element>) -> Vec<DrawingColor> {
    let Some(container) = container else {
        return Vec::new();
    };
    elements(container)
        .into_iter()
        .filter_map(parse_drawing_color)
        .collect()
}

pub trait DrawingColorTheme {
    fn scheme(&self, name: &str) -> Option<String>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedDrawingColor {
    pub hex: String,
    pub alpha: f64,
    pub unapplied: Vec<String>,
}

impl DrawingColorTheme for std::collections::HashMap<String, String> {
    fn scheme(&self, name: &str) -> Option<String> {
        self.get(name).cloned()
    }
}

const PRESET_COLORS: &[(&str, &str)] = &[
    ("black", "#000000"),
    ("white", "#FFFFFF"),
    ("red", "#FF0000"),
    ("green", "#008000"),
    ("blue", "#0000FF"),
    ("yellow", "#FFFF00"),
    ("gray", "#808080"),
    ("grey", "#808080"),
];

fn base_hex(color: &DrawingColor, theme: Option<&dyn DrawingColorTheme>) -> Option<String> {
    match color.kind {
        DrawingColorKind::Srgb => {
            let v = color.value.as_str();
            if v.len() == 6 && v.chars().all(|c| c.is_ascii_hexdigit()) {
                Some(format!("#{}", v.to_uppercase()))
            } else {
                None
            }
        }
        DrawingColorKind::Scheme => theme
            .and_then(|t| t.scheme(&color.value))
            .map(|h| h.to_uppercase()),
        DrawingColorKind::System => color
            .fallback
            .as_ref()
            .filter(|f| f.len() == 6 && f.chars().all(|c| c.is_ascii_hexdigit()))
            .map(|f| format!("#{}", f.to_uppercase())),
        DrawingColorKind::Preset => PRESET_COLORS
            .iter()
            .find(|(k, _)| *k == color.value)
            .map(|(_, v)| v.to_string()),
        DrawingColorKind::Scrgb => {
            let parts: Vec<&str> = color.value.split(',').collect();
            if parts.len() != 3 {
                return None;
            }
            let r = parse_drawing_fraction(parts[0])?;
            let g = parse_drawing_fraction(parts[1])?;
            let b = parse_drawing_fraction(parts[2])?;
            Some(format!(
                "#{}{}{}",
                to_hex(scrgb_linear_to_srgb8(r) as f64),
                to_hex(scrgb_linear_to_srgb8(g) as f64),
                to_hex(scrgb_linear_to_srgb8(b) as f64)
            ))
        }
        DrawingColorKind::Hsl => {
            let parts: Vec<&str> = color.value.split(',').collect();
            if parts.len() != 3 {
                return None;
            }
            let hue = parse_drawing_hue_degrees(parts[0])?;
            let sat = parse_drawing_fraction(parts[1])?;
            let lum = parse_drawing_fraction(parts[2])?;
            let (r, g, b) = hsl_to_rgb(hue, sat, lum, |c| c.round(), true);
            Some(format!("#{}{}{}", to_hex(r), to_hex(g), to_hex(b)))
        }
    }
}

pub struct ResolveOptions {
    pub hsl_rounding: HslRounding,
    pub transform_order: TransformOrder,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum HslRounding {
    #[default]
    Nearest,
    HalfDown,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum TransformOrder {
    #[default]
    Legacy,
    Document,
}

pub fn resolve_drawing_color(
    color: &DrawingColor,
    theme: Option<&dyn DrawingColorTheme>,
    options: Option<ResolveOptions>,
) -> Option<ResolvedDrawingColor> {
    let opts = options.unwrap_or_default();
    let base = base_hex(color, theme)?;
    let rgb = hex_to_rgb_channels(&base)?;
    let unapplied = || {
        let mut names: Vec<String> = color.transforms.iter().map(|t| t.name.clone()).collect();
        names.sort();
        names.dedup();
        names
            .into_iter()
            .filter(|n| !APPLIED.contains(&n.as_str()))
            .collect()
    };

    if opts.transform_order == TransformOrder::Document {
        let value = ordered_color_transforms(rgb, &color.transforms);
        let round = match opts.hsl_rounding {
            HslRounding::HalfDown => |c: f64| (c - 0.5).ceil(),
            HslRounding::Nearest => |c: f64| c.round(),
        };
        return Some(ResolvedDrawingColor {
            hex: format!(
                "#{}{}{}",
                to_hex(round(value.r)),
                to_hex(round(value.g)),
                to_hex(round(value.b))
            ),
            alpha: value.alpha,
            unapplied: unapplied(),
        });
    }

    let (mut r, mut g, mut b) = rgb;
    let find = |name: &str| {
        color
            .transforms
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.value.as_str())
    };
    let has = |name: &str| color.transforms.iter().any(|t| t.name == name);

    if has("comp") {
        let hsl = rgb_to_hsl(r, g, b);
        let rgb2 = hsl_to_rgb((hsl.h + 180.0) % 360.0, hsl.s, hsl.l, |c| c.round(), true);
        r = rgb2.0;
        g = rgb2.1;
        b = rgb2.2;
    }
    if has("inv") {
        r = 255.0 - r;
        g = 255.0 - g;
        b = 255.0 - b;
    }
    if has("gray") {
        let gray = (0.299 * r + 0.587 * g + 0.114 * b).round();
        r = gray;
        g = gray;
        b = gray;
    }

    if let Some(shade) = find("shade").and_then(parse_drawing_percent) {
        r = linear_to_srgb255(srgb255_to_linear(r) * shade);
        g = linear_to_srgb255(srgb255_to_linear(g) * shade);
        b = linear_to_srgb255(srgb255_to_linear(b) * shade);
    }
    if let Some(tint) = find("tint").and_then(parse_drawing_percent) {
        r = linear_to_srgb255(1.0 - (1.0 - srgb255_to_linear(r)) * tint);
        g = linear_to_srgb255(1.0 - (1.0 - srgb255_to_linear(g)) * tint);
        b = linear_to_srgb255(1.0 - (1.0 - srgb255_to_linear(b)) * tint);
    }

    if [
        "hue", "hueMod", "hueOff", "sat", "satMod", "satOff", "lum", "lumMod", "lumOff",
    ]
    .iter()
    .any(|n| has(n))
    {
        let hsl = rgb_to_hsl(r, g, b);
        let wrap = |degrees: f64| ((degrees % 360.0) + 360.0) % 360.0;
        let mut h = hsl.h;
        let mut s = hsl.s;
        let mut l = hsl.l;
        if let Some(v) = find("hue").and_then(parse_drawing_hue_degrees) {
            h = wrap(v);
        }
        if let Some(v) = find("hueMod").and_then(parse_drawing_fraction) {
            h = wrap(h * v);
        }
        if let Some(v) = find("hueOff").and_then(parse_drawing_hue_degrees) {
            h = wrap(h + v);
        }
        if let Some(v) = find("sat").and_then(parse_drawing_fraction) {
            s = super::color_primitives::clamp_unit_interval(v);
        }
        if let Some(v) = find("satMod").and_then(parse_drawing_fraction) {
            s = super::color_primitives::clamp_unit_interval(s * v);
        }
        if let Some(v) = find("satOff").and_then(parse_drawing_fraction) {
            s = super::color_primitives::clamp_unit_interval(s + v);
        }
        if let Some(v) = find("lum").and_then(parse_drawing_fraction) {
            l = super::color_primitives::clamp_unit_interval(v);
        }
        if let Some(v) = find("lumMod").and_then(parse_drawing_fraction) {
            l = super::color_primitives::clamp_unit_interval(l * v);
        }
        if let Some(v) = find("lumOff").and_then(parse_drawing_fraction) {
            l = super::color_primitives::clamp_unit_interval(l + v);
        }
        let round = match opts.hsl_rounding {
            HslRounding::HalfDown => |c: f64| (c - 0.5).ceil(),
            HslRounding::Nearest => |c: f64| c.round(),
        };
        let rgb2 = hsl_to_rgb(h, s, l, round, true);
        r = rgb2.0;
        g = rgb2.1;
        b = rgb2.2;
    }

    let mut alpha = find("alpha").and_then(parse_drawing_percent).unwrap_or(1.0);
    if let Some(v) = find("alphaMod").and_then(parse_drawing_fraction) {
        alpha = super::color_primitives::clamp_unit_interval(alpha * v);
    }
    if let Some(v) = find("alphaOff").and_then(parse_drawing_fraction) {
        alpha = super::color_primitives::clamp_unit_interval(alpha + v);
    }

    Some(ResolvedDrawingColor {
        hex: format!("#{}{}{}", to_hex(r), to_hex(g), to_hex(b)),
        alpha,
        unapplied: unapplied(),
    })
}

impl Default for ResolveOptions {
    fn default() -> Self {
        Self {
            hsl_rounding: HslRounding::Nearest,
            transform_order: TransformOrder::Legacy,
        }
    }
}
