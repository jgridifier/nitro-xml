// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/write/style-xml.ts; changes listed in PROVENANCE.md.
//! Font, fill, border, alignment and number-format fragments.

use crate::model::{Alignment, Border, BorderEdge, Color, Fill, Font, Protection};

use super::xml_out::{attr_bool, attr_f64, attr_str, el, escape_attr, num};

fn argb(rgb: &str) -> String {
    if rgb.len() == 6 {
        format!("FF{}", rgb.to_ascii_uppercase())
    } else {
        rgb.to_ascii_uppercase()
    }
}

pub fn color_xml(color: Option<&Color>, name: &str) -> String {
    let Some(color) = color else {
        return String::new();
    };
    if let Some(rgb) = &color.rgb {
        let attrs = format!(
            "{}{}",
            attr_str("rgb", Some(&argb(rgb))),
            attr_f64("tint", color.tint)
        );
        return el(name, &attrs, "");
    }
    if let Some(theme) = color.theme {
        let attrs = format!(
            "{}{}",
            attr_f64("theme", Some(theme)),
            attr_f64("tint", color.tint)
        );
        return el(name, &attrs, "");
    }
    if let Some(indexed) = color.indexed {
        let attrs = format!(
            "{}{}",
            attr_f64("indexed", Some(indexed)),
            attr_f64("tint", color.tint)
        );
        return el(name, &attrs, "");
    }
    if color.auto == Some(true) {
        return el(name, &attr_bool("auto", Some(true)), "");
    }
    String::new()
}

/// A font: `<font>` for cell formats, `<rPr>` with `rFont` for rich text.
pub fn font_xml(font: &Font, tag: &str, differential: bool) -> String {
    let flag = |name: &str, value: Option<bool>| -> String {
        match value {
            Some(true) => format!("<{name}/>"),
            Some(false) if differential => format!("<{name} val=\"0\"/>"),
            _ => String::new(),
        }
    };
    let mut out = String::new();
    out.push_str(&flag("b", font.bold));
    out.push_str(&flag("i", font.italic));
    out.push_str(&flag("strike", font.strike));
    if let Some(underline) = &font.underline {
        if underline == "single" {
            out.push_str("<u/>");
        } else {
            out.push_str(&el("u", &attr_str("val", Some(underline)), ""));
        }
    }
    if let Some(vert) = &font.vert_align {
        out.push_str(&el("vertAlign", &attr_str("val", Some(vert)), ""));
    }
    if let Some(size) = font.size {
        out.push_str(&el("sz", &attr_str("val", Some(&num(size))), ""));
    }
    out.push_str(&color_xml(font.color.as_ref(), "color"));
    if let Some(name) = &font.name {
        let face = if tag == "rPr" { "rFont" } else { "name" };
        out.push_str(&el(face, &attr_str("val", Some(name)), ""));
    }
    if let Some(family) = font.family {
        out.push_str(&el("family", &attr_str("val", Some(&num(family))), ""));
    }
    if let Some(scheme) = &font.scheme {
        out.push_str(&el("scheme", &attr_str("val", Some(scheme)), ""));
    }
    format!("<{tag}>{out}</{tag}>")
}

pub fn fill_xml(fill: &Fill, differential: bool) -> String {
    match fill {
        Fill::Gradient {
            gradient,
            degree,
            stops,
        } => {
            let body: String = stops
                .iter()
                .map(|stop| {
                    format!(
                        "<stop position=\"{}\">{}</stop>",
                        num(stop.position),
                        color_xml(Some(&stop.color), "color")
                    )
                })
                .collect();
            let kind = if gradient == "path" {
                Some("path")
            } else {
                None
            };
            let attrs = format!("{}{}", attr_str("type", kind), attr_f64("degree", *degree));
            format!("<fill>{}</fill>", el("gradientFill", &attrs, &body))
        }
        Fill::Pattern {
            pattern,
            fg_color,
            bg_color,
        } => {
            if differential && pattern == "solid" {
                let color = fg_color.as_ref().or(bg_color.as_ref());
                let inner = format!(
                    "{}{}",
                    color_xml(color, "fgColor"),
                    color_xml(color, "bgColor")
                );
                return format!(
                    "<fill>{}</fill>",
                    el(
                        "patternFill",
                        &attr_str("patternType", Some("solid")),
                        &inner
                    )
                );
            }
            let colors = format!(
                "{}{}",
                color_xml(fg_color.as_ref(), "fgColor"),
                color_xml(bg_color.as_ref(), "bgColor")
            );
            format!(
                "<fill>{}</fill>",
                el(
                    "patternFill",
                    &attr_str("patternType", Some(pattern)),
                    &colors
                )
            )
        }
    }
}

fn edge_xml(name: &str, value: Option<&BorderEdge>) -> String {
    match value {
        Some(edge) => el(
            name,
            &attr_str("style", Some(&edge.style)),
            &color_xml(edge.color.as_ref(), "color"),
        ),
        None => format!("<{name}/>"),
    }
}

pub fn border_xml(border: &Border) -> String {
    let inner = format!(
        "{}{}{}{}{}",
        edge_xml("left", border.left.as_ref()),
        edge_xml("right", border.right.as_ref()),
        edge_xml("top", border.top.as_ref()),
        edge_xml("bottom", border.bottom.as_ref()),
        edge_xml("diagonal", border.diagonal.as_ref()),
    );
    let attrs = format!(
        "{}{}",
        attr_bool("diagonalUp", border.diagonal_up.filter(|v| *v)),
        attr_bool("diagonalDown", border.diagonal_down.filter(|v| *v)),
    );
    format!("<border{attrs}>{inner}</border>")
}

pub fn alignment_xml(alignment: Option<&Alignment>) -> String {
    let Some(alignment) = alignment else {
        return String::new();
    };
    let attrs = format!(
        "{}{}{}{}{}{}{}",
        attr_str("horizontal", alignment.horizontal.as_deref()),
        attr_str("vertical", alignment.vertical.as_deref()),
        attr_f64("textRotation", alignment.text_rotation),
        attr_bool("wrapText", alignment.wrap_text.filter(|v| *v)),
        attr_f64("indent", alignment.indent),
        attr_bool("shrinkToFit", alignment.shrink_to_fit.filter(|v| *v)),
        attr_f64("readingOrder", alignment.reading_order),
    );
    if attrs.is_empty() {
        String::new()
    } else {
        el("alignment", &attrs, "")
    }
}

pub fn protection_xml(protection: Option<&Protection>) -> String {
    let Some(protection) = protection else {
        return String::new();
    };
    let attrs = format!(
        "{}{}",
        attr_bool("locked", protection.locked),
        attr_bool("hidden", protection.hidden),
    );
    if attrs.is_empty() {
        String::new()
    } else {
        el("protection", &attrs, "")
    }
}

pub fn num_fmt_xml(id: u32, code: &str) -> String {
    format!(
        "<numFmt numFmtId=\"{id}\" formatCode=\"{}\"/>",
        escape_attr(code)
    )
}
