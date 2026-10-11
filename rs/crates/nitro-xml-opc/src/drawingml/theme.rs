// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/drawingml/theme.ts; changes listed in PROVENANCE.md.

use super::color::parse_drawing_color_in;
use super::dom::{elements, first};
use super::theme_model::{
    DrawingTheme, ThemeColorMap, ThemeColorScheme, ThemeColorSlot, ThemeFontCollection,
    ThemeFontScheme, THEME_COLOR_SLOTS,
};
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

pub fn parse_theme(source: &Element) -> DrawingTheme {
    let holder = first(source, "themeElements", ns::A).unwrap_or(source);
    DrawingTheme {
        name: non_empty(super::dom::attr(source, "name")),
        color_scheme: parse_theme_color_scheme(first(holder, "clrScheme", ns::A)),
        font_scheme: parse_theme_font_scheme(first(holder, "fontScheme", ns::A)),
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
