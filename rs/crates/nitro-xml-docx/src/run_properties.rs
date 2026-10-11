// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/run-properties.ts; changes listed in
// PROVENANCE.md.

use serde_json::{json, Map, Value};

use nitro_xml_opc::xml::Element;

use crate::borders::parse_shading_fill;
use crate::json_util::{insert_bool, insert_i64, insert_number, insert_str, omit_undefined_object};
use crate::ligatures::parse_ligatures;
use crate::simple_types::{
    on_off_element, parse_half_points, parse_rgb_color, parse_signed_half_points,
    parse_signed_twips, parse_text_scale, parse_tint_shade,
};
use crate::xml_util::{first_w, get_w};

pub fn parse_run_properties(props: Option<&Element>) -> Value {
    let Some(props) = props else {
        return Value::Object(Map::new());
    };
    let mut result = Map::new();
    if let Some(lig) = parse_ligatures(props) {
        result.insert("ligatures".into(), Value::String(lig));
    }
    let toggle = |local: &str| on_off_element(first_w(Some(props), local));
    for (local, key) in [
        ("b", "bold"),
        ("i", "italic"),
        ("bCs", "boldComplexScript"),
        ("iCs", "italicComplexScript"),
        ("caps", "caps"),
        ("smallCaps", "smallCaps"),
        ("vanish", "vanish"),
        ("strike", "strike"),
        ("dstrike", "doubleStrike"),
    ] {
        insert_bool(&mut result, key, toggle(local));
    }
    let underline = first_w(Some(props), "u");
    let underline_value = get_w(underline, "val").map(|s| s.to_lowercase());
    if underline.is_some() {
        if matches!(
            underline_value.as_deref(),
            Some("none") | Some("0") | Some("false") | Some("off")
        ) {
            result.insert("underline".into(), json!(false));
        } else {
            result.insert("underline".into(), json!(true));
            if let Some(uv) = underline_value {
                if uv != "single" && is_word_underline_style(&uv) {
                    result.insert("underlineStyle".into(), json!(uv));
                }
            }
            if let Some(c) = parse_rgb_color(get_w(underline, "color").as_deref()) {
                result.insert("underlineColor".into(), json!(c));
            }
        }
    }
    let highlight = get_w(first_w(Some(props), "highlight"), "val").filter(|v| is_st_highlight(v));
    insert_str(&mut result, "highlight", highlight.map(|s| s.to_owned()));
    let vertical = get_w(first_w(Some(props), "vertAlign"), "val");
    if matches!(
        vertical.as_deref(),
        Some("baseline") | Some("superscript") | Some("subscript")
    ) {
        insert_str(&mut result, "verticalAlign", vertical);
    }
    if let Some(size) = parse_half_points(get_w(first_w(Some(props), "sz"), "val").as_deref()) {
        insert_number(&mut result, "fontSize", size as f64 / 2.0);
    }
    if let Some(size) = parse_half_points(get_w(first_w(Some(props), "szCs"), "val").as_deref()) {
        insert_number(&mut result, "fontSizeComplexScript", size as f64 / 2.0);
    }
    let fonts = first_w(Some(props), "rFonts");
    insert_str(
        &mut result,
        "fontFamily",
        get_w(fonts, "ascii")
            .or_else(|| get_w(fonts, "hAnsi"))
            .map(|s| s.to_owned()),
    );
    insert_str(&mut result, "fontFamilyEastAsia", get_w(fonts, "eastAsia"));
    insert_str(&mut result, "fontFamilyComplexScript", get_w(fonts, "cs"));
    let mut font_theme = Map::new();
    for (xml_key, script) in [
        ("asciiTheme", "ascii"),
        ("hAnsiTheme", "hAnsi"),
        ("eastAsiaTheme", "eastAsia"),
        ("cstheme", "cs"),
    ] {
        let value = get_w(fonts, xml_key);
        if let Some(v) = value {
            if v.starts_with("major") {
                font_theme.insert(script.into(), json!("major"));
            } else if v.starts_with("minor") {
                font_theme.insert(script.into(), json!("minor"));
            }
        }
    }
    if !font_theme.is_empty() {
        result.insert("fontTheme".into(), Value::Object(font_theme));
    }
    let color_el = first_w(Some(props), "color");
    if let Some(hex) = parse_rgb_color(get_w(color_el, "val").as_deref()) {
        result.insert("color".into(), json!(hex));
    }
    let theme_color = get_w(color_el, "themeColor").filter(|v| is_st_theme_color(v));
    if let Some(token) = theme_color {
        let mut ct = Map::new();
        ct.insert("token".into(), json!(token));
        if let Some(t) = parse_tint_shade(get_w(color_el, "themeTint").as_deref()) {
            insert_number(&mut ct, "tint", t);
        }
        if let Some(s) = parse_tint_shade(get_w(color_el, "themeShade").as_deref()) {
            insert_number(&mut ct, "shade", s);
        }
        result.insert("colorTheme".into(), Value::Object(ct));
    }
    if let Some(spacing) =
        parse_signed_twips(get_w(first_w(Some(props), "spacing"), "val").as_deref())
    {
        insert_i64(&mut result, "characterSpacingTwips", Some(spacing));
    }
    let scale_element = first_w(Some(props), "w");
    if let Some(scale_element) = scale_element {
        if let Some(scale) =
            parse_text_scale(get_w(Some(scale_element), "val").as_deref().or(Some("100")))
        {
            insert_i64(&mut result, "textScalePercent", Some(scale));
        }
    }
    if let Some(k) = parse_half_points(get_w(first_w(Some(props), "kern"), "val").as_deref()) {
        insert_i64(&mut result, "kerningHalfPoints", Some(k));
    }
    if let Some(p) =
        parse_signed_half_points(get_w(first_w(Some(props), "position"), "val").as_deref())
    {
        insert_i64(&mut result, "positionHalfPoints", Some(p));
    }
    let shd = first_w(Some(props), "shd");
    if let Some(fill) = parse_shading_fill(shd) {
        result.insert("shadingFill".into(), json!(fill));
    }
    let shd_theme = get_w(shd, "themeFill").filter(|v| is_st_theme_color(v));
    if let Some(token) = shd_theme {
        let mut st = Map::new();
        st.insert("token".into(), json!(token));
        if let Some(t) = parse_tint_shade(get_w(shd, "themeFillTint").as_deref()) {
            insert_number(&mut st, "tint", t);
        }
        if let Some(s) = parse_tint_shade(get_w(shd, "themeFillShade").as_deref()) {
            insert_number(&mut st, "shade", s);
        }
        result.insert("shadingThemeFill".into(), Value::Object(st));
    }
    omit_undefined_object(result)
}

fn is_word_underline_style(value: &str) -> bool {
    matches!(
        value,
        "single"
            | "words"
            | "double"
            | "thick"
            | "dotted"
            | "dottedheavy"
            | "dash"
            | "dashheavy"
            | "dashlong"
            | "dashlongheavy"
            | "dotdash"
            | "dotdashheavy"
            | "dotdotdash"
            | "dotdotdashheavy"
            | "wave"
            | "wavyheavy"
            | "wavydouble"
    )
}

fn is_st_highlight(value: &str) -> bool {
    matches!(
        value,
        "black"
            | "blue"
            | "cyan"
            | "green"
            | "magenta"
            | "red"
            | "yellow"
            | "white"
            | "darkBlue"
            | "darkCyan"
            | "darkGreen"
            | "darkMagenta"
            | "darkRed"
            | "darkYellow"
            | "darkGray"
            | "lightGray"
            | "none"
    )
}

fn is_st_theme_color(value: &str) -> bool {
    matches!(
        value,
        "dark1"
            | "light1"
            | "dark2"
            | "light2"
            | "accent1"
            | "accent2"
            | "accent3"
            | "accent4"
            | "accent5"
            | "accent6"
            | "hyperlink"
            | "followedHyperlink"
            | "background1"
            | "text1"
            | "background2"
            | "text2"
    )
}
