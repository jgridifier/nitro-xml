// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/theme.ts with a minimal DrawingML
// colour/font scheme reader (from src/core/drawingml/theme.ts + drawing-color.ts); changes listed
// in PROVENANCE.md.
//! Theme palette and major/minor Latin fonts.

use nitro_xml_opc::xml::{parse_xml_str, Element};

use crate::model::ThemePalette;
use crate::workbook::default_theme;
use crate::xml_util::{a_first, att, elements};

/// SpreadsheetML theme indices: `theme="0"` is lt1, `theme="1"` is dk1.
pub const PALETTE_SLOTS: [&str; 12] = [
    "lt1", "dk1", "lt2", "dk2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6",
    "hlink", "folHlink",
];

/// A scheme colour as `RRGGBB`: `sysClr` reads `lastClr` then `val`; others read `val`.
fn palette_hex(slot: &Element) -> Option<String> {
    for child in elements(slot) {
        let kind = child.local.as_str();
        let is_color = matches!(
            kind,
            "srgbClr" | "schemeClr" | "sysClr" | "prstClr" | "scrgbClr" | "hslClr"
        );
        if !is_color {
            continue;
        }
        let value = if kind == "sysClr" {
            att(Some(child), "lastClr")
                .or_else(|| att(Some(child), "val"))
                .unwrap_or("")
        } else {
            att(Some(child), "val").unwrap_or("")
        };
        if value.len() == 6 && value.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(value.to_ascii_uppercase());
        }
        return None;
    }
    None
}

fn latin_typeface(collection: Option<&Element>) -> Option<String> {
    let latin = a_first(collection, "latin")?;
    att(Some(latin), "typeface")
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// Reads the palette and major/minor Latin fonts of a theme part.
pub fn parse_theme(xml: Option<&str>) -> ThemePalette {
    let defaults = default_theme();
    let mut palette = ThemePalette {
        colors: defaults.colors.clone(),
        major_font: defaults.major_font.clone(),
        minor_font: defaults.minor_font.clone(),
    };
    let Some(xml) = xml else {
        return palette;
    };
    let Ok(doc) = parse_xml_str(xml) else {
        return palette;
    };
    let holder = a_first(Some(&doc.root), "themeElements").unwrap_or(&doc.root);
    if let Some(scheme) = a_first(Some(holder), "clrScheme") {
        palette.colors = PALETTE_SLOTS
            .iter()
            .enumerate()
            .map(|(index, slot)| {
                a_first(Some(scheme), slot)
                    .and_then(palette_hex)
                    .unwrap_or_else(|| {
                        defaults
                            .colors
                            .get(index)
                            .cloned()
                            .unwrap_or_else(|| "000000".into())
                    })
            })
            .collect();
    }
    let font_scheme = a_first(Some(holder), "fontScheme");
    if let Some(major) = latin_typeface(a_first(font_scheme, "majorFont")) {
        palette.major_font = major;
    }
    if let Some(minor) = latin_typeface(a_first(font_scheme, "minorFont")) {
        palette.minor_font = minor;
    }
    palette
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_slot_order_lt1_before_dk1() {
        // Catches: SpreadsheetML theme="0" is lt1 (background), not DrawingML dk1-first order.
        assert_eq!(PALETTE_SLOTS[0], "lt1");
        assert_eq!(PALETTE_SLOTS[1], "dk1");
        let xml = r#"<?xml version="1.0"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <a:themeElements>
    <a:clrScheme name="Office">
      <a:dk1><a:sysClr val="windowText" lastClr="111111"/></a:dk1>
      <a:lt1><a:sysClr val="window" lastClr="EEEEEE"/></a:lt1>
      <a:dk2><a:srgbClr val="222222"/></a:dk2>
      <a:lt2><a:srgbClr val="DDDDDD"/></a:lt2>
      <a:accent1><a:srgbClr val="4472C4"/></a:accent1>
      <a:accent2><a:srgbClr val="ED7D31"/></a:accent2>
      <a:accent3><a:srgbClr val="A5A5A5"/></a:accent3>
      <a:accent4><a:srgbClr val="FFC000"/></a:accent4>
      <a:accent5><a:srgbClr val="5B9BD5"/></a:accent5>
      <a:accent6><a:srgbClr val="70AD47"/></a:accent6>
      <a:hlink><a:srgbClr val="0563C1"/></a:hlink>
      <a:folHlink><a:srgbClr val="954F72"/></a:folHlink>
    </a:clrScheme>
    <a:fontScheme name="Office">
      <a:majorFont><a:latin typeface="HeadingFace"/></a:majorFont>
      <a:minorFont><a:latin typeface="BodyFace"/></a:minorFont>
    </a:fontScheme>
  </a:themeElements>
</a:theme>"#;
        let theme = parse_theme(Some(xml));
        assert_eq!(theme.colors[0], "EEEEEE");
        assert_eq!(theme.colors[1], "111111");
        assert_eq!(theme.major_font, "HeadingFace");
        assert_eq!(theme.minor_font, "BodyFace");
    }
}
