// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/core/runtime/PptxHandlerRuntimeThemeLoading.ts;
// theme colour/font extraction via nitro_xml_opc::drawingml (#25); changes in PROVENANCE.md.

use std::collections::BTreeMap;

use nitro_xml_opc::drawingml::{
    parse_theme, theme_slot_hex, DrawingTheme, ThemeColorSlot, ThemeFormatScheme, THEME_COLOR_SLOTS,
};
use nitro_xml_opc::xml::{ns, Element};
use serde::Serialize;

use crate::inheritance::child_local;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeFonts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub east_asia: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complex_script: Option<String>,
    /// Non-empty `a:ea` only; `east_asia` above keeps upstream's latin fallback for the model dump.
    #[serde(skip)]
    pub east_asia_explicit: Option<String>,
    /// Per-script faces from `<a:font script="Hans|Hant|Jpan|…">` (upstream `byScript`);
    /// not part of the model dump.
    #[serde(skip)]
    pub scripts: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeFontScheme {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub major_font: Option<ThemeFonts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minor_font: Option<ThemeFonts>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_scheme: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_scheme: Option<ThemeFontScheme>,
    /// Style matrix for `a:fillRef` / `a:lnRef` / `a:bgRef`. Not part of the issue-24 dump.
    #[serde(skip)]
    pub format_scheme: Option<ThemeFormatScheme>,
}

/// Office defaults used when a theme slot is absent (matches upstream pptx loader).
pub fn default_scheme_color_map() -> BTreeMap<String, String> {
    [
        ("dk1", "#000000"),
        ("lt1", "#FFFFFF"),
        ("dk2", "#1F497D"),
        ("lt2", "#EEECE1"),
        ("accent1", "#4472C4"),
        ("accent2", "#ED7D31"),
        ("accent3", "#A5A5A5"),
        ("accent4", "#FFC000"),
        ("accent5", "#5B9BD5"),
        ("accent6", "#70AD47"),
        ("hlink", "#0563C1"),
        ("folHlink", "#954F72"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect()
}

pub fn parse_theme_part(root: &Element) -> Theme {
    let theme_root = if root.local == "theme" && root.ns.as_deref() == Some(ns::A) {
        root
    } else {
        child_local(root, "theme", ns::A).unwrap_or(root)
    };
    let drawing = parse_theme(theme_root);
    theme_from_drawing(drawing)
}

fn theme_from_drawing(drawing: DrawingTheme) -> Theme {
    let mut color_map = default_scheme_color_map();
    for slot in THEME_COLOR_SLOTS {
        if let Some(hex) = theme_slot_hex(drawing.color_scheme.colors.get(slot.as_str())) {
            let hex = hex.trim_start_matches('#').to_uppercase();
            if hex.len() == 6 {
                color_map.insert(slot.as_str().to_owned(), format!("#{hex}"));
            }
        }
    }
    // Keep stable key order for dumps that iterate insertion order elsewhere.
    let _ = ThemeColorSlot::Dk1;

    let major = font_group(
        drawing.font_scheme.major.latin.clone(),
        drawing.font_scheme.major.east_asia.clone(),
        drawing.font_scheme.major.complex_script.clone(),
        drawing.font_scheme.major.scripts.clone(),
    );
    let minor = font_group(
        drawing.font_scheme.minor.latin.clone(),
        drawing.font_scheme.minor.east_asia.clone(),
        drawing.font_scheme.minor.complex_script.clone(),
        drawing.font_scheme.minor.scripts.clone(),
    );
    let font_scheme = if major.is_some() || minor.is_some() {
        Some(ThemeFontScheme {
            major_font: major,
            minor_font: minor,
        })
    } else {
        None
    };

    Theme {
        color_scheme: Some(color_map),
        font_scheme,
        format_scheme: drawing.format_scheme,
    }
}

fn font_group(
    latin: Option<String>,
    east_asia: Option<String>,
    complex_script: Option<String>,
    scripts: impl IntoIterator<Item = (String, String)>,
) -> Option<ThemeFonts> {
    let latin = latin.filter(|s| !s.is_empty());
    // Empty `a:ea` stays model-visible as latin (upstream), but `+mn-ea` may fall
    // through to theme script faces (Hans/…) via `east_asia_explicit`.
    let east_asia_explicit = east_asia.filter(|s| !s.is_empty());
    let east_asia = east_asia_explicit.clone().or_else(|| latin.clone());
    let complex_script = complex_script
        .filter(|s| !s.is_empty())
        .or_else(|| latin.clone());
    let scripts: BTreeMap<String, String> = scripts
        .into_iter()
        .filter(|(k, v)| !k.is_empty() && !v.is_empty())
        .collect();
    if latin.is_none() && east_asia.is_none() && complex_script.is_none() && scripts.is_empty() {
        None
    } else {
        Some(ThemeFonts {
            latin,
            east_asia,
            east_asia_explicit,
            complex_script,
            scripts,
        })
    }
}

pub fn is_presentation_theme_path(path: &str) -> bool {
    path.starts_with("ppt/theme/") && !path.contains("themeOverride") && path.ends_with(".xml")
}

/// Resolve `a:latin` / `a:ea` theme tokens (`+mn-lt`, `+mj-ea`, …) to a concrete face name.
pub fn resolve_theme_typeface(
    typeface: &str,
    font_scheme: Option<&ThemeFontScheme>,
    _script: ThemeTypefaceScript,
) -> Option<String> {
    let t = typeface.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    if lower.starts_with('+') {
        let scheme = font_scheme?;
        return match lower.as_str() {
            "+mn-lt" => theme_font_latin(scheme, "minor"),
            "+mj-lt" => theme_font_latin(scheme, "major"),
            "+mn-ea" => theme_font_east_asia(scheme, "minor"),
            "+mj-ea" => theme_font_east_asia(scheme, "major"),
            "+mn-cs" => theme_font_complex(scheme, "minor"),
            "+mj-cs" => theme_font_complex(scheme, "major"),
            _ => None,
        };
    }
    Some(t.to_owned())
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum ThemeTypefaceScript {
    Latin,
    EastAsia,
    ComplexScript,
}

fn theme_font_latin(scheme: &ThemeFontScheme, idx: &str) -> Option<String> {
    theme_font_field(scheme, idx, |f| f.latin.as_deref())
}

fn theme_font_east_asia(scheme: &ThemeFontScheme, idx: &str) -> Option<String> {
    theme_font_field(scheme, idx, |f| {
        f.east_asia_explicit
            .as_deref()
            .or_else(|| theme_script_east_asia(f))
            .or(f.east_asia.as_deref())
            .or(f.latin.as_deref())
    })
}

/// Prefer common CJK script overrides when `a:ea` is an empty placeholder.
fn theme_script_east_asia(fonts: &ThemeFonts) -> Option<&str> {
    for key in ["Hans", "Hant", "Jpan", "Hang"] {
        if let Some(face) = fonts.scripts.get(key) {
            let face = face.trim();
            if !face.is_empty() {
                return Some(face);
            }
        }
    }
    None
}

fn theme_font_complex(scheme: &ThemeFontScheme, idx: &str) -> Option<String> {
    theme_font_field(scheme, idx, |f| {
        f.complex_script.as_deref().or(f.latin.as_deref())
    })
}

fn theme_font_field(
    scheme: &ThemeFontScheme,
    idx: &str,
    pick: fn(&ThemeFonts) -> Option<&str>,
) -> Option<String> {
    let fonts = match idx {
        "major" => scheme.major_font.as_ref(),
        "minor" => scheme.minor_font.as_ref(),
        _ => None,
    }?;
    pick(fonts)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{parse_theme_part, resolve_theme_typeface, ThemeTypefaceScript};
    use nitro_xml_opc::xml::parse_xml_str;

    // Catches: +mn-ea with empty a:ea collapsing to latin Arial instead of theme Hans script face.
    #[test]
    fn mn_ea_prefers_hans_script_when_ea_typeface_empty() {
        let doc = parse_xml_str(
            r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <a:themeElements>
                <a:clrScheme name="x"><a:dk1><a:srgbClr val="000000"/></a:dk1>
                  <a:lt1><a:srgbClr val="FFFFFF"/></a:lt1>
                  <a:dk2><a:srgbClr val="000000"/></a:dk2>
                  <a:lt2><a:srgbClr val="FFFFFF"/></a:lt2>
                  <a:accent1><a:srgbClr val="000000"/></a:accent1>
                  <a:accent2><a:srgbClr val="000000"/></a:accent2>
                  <a:accent3><a:srgbClr val="000000"/></a:accent3>
                  <a:accent4><a:srgbClr val="000000"/></a:accent4>
                  <a:accent5><a:srgbClr val="000000"/></a:accent5>
                  <a:accent6><a:srgbClr val="000000"/></a:accent6>
                  <a:hlink><a:srgbClr val="000000"/></a:hlink>
                  <a:folHlink><a:srgbClr val="000000"/></a:folHlink>
                </a:clrScheme>
                <a:fontScheme name="x">
                  <a:majorFont>
                    <a:latin typeface="Arial"/><a:ea typeface=""/><a:cs typeface=""/>
                    <a:font script="Hans" typeface="黑体"/>
                  </a:majorFont>
                  <a:minorFont>
                    <a:latin typeface="Arial"/><a:ea typeface=""/><a:cs typeface=""/>
                    <a:font script="Hans" typeface="黑体"/>
                  </a:minorFont>
                </a:fontScheme>
                <a:fmtScheme name="x"/>
              </a:themeElements>
            </a:theme>"#,
        )
        .unwrap();
        let theme = parse_theme_part(&doc.root);
        let scheme = theme.font_scheme.as_ref();
        assert_eq!(
            resolve_theme_typeface("+mn-ea", scheme, ThemeTypefaceScript::EastAsia).as_deref(),
            Some("黑体")
        );
        assert_eq!(
            resolve_theme_typeface("+mj-ea", scheme, ThemeTypefaceScript::EastAsia).as_deref(),
            Some("黑体")
        );
        // Latin token still resolves to Arial.
        assert_eq!(
            resolve_theme_typeface("+mn-lt", scheme, ThemeTypefaceScript::Latin).as_deref(),
            Some("Arial")
        );
    }
}
