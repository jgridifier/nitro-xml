// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/drawingml/theme.ts; changes listed in PROVENANCE.md.

use super::color::parse_drawing_color_in;
use super::dom::attr;
use super::dom::{elements, first};
use super::fill::{parse_drawing_line, parse_fill_element};
use super::theme_model::{
    DrawingTheme, ThemeColorMap, ThemeColorScheme, ThemeColorSlot, ThemeEffectStyle,
    ThemeFontCollection, ThemeFontScheme, ThemeFormatScheme, ThemeSp3dBevel, THEME_COLOR_SLOTS,
};
use super::types::DrawingFill;
use crate::xml::{ns, parse_xml_str, Document, Element};

const MAP_KEYS: &[&str] = &[
    "bg1", "tx1", "bg2", "tx2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6",
    "hlink", "folHlink",
];

pub fn is_theme_color_slot(value: &str) -> bool {
    ThemeColorSlot::parse(value).is_some()
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value.filter(|s| !s.is_empty()).map(|s| s.to_string())
}

pub fn parse_theme_color_scheme(scheme: Option<&Element>) -> ThemeColorScheme {
    let mut result = ThemeColorScheme::default();
    let Some(scheme) = scheme else {
        return result;
    };
    result.name = non_empty(super::dom::attr(scheme, "name"));
    for slot in THEME_COLOR_SLOTS {
        if let Some(color) = parse_drawing_color_in(first(scheme, slot.as_str(), ns::A)) {
            result.colors.insert(slot.as_str().to_string(), color);
        }
    }
    result
}

fn parse_font_collection(collection: Option<&Element>) -> ThemeFontCollection {
    let mut result = ThemeFontCollection::default();
    let Some(collection) = collection else {
        return result;
    };
    let typeface = |local: &str| {
        first(collection, local, ns::A).and_then(|e| non_empty(super::dom::attr(e, "typeface")))
    };
    result.latin = typeface("latin");
    result.east_asia = typeface("ea");
    result.complex_script = typeface("cs");
    for font in elements(collection) {
        if font.local != "font" {
            continue;
        }
        let script = non_empty(super::dom::attr(font, "script"));
        let face = non_empty(super::dom::attr(font, "typeface"));
        if let (Some(script), Some(face)) = (script, face) {
            result.scripts.entry(script).or_insert(face);
        }
    }
    result
}

pub fn parse_theme_font_scheme(scheme: Option<&Element>) -> ThemeFontScheme {
    ThemeFontScheme {
        name: scheme.and_then(|s| non_empty(super::dom::attr(s, "name"))),
        major: parse_font_collection(scheme.and_then(|s| first(s, "majorFont", ns::A))),
        minor: parse_font_collection(scheme.and_then(|s| first(s, "minorFont", ns::A))),
    }
}

fn parse_fill_style_list(list: Option<&Element>) -> Vec<DrawingFill> {
    let Some(list) = list else {
        return Vec::new();
    };
    elements(list)
        .into_iter()
        .filter(|element| {
            matches!(
                element.local.as_str(),
                "noFill" | "solidFill" | "gradFill" | "pattFill" | "blipFill" | "grpFill"
            )
        })
        .filter_map(parse_fill_element)
        .collect()
}

fn parse_line_style_list(list: Option<&Element>) -> Vec<super::types::DrawingLine> {
    let Some(list) = list else {
        return Vec::new();
    };
    elements(list)
        .into_iter()
        .filter(|element| element.local == "ln")
        .filter_map(|ln| parse_drawing_line(Some(ln)))
        .collect()
}

fn parse_bevel_emu(value: Option<&str>) -> Option<i32> {
    value.and_then(|s| s.parse().ok())
}

fn parse_bevel_node(bevel: &Element) -> (String, Option<i32>, Option<i32>) {
    let prst = attr(bevel, "prst")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "circle".to_string());
    let w = parse_bevel_emu(attr(bevel, "w"));
    let h = parse_bevel_emu(attr(bevel, "h"));
    (prst, w, h)
}

fn parse_sp3d_bevel(sp3d: &Element) -> Option<ThemeSp3dBevel> {
    let bevel_t = first(sp3d, "bevelT", ns::A);
    let bevel_b = first(sp3d, "bevelB", ns::A);
    if bevel_t.is_none() && bevel_b.is_none() {
        return None;
    }
    let (top_type, top_width_emu, top_height_emu) = bevel_t
        .map(parse_bevel_node)
        .unwrap_or_else(|| ("none".to_string(), None, None));
    let (bottom_type, bottom_width_emu, bottom_height_emu) = bevel_b
        .map(|b| {
            let (t, w, h) = parse_bevel_node(b);
            (Some(t), w, h)
        })
        .unwrap_or((None, None, None));
    Some(ThemeSp3dBevel {
        top_type,
        top_width_emu,
        top_height_emu,
        bottom_type,
        bottom_width_emu,
        bottom_height_emu,
    })
}

fn parse_effect_style_list(list: Option<&Element>) -> Vec<ThemeEffectStyle> {
    let Some(list) = list else {
        return Vec::new();
    };
    elements(list)
        .into_iter()
        .filter(|element| element.local == "effectStyle")
        .map(|effect_style| {
            let effect_lst = first(effect_style, "effectLst", ns::A)
                .or_else(|| first(effect_style, "effectDag", ns::A))
                .cloned();
            let sp3d = first(effect_style, "sp3d", ns::A);
            let prst_material = sp3d.and_then(|s| non_empty(super::dom::attr(s, "prstMaterial")));
            let sp3d_bevel = sp3d.and_then(parse_sp3d_bevel);
            let has_bevel = sp3d_bevel.is_some();
            ThemeEffectStyle {
                effect_lst,
                prst_material,
                sp3d_bevel,
                has_bevel,
            }
        })
        .collect()
}

/// `a:fmtScheme` fill / line / effect / background-fill style lists.
pub fn parse_theme_format_scheme(fmt: Option<&Element>) -> Option<ThemeFormatScheme> {
    let fmt = fmt?;
    Some(ThemeFormatScheme {
        name: non_empty(super::dom::attr(fmt, "name")),
        fill_styles: parse_fill_style_list(first(fmt, "fillStyleLst", ns::A)),
        line_styles: parse_line_style_list(first(fmt, "lnStyleLst", ns::A)),
        effect_styles: parse_effect_style_list(first(fmt, "effectStyleLst", ns::A)),
        background_fill_styles: parse_fill_style_list(first(fmt, "bgFillStyleLst", ns::A)),
    })
}

pub fn parse_theme(source: &Element) -> DrawingTheme {
    let holder = first(source, "themeElements", ns::A).unwrap_or(source);
    DrawingTheme {
        name: non_empty(super::dom::attr(source, "name")),
        color_scheme: parse_theme_color_scheme(first(holder, "clrScheme", ns::A)),
        font_scheme: parse_theme_font_scheme(first(holder, "fontScheme", ns::A)),
        format_scheme: parse_theme_format_scheme(first(holder, "fmtScheme", ns::A)),
    }
}

pub fn parse_theme_xml(xml: &str, _label: &str) -> DrawingTheme {
    let doc = parse_xml_str(xml).expect("theme xml");
    parse_theme(&doc.root)
}

#[allow(dead_code)]
pub fn parse_theme_document(doc: &Document) -> DrawingTheme {
    parse_theme(&doc.root)
}

pub fn parse_theme_color_map(element: Option<&Element>) -> ThemeColorMap {
    let mut map = ThemeColorMap::new();
    let Some(element) = element else {
        return map;
    };
    for key in MAP_KEYS {
        let value = super::dom::attr(element, key);
        if let Some(value) = value {
            if let Some(slot) = ThemeColorSlot::parse(value) {
                map.insert(key.to_string(), slot);
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: a:fmtScheme fillStyleLst/bgFillStyleLst/lnStyleLst dropped, so fillRef idx cannot resolve.
    #[test]
    fn format_scheme_keeps_fill_line_and_background_lists() {
        let xml = r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office">
          <a:themeElements>
            <a:clrScheme name="Office"/>
            <a:fontScheme name="Office"><a:majorFont/><a:minorFont/></a:fontScheme>
            <a:fmtScheme name="Office">
              <a:fillStyleLst>
                <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
                <a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs></a:gsLst><a:lin ang="0"/></a:gradFill>
              </a:fillStyleLst>
              <a:lnStyleLst>
                <a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
                <a:ln w="25400"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
              </a:lnStyleLst>
              <a:bgFillStyleLst>
                <a:solidFill><a:srgbClr val="ABCDEF"/></a:solidFill>
              </a:bgFillStyleLst>
              <a:effectStyleLst>
                <a:effectStyle>
                  <a:effectLst>
                    <a:outerShdw blurRad="19050" dist="38100" dir="2700000" rotWithShape="0">
                      <a:schemeClr val="phClr"><a:alpha val="45000"/></a:schemeClr>
                    </a:outerShdw>
                  </a:effectLst>
                </a:effectStyle>
              </a:effectStyleLst>
            </a:fmtScheme>
          </a:themeElements>
        </a:theme>"#;
        let theme = parse_theme_xml(xml, "t");
        let fmt = theme.format_scheme.expect("fmtScheme");
        assert_eq!(fmt.fill_styles.len(), 2);
        assert!(matches!(fmt.fill_styles[1], DrawingFill::Gradient { .. }));
        assert_eq!(fmt.line_styles.len(), 2);
        assert_eq!(fmt.line_styles[1].width_emu, Some(25400));
        assert_eq!(fmt.background_fill_styles.len(), 1);
        match &fmt.background_fill_styles[0] {
            DrawingFill::Solid { color } => assert_eq!(color.value, "ABCDEF"),
            other => panic!("expected solid bg fill, got {other:?}"),
        }
        assert_eq!(fmt.effect_styles.len(), 1);
        let lst = fmt.effect_styles[0].effect_lst.as_ref().expect("effectLst");
        assert_eq!(lst.local, "effectLst");
        assert!(
            first(lst, "outerShdw", ns::A).is_some(),
            "expected outerShdw in parsed effect style"
        );
    }

    // Catches: effectStyleLst sp3d/@prstMaterial dropped so effectRef idx=2 never picks up metal.
    #[test]
    fn effect_style_parses_sp3d_preset_material() {
        let xml = r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <a:themeElements>
            <a:clrScheme name="Office"/>
            <a:fontScheme name="Office"><a:majorFont/><a:minorFont/></a:fontScheme>
            <a:fmtScheme name="Office">
              <a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst>
              <a:lnStyleLst><a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst>
              <a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst>
              <a:effectStyleLst>
                <a:effectStyle><a:effectLst/></a:effectStyle>
                <a:effectStyle>
                  <a:effectLst/>
                  <a:sp3d prstMaterial="metal"><a:bevelT w="10000" h="10000"/></a:sp3d>
                </a:effectStyle>
              </a:effectStyleLst>
            </a:fmtScheme>
          </a:themeElements>
        </a:theme>"#;
        let theme = parse_theme_xml(xml, "t");
        let fmt = theme.format_scheme.expect("fmtScheme");
        assert_eq!(fmt.effect_styles.len(), 2);
        assert!(fmt.effect_styles[0].prst_material.is_none());
        assert_eq!(fmt.effect_styles[1].prst_material.as_deref(), Some("metal"));
        assert!(fmt.effect_styles[1].has_bevel);
        let bevel = fmt.effect_styles[1]
            .sp3d_bevel
            .as_ref()
            .expect("sp3d bevel");
        assert_eq!(bevel.top_type, "circle");
        assert_eq!(bevel.top_width_emu, Some(10_000));
        assert_eq!(bevel.top_height_emu, Some(10_000));
    }
}
