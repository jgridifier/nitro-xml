// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/theme.ts and diagram-theme.ts; changes
// listed in PROVENANCE.md.

use serde_json::{json, Map, Value};

use nitro_xml_opc::drawingml::{
    parse_theme as parse_drawing_theme, theme_color_slot_for, theme_slot_hex, ThemeColorMap,
    ThemeFontCollection, THEME_COLOR_SLOTS,
};
use nitro_xml_opc::xml::parse_xml_str;

use crate::xml_util::{by_name_w, get_w};

pub fn parse_theme(xml: &str) -> Value {
    let document = parse_xml_str(xml).expect("theme xml");
    let theme = parse_drawing_theme(&document.root);
    let mut colors = Map::new();
    for slot in THEME_COLOR_SLOTS {
        let color = theme.color_scheme.colors.get(slot.as_str());
        if let Some(hex) = theme_slot_hex(color) {
            colors.insert(slot.as_str().into(), json!(hex));
        }
    }
    let major = font_set_of(&theme.font_scheme.major);
    let minor = font_set_of(&theme.font_scheme.minor);
    json!({
        "colors": colors,
        "colorMapping": {},
        "fonts": { "major": major, "minor": minor }
    })
}

fn font_set_of(collection: &ThemeFontCollection) -> Value {
    let mut m = Map::new();
    if let Some(v) = &collection.latin {
        m.insert("latin".into(), json!(v));
    }
    if let Some(v) = &collection.east_asia {
        m.insert("eastAsia".into(), json!(v));
    }
    if let Some(v) = &collection.complex_script {
        m.insert("complexScript".into(), json!(v));
    }
    Value::Object(m)
}

const MAPPING_KEYS: [&str; 4] = ["bg1", "tx1", "bg2", "tx2"];

pub fn parse_color_scheme_mapping(xml: &str) -> Value {
    let document = parse_xml_str(xml).expect("settings xml");
    let mapping = by_name_w(&document.root, "clrSchemeMapping")
        .into_iter()
        .next();
    let mut result = Map::new();
    let Some(mapping) = mapping else {
        return Value::Object(result);
    };
    for key in MAPPING_KEYS {
        let value = get_w(Some(mapping), key);
        if let Some(slot) = value.and_then(|v| token_to_slot(&v)) {
            result.insert(key.into(), json!(slot));
        }
    }
    Value::Object(result)
}

fn token_to_slot(token: &str) -> Option<&'static str> {
    match token {
        "dark1" => Some("dk1"),
        "light1" => Some("lt1"),
        "dark2" => Some("dk2"),
        "light2" => Some("lt2"),
        "accent1" => Some("accent1"),
        "accent2" => Some("accent2"),
        "accent3" => Some("accent3"),
        "accent4" => Some("accent4"),
        "accent5" => Some("accent5"),
        "accent6" => Some("accent6"),
        "hyperlink" => Some("hlink"),
        "followedHyperlink" => Some("folHlink"),
        _ => None,
    }
}

/// Theme-aware colour resolver for diagram/SmartArt (scheme name → `#RRGGBB`).
pub fn diagram_color_theme(theme: Option<&Value>) -> impl Fn(&str) -> Option<String> {
    let theme = theme.cloned();
    move |name: &str| {
        let theme = theme.as_ref()?;
        let map = color_map_from_theme(theme);
        let slot = theme_color_slot_for(name, &map)?;
        let colors = theme.get("colors").and_then(|c| c.as_object())?;
        let hex = colors.get(slot.as_str())?.as_str()?;
        Some(format!("#{}", hex.replace('#', "").to_uppercase()))
    }
}

fn color_map_from_theme(theme: &Value) -> ThemeColorMap {
    let mut map = ThemeColorMap::new();
    let Some(mapping) = theme.get("colorMapping").and_then(|m| m.as_object()) else {
        return map;
    };
    for (key, value) in mapping {
        if let Some(slot_name) = value.as_str() {
            if let Some(slot) = nitro_xml_opc::drawingml::ThemeColorSlot::parse(slot_name) {
                map.insert(key.clone(), slot);
            }
        }
    }
    map
}
