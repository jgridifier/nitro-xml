// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/write/theme.ts; changes listed in PROVENANCE.md.
//! Theme part: carry the source when the palette is unchanged, else write a default Office theme.

use nitro_xml_opc::drawingml::THEME_COLOR_SLOTS;
use nitro_xml_opc::xml::ns;

use crate::model::ThemePalette;
use crate::theme::{parse_theme, PALETTE_SLOTS};

use super::xml_out::{escape_attr, XML_HEADER};

fn color_of(theme: &ThemePalette, slot: &str) -> String {
    let raw = PALETTE_SLOTS
        .iter()
        .position(|name| *name == slot)
        .and_then(|index| theme.colors.get(index))
        .map(String::as_str)
        .unwrap_or("000000");
    let hex = if raw.len() > 6 {
        &raw[raw.len() - 6..]
    } else {
        raw
    };
    hex.to_ascii_uppercase()
}

fn colors_match(theme: &ThemePalette, parsed: &ThemePalette) -> bool {
    let norm = |colors: &[String]| -> String {
        colors
            .iter()
            .map(|c| {
                let hex = if c.len() > 6 {
                    &c[c.len() - 6..]
                } else {
                    c.as_str()
                };
                hex.to_ascii_uppercase()
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    norm(&theme.colors) == norm(&parsed.colors)
}

/// A complete Office theme part built from the model palette and fonts.
pub fn default_theme_xml(theme: &ThemePalette) -> String {
    let scheme: String = THEME_COLOR_SLOTS
        .iter()
        .map(|slot| {
            let name = slot.as_str();
            format!(
                "<a:{name}><a:srgbClr val=\"{}\"/></a:{name}>",
                color_of(theme, name)
            )
        })
        .collect();
    let font = |kind: &str, face: &str| {
        format!(
            "<a:{kind}><a:latin typeface=\"{}\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:{kind}>",
            escape_attr(face)
        )
    };
    let solid = "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>";
    let line = |w: u32| {
        format!(
            "<a:ln w=\"{w}\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\">{solid}<a:prstDash val=\"solid\"/><a:miter lim=\"800000\"/></a:ln>"
        )
    };
    let effect = "<a:effectStyle><a:effectLst/></a:effectStyle>";
    format!(
        "{XML_HEADER}<a:theme xmlns:a=\"{}\" name=\"Office Theme\"><a:themeElements>\
<a:clrScheme name=\"Office\">{scheme}</a:clrScheme>\
<a:fontScheme name=\"Office\">{}{}</a:fontScheme>\
<a:fmtScheme name=\"Office\"><a:fillStyleLst>{solid}{solid}{solid}</a:fillStyleLst>\
<a:lnStyleLst>{}{}{}</a:lnStyleLst>\
<a:effectStyleLst>{effect}{effect}{effect}</a:effectStyleLst>\
<a:bgFillStyleLst>{solid}{solid}{solid}</a:bgFillStyleLst></a:fmtScheme>\
</a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>",
        ns::A,
        font("majorFont", &theme.major_font),
        font("minorFont", &theme.minor_font),
        line(6350),
        line(12700),
        line(19050),
    )
}

/// `Some(xml)` to write, `None` to carry the source part unchanged.
pub fn theme_part(theme: &ThemePalette, source_xml: Option<&str>) -> Option<String> {
    let Some(source_xml) = source_xml else {
        return Some(default_theme_xml(theme));
    };
    let current = parse_theme(Some(source_xml));
    if current.major_font == theme.major_font
        && current.minor_font == theme.minor_font
        && colors_match(theme, &current)
    {
        return None;
    }
    Some(default_theme_xml(theme))
}
