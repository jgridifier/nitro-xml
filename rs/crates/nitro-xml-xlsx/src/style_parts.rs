// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/style-parts.ts; changes listed in
// PROVENANCE.md.
//! Fonts, fills, borders, colours and the indexed palette.

use nitro_xml_opc::xml::Element;

use crate::model::{Alignment, Border, BorderEdge, Color, Fill, Font, GradientStop, Protection};
use crate::xml_util::{
    att, bool_attr, bool_attr_or, child_val, elements, num_attr, x_children, x_first,
};

/// Reads a `CT_Color`; `palette` resolves overridden legacy indexed colours to RGB.
pub fn parse_color(element: Option<&Element>, palette: Option<&[String]>) -> Option<Color> {
    let element = element?;
    let mut color = Color {
        rgb: None,
        theme: None,
        tint: None,
        indexed: None,
        auto: None,
    };
    let rgb = att(Some(element), "rgb");
    let theme = num_attr(Some(element), "theme");
    let indexed = num_attr(Some(element), "indexed");
    if let Some(rgb) = rgb.filter(|r| is_hex_rgb(r)) {
        color.rgb = Some(rgb.to_ascii_uppercase());
    } else if let Some(theme) = theme {
        color.theme = Some(theme);
    } else if let Some(indexed) = indexed {
        apply_indexed(&mut color, indexed, palette);
    } else if bool_attr_or(Some(element), "auto", false) {
        color.auto = Some(true);
    }
    let tint = num_attr(Some(element), "tint");
    if let Some(tint) = tint {
        if tint != 0.0 {
            color.tint = Some(tint);
        }
    }
    if color.rgb.is_none()
        && color.theme.is_none()
        && color.indexed.is_none()
        && color.auto.is_none()
        && color.tint.is_none()
    {
        None
    } else {
        Some(color)
    }
}

fn apply_indexed(color: &mut Color, indexed: f64, palette: Option<&[String]>) {
    let idx = indexed as usize;
    if let Some(override_rgb) = palette.and_then(|p| p.get(idx)) {
        color.rgb = Some(override_rgb.clone());
    } else {
        color.indexed = Some(indexed);
    }
}

fn is_hex_rgb(rgb: &str) -> bool {
    let len = rgb.len();
    (len == 6 || len == 8) && rgb.chars().all(|c| c.is_ascii_hexdigit())
}

const UNDERLINES: &[&str] = &["single", "double", "singleAccounting", "doubleAccounting"];

/// Reads a `CT_Font` (cell fonts, `rPr`, or a `dxf` font).
pub fn parse_font(
    element: Option<&Element>,
    palette: Option<&[String]>,
    differential: bool,
) -> Font {
    let mut font = Font {
        name: None,
        size: None,
        bold: None,
        italic: None,
        underline: None,
        strike: None,
        color: None,
        vert_align: None,
        family: None,
        scheme: None,
    };
    let Some(element) = element else {
        return font;
    };
    let flag = |local: &str| -> Option<bool> {
        let node = x_first(Some(element), local)?;
        let on = bool_attr_or(Some(node), "val", true);
        if on || differential {
            Some(on)
        } else {
            None
        }
    };
    if let Some(name) = child_val(element, "name").or_else(|| child_val(element, "rFont")) {
        font.name = Some(name.to_owned());
    }
    if let Some(size) = num_attr(x_first(Some(element), "sz"), "val") {
        font.size = Some(size);
    }
    if let Some(bold) = flag("b") {
        font.bold = Some(bold);
    }
    if let Some(italic) = flag("i") {
        font.italic = Some(italic);
    }
    if let Some(strike) = flag("strike") {
        font.strike = Some(strike);
    }
    if let Some(u) = x_first(Some(element), "u") {
        let value = att(Some(u), "val").unwrap_or("single");
        if UNDERLINES.contains(&value) {
            font.underline = Some(value.to_owned());
        }
    }
    if let Some(color) = parse_color(x_first(Some(element), "color"), palette) {
        font.color = Some(color);
    }
    if let Some(vert) = child_val(element, "vertAlign") {
        if vert == "superscript" || vert == "subscript" {
            font.vert_align = Some(vert.to_owned());
        }
    }
    if let Some(family) = num_attr(x_first(Some(element), "family"), "val") {
        font.family = Some(family);
    }
    if let Some(scheme) = child_val(element, "scheme") {
        if scheme == "major" || scheme == "minor" {
            font.scheme = Some(scheme.to_owned());
        }
    }
    font
}

/// Reads a `CT_Fill`.
pub fn parse_fill(
    element: Option<&Element>,
    palette: Option<&[String]>,
    differential: bool,
) -> Fill {
    if let Some(gradient) = x_first(element, "gradientFill") {
        let mut stops = Vec::new();
        for stop in x_children(gradient, "stop") {
            if let Some(color) = parse_color(x_first(Some(stop), "color"), palette) {
                stops.push(GradientStop {
                    position: num_attr(Some(stop), "position").unwrap_or(0.0),
                    color,
                });
            }
        }
        let gradient_kind = if att(Some(gradient), "type") == Some("path") {
            "path"
        } else {
            "linear"
        };
        return Fill::Gradient {
            gradient: gradient_kind.to_owned(),
            degree: num_attr(Some(gradient), "degree"),
            stops,
        };
    }
    let pattern = x_first(element, "patternFill");
    let declared = att(pattern, "patternType");
    let fg = parse_color(x_first(pattern, "fgColor"), palette);
    let bg = parse_color(x_first(pattern, "bgColor"), palette);
    if differential && pattern.is_some() && (declared.is_none() || declared == Some("solid")) {
        if fg.is_none() && bg.is_none() {
            return Fill::Pattern {
                pattern: "none".into(),
                fg_color: None,
                bg_color: None,
            };
        }
        let color = bg.or(fg);
        return match color {
            Some(c) => Fill::Pattern {
                pattern: "solid".into(),
                fg_color: Some(c),
                bg_color: None,
            },
            None => Fill::Pattern {
                pattern: "solid".into(),
                fg_color: None,
                bg_color: None,
            },
        };
    }
    let type_ = declared.unwrap_or("none");
    if type_ == "none" {
        return Fill::Pattern {
            pattern: "none".into(),
            fg_color: None,
            bg_color: None,
        };
    }
    let bg_color = bg.filter(|bg| {
        !(type_ == "solid"
            && bg.indexed == Some(64.0)
            && bg.rgb.is_none()
            && bg.theme.is_none()
            && bg.tint.is_none()
            && bg.auto.is_none())
    });
    Fill::Pattern {
        pattern: type_.to_owned(),
        fg_color: fg,
        bg_color,
    }
}

fn parse_edge(element: Option<&Element>, palette: Option<&[String]>) -> Option<BorderEdge> {
    let style = att(element, "style")?;
    if style == "none" || element.is_none() {
        return None;
    }
    let mut edge = BorderEdge {
        style: style.to_owned(),
        color: None,
    };
    if let Some(color) = parse_color(x_first(element, "color"), palette) {
        edge.color = Some(color);
    }
    Some(edge)
}

pub fn parse_border(element: Option<&Element>, palette: Option<&[String]>) -> Border {
    let mut border = Border::default();
    let Some(element) = element else {
        return border;
    };
    for side in ["left", "right", "top", "bottom", "diagonal"] {
        let node = x_first(Some(element), side).or_else(|| match side {
            "left" => x_first(Some(element), "start"),
            "right" => x_first(Some(element), "end"),
            _ => None,
        });
        if let Some(edge) = parse_edge(node, palette) {
            match side {
                "left" => border.left = Some(edge),
                "right" => border.right = Some(edge),
                "top" => border.top = Some(edge),
                "bottom" => border.bottom = Some(edge),
                "diagonal" => border.diagonal = Some(edge),
                _ => {}
            }
        }
    }
    if bool_attr_or(Some(element), "diagonalUp", false) {
        border.diagonal_up = Some(true);
    }
    if bool_attr_or(Some(element), "diagonalDown", false) {
        border.diagonal_down = Some(true);
    }
    border
}

pub fn parse_alignment(element: Option<&Element>) -> Option<Alignment> {
    let element = element?;
    let mut alignment = Alignment {
        horizontal: None,
        vertical: None,
        wrap_text: None,
        shrink_to_fit: None,
        indent: None,
        text_rotation: None,
        reading_order: None,
    };
    if let Some(horizontal) = att(Some(element), "horizontal") {
        if horizontal != "general" {
            alignment.horizontal = Some(horizontal.to_owned());
        }
    }
    if let Some(vertical) = att(Some(element), "vertical") {
        if vertical != "bottom" {
            alignment.vertical = Some(vertical.to_owned());
        }
    }
    if bool_attr_or(Some(element), "wrapText", false) {
        alignment.wrap_text = Some(true);
    }
    if bool_attr_or(Some(element), "shrinkToFit", false) {
        alignment.shrink_to_fit = Some(true);
    }
    if let Some(indent) = num_attr(Some(element), "indent") {
        if indent != 0.0 {
            alignment.indent = Some(indent);
        }
    }
    if let Some(rotation) = num_attr(Some(element), "textRotation") {
        if rotation != 0.0 {
            alignment.text_rotation = Some(rotation);
        }
    }
    if let Some(order) = num_attr(Some(element), "readingOrder") {
        if order != 0.0 {
            alignment.reading_order = Some(order);
        }
    }
    if alignment.horizontal.is_none()
        && alignment.vertical.is_none()
        && alignment.wrap_text.is_none()
        && alignment.shrink_to_fit.is_none()
        && alignment.indent.is_none()
        && alignment.text_rotation.is_none()
        && alignment.reading_order.is_none()
    {
        None
    } else {
        Some(alignment)
    }
}

pub fn parse_protection(element: Option<&Element>) -> Option<Protection> {
    let element = element?;
    let mut protection = Protection {
        locked: None,
        hidden: None,
    };
    if bool_attr(Some(element), "locked") == Some(false) {
        protection.locked = Some(false);
    }
    if bool_attr_or(Some(element), "hidden", false) {
        protection.hidden = Some(true);
    }
    if protection.locked.is_none() && protection.hidden.is_none() {
        None
    } else {
        Some(protection)
    }
}

/// `<colors><indexedColors>` overrides; returns `RRGGBB`/`AARRGGBB` entries.
pub fn parse_indexed_palette(style_sheet: &Element) -> Option<Vec<String>> {
    let indexed = x_first(x_first(Some(style_sheet), "colors"), "indexedColors")?;
    let colors: Vec<String> = elements(indexed)
        .filter_map(|node| {
            let rgb = att(Some(node), "rgb")?.to_ascii_uppercase();
            if is_hex_rgb(&rgb) {
                Some(rgb)
            } else {
                None
            }
        })
        .collect();
    if colors.is_empty() {
        None
    } else {
        Some(colors)
    }
}
