// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/core/runtime/PptxHandlerRuntimePlaceholderStyles.ts
// and PptxHandlerRuntimeMasterElements.ts; changes in PROVENANCE.md.

use std::collections::BTreeMap;

use nitro_xml_opc::drawingml::{
    emu_to_pixels, parse_drawing_color_in, resolve_drawing_color, DrawingColor,
};
use nitro_xml_opc::xml::{ns, Element};

use crate::inheritance::{attr, child_local, children_local};

pub type LevelStylesMap = BTreeMap<i8, PlaceholderTextLevelStyle>;

#[derive(Debug, Clone, Default)]
pub struct PlaceholderTextLevelStyle {
    pub bullet_char: Option<String>,
    pub bullet_auto_num_type: Option<String>,
    pub bullet_auto_num_start_at: u32,
    pub bullet_none: bool,
    /// Resolved `#RRGGBB` when `a:defRPr/a:solidFill` does not need a theme.
    pub text_color_hex: Option<String>,
    /// Scheme (or other) colour kept for resolve against the slide colour map.
    pub text_color: Option<DrawingColor>,
    /// `a:defRPr/@sz` in CSS px (hundredths of a point, same scale as `sz_to_px`).
    pub font_size_px: Option<f64>,
    /// `a:lvlNpPr/@eaLnBrk` or `a:defPPr/@eaLnBrk` from presentation `defaultTextStyle`.
    pub ea_line_break: Option<bool>,
    /// `a:defRPr/a:latin/@typeface` (may be a theme token such as `+mn-lt`).
    pub default_latin_typeface: Option<String>,
    /// `a:defRPr/@b` when authored (`1` / `0`).
    pub font_bold: Option<bool>,
    /// `a:defRPr/@i` when authored.
    pub font_italic: Option<bool>,
    /// `a:pPr/@marL` in CSS px.
    pub margin_left_px: Option<f64>,
    /// `a:pPr/@indent` in CSS px (may be negative for hanging indents).
    pub indent_px: Option<f64>,
}

pub fn parse_level_bullet_from_ppr(p_pr: &Element) -> PlaceholderTextLevelStyle {
    let mut style = PlaceholderTextLevelStyle::default();
    if child_local(p_pr, "buNone", ns::A).is_some() {
        style.bullet_none = true;
        return style;
    }
    if let Some(bu_char) = child_local(p_pr, "buChar", ns::A) {
        let ch = attr(bu_char, "char").unwrap_or("•");
        style.bullet_char = Some(ch.to_owned());
    }
    if let Some(bu_auto) = child_local(p_pr, "buAutoNum", ns::A) {
        style.bullet_auto_num_type = attr(bu_auto, "type").map(str::to_owned);
        let start = attr(bu_auto, "startAt")
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|v| *v > 0);
        style.bullet_auto_num_start_at = start.unwrap_or(1);
    }
    style
}

pub fn parse_level_style_from_ppr(p_pr: &Element) -> Option<PlaceholderTextLevelStyle> {
    let mut style = parse_level_bullet_from_ppr(p_pr);
    if let Some(v) = attr(p_pr, "eaLnBrk") {
        style.ea_line_break = Some(matches!(v, "1" | "true" | "on"));
    }
    apply_paragraph_margin_attrs(&mut style, p_pr);
    apply_def_rpr(&mut style, p_pr);
    if level_style_has_payload(&style) {
        Some(style)
    } else {
        None
    }
}

fn level_style_has_payload(style: &PlaceholderTextLevelStyle) -> bool {
    style.bullet_none
        || style.bullet_char.is_some()
        || style.bullet_auto_num_type.is_some()
        || style.text_color_hex.is_some()
        || style.text_color.is_some()
        || style.font_size_px.is_some()
        || style.ea_line_break.is_some()
        || style.default_latin_typeface.is_some()
        || style.font_bold.is_some()
        || style.font_italic.is_some()
        || style.margin_left_px.is_some()
        || style.indent_px.is_some()
}

fn apply_paragraph_margin_attrs(style: &mut PlaceholderTextLevelStyle, p_pr: &Element) {
    style.margin_left_px = emu_attr_to_px(p_pr, "marL");
    style.indent_px = emu_attr_to_px(p_pr, "indent");
}

fn emu_attr_to_px(el: &Element, local: &str) -> Option<f64> {
    attr(el, local)
        .and_then(|s| s.parse::<i64>().ok())
        .map(emu_to_pixels)
}

/// `a:defRPr` solid fill and `sz` on `a:lvlNpPr` / `a:defPPr`.
fn apply_def_rpr(style: &mut PlaceholderTextLevelStyle, p_pr: &Element) {
    let Some(def) = child_local(p_pr, "defRPr", ns::A) else {
        return;
    };
    if let Some(latin) = child_local(def, "latin", ns::A) {
        if let Some(tf) = attr(latin, "typeface") {
            let trimmed = tf.trim();
            if !trimmed.is_empty() {
                style.default_latin_typeface = Some(trimmed.to_owned());
            }
        }
    }
    style.font_size_px = sz_attr_to_px(def);
    if let Some(b) = attr(def, "b") {
        style.font_bold = Some(matches!(b, "1" | "true" | "on"));
    }
    if let Some(i) = attr(def, "i") {
        style.font_italic = Some(matches!(i, "1" | "true" | "on"));
    }
    let Some(fill) = child_local(def, "solidFill", ns::A) else {
        return;
    };
    let Some(color) = parse_drawing_color_in(Some(fill)) else {
        return;
    };
    if let Some(resolved) = resolve_drawing_color(&color, None, None) {
        if resolved.alpha >= 0.999 {
            style.text_color_hex = Some(resolved.hex);
            return;
        }
    }
    style.text_color = Some(color);
}

fn sz_attr_to_px(rpr: &Element) -> Option<f64> {
    let sz = attr(rpr, "sz")?.parse::<f64>().ok()?;
    if sz <= 0.0 {
        return None;
    }
    Some(sz / 100.0 * 96.0 / 72.0)
}

pub fn parse_lst_style_levels(lst_style: &Element) -> LevelStylesMap {
    let mut map = LevelStylesMap::new();
    for level in 1..=9 {
        let local = format!("lvl{level}pPr");
        if let Some(node) = child_local(lst_style, &local, ns::A) {
            if let Some(parsed) = parse_level_style_from_ppr(node) {
                map.insert((level - 1) as i8, parsed);
            }
        }
    }
    if let Some(def) = child_local(lst_style, "defPPr", ns::A) {
        if let Some(parsed) = parse_level_style_from_ppr(def) {
            map.insert(-1, parsed);
        }
    }
    map
}

pub fn parse_tx_style_category(style_root: &Element) -> LevelStylesMap {
    let mut map = LevelStylesMap::new();
    for level in 1..=9 {
        let local = format!("lvl{level}pPr");
        if let Some(node) = child_local(style_root, &local, ns::A) {
            if let Some(parsed) = parse_level_style_from_ppr(node) {
                map.insert((level - 1) as i8, parsed);
            }
        }
    }
    if let Some(def) = child_local(style_root, "defPPr", ns::A) {
        if let Some(parsed) = parse_level_style_from_ppr(def) {
            map.insert(-1, parsed);
        }
    }
    map
}

pub fn merge_level_styles(base: &LevelStylesMap, override_: &LevelStylesMap) -> LevelStylesMap {
    let mut merged = base.clone();
    for (k, v) in override_ {
        let next = merged
            .get(k)
            .map(|prev| merge_level_style(prev, v))
            .unwrap_or_else(|| v.clone());
        merged.insert(*k, next);
    }
    merged
}

/// Field merge so a layout level that only sets `sz` keeps the master's colour.
fn merge_level_style(
    base: &PlaceholderTextLevelStyle,
    over: &PlaceholderTextLevelStyle,
) -> PlaceholderTextLevelStyle {
    let over_has_bullet =
        over.bullet_none || over.bullet_char.is_some() || over.bullet_auto_num_type.is_some();
    let over_has_color = over.text_color_hex.is_some() || over.text_color.is_some();
    PlaceholderTextLevelStyle {
        bullet_char: if over_has_bullet {
            over.bullet_char.clone()
        } else {
            base.bullet_char.clone()
        },
        bullet_auto_num_type: if over_has_bullet {
            over.bullet_auto_num_type.clone()
        } else {
            base.bullet_auto_num_type.clone()
        },
        bullet_auto_num_start_at: if over_has_bullet {
            over.bullet_auto_num_start_at
        } else {
            base.bullet_auto_num_start_at
        },
        bullet_none: if over_has_bullet {
            over.bullet_none
        } else {
            base.bullet_none
        },
        text_color_hex: if over_has_color {
            over.text_color_hex.clone()
        } else {
            base.text_color_hex.clone()
        },
        text_color: if over_has_color {
            over.text_color.clone()
        } else {
            base.text_color.clone()
        },
        font_size_px: over.font_size_px.or(base.font_size_px),
        ea_line_break: over.ea_line_break.or(base.ea_line_break),
        default_latin_typeface: over
            .default_latin_typeface
            .clone()
            .or_else(|| base.default_latin_typeface.clone()),
        font_bold: over.font_bold.or(base.font_bold),
        font_italic: over.font_italic.or(base.font_italic),
        margin_left_px: over.margin_left_px.or(base.margin_left_px),
        indent_px: over.indent_px.or(base.indent_px),
    }
}

/// True when the deck's `p:defaultTextStyle` enables East Asian line breaks (`eaLnBrk="1"`).
pub fn presentation_default_ea_line_break(styles: Option<&LevelStylesMap>) -> bool {
    let Some(styles) = styles else {
        return false;
    };
    for key in [0i8, -1i8] {
        if styles
            .get(&key)
            .and_then(|s| s.ea_line_break)
            .is_some_and(|v| v)
        {
            return true;
        }
    }
    false
}

pub fn level_style_for(map: &LevelStylesMap, level: i8) -> Option<&PlaceholderTextLevelStyle> {
    map.get(&level).or_else(|| map.get(&-1))
}

/// Walk `p:txStyles` on a slide master.
pub fn parse_master_tx_styles(sld_master: &Element) -> MasterTxStyles {
    let tx_styles = child_local(sld_master, "txStyles", ns::P);
    let mut out = MasterTxStyles::default();
    if let Some(tx) = tx_styles {
        if let Some(title) = child_local(tx, "titleStyle", ns::P) {
            out.title_style = parse_tx_style_category(title);
        }
        if let Some(body) = child_local(tx, "bodyStyle", ns::P) {
            out.body_style = parse_tx_style_category(body);
        }
        if let Some(other) = child_local(tx, "otherStyle", ns::P) {
            out.other_style = parse_tx_style_category(other);
        }
    }
    out
}

#[derive(Debug, Clone, Default)]
pub struct MasterTxStyles {
    pub title_style: LevelStylesMap,
    pub body_style: LevelStylesMap,
    pub other_style: LevelStylesMap,
}

pub fn master_text_levels_for_placeholder<'a>(
    tx: &'a MasterTxStyles,
    placeholder_type: Option<&str>,
) -> &'a LevelStylesMap {
    use crate::inheritance::placeholder_style_family;
    let family = placeholder_style_family(placeholder_type);
    match family.as_str() {
        "title" => &tx.title_style,
        "body" => &tx.body_style,
        _ => &tx.other_style,
    }
}

pub fn extract_lst_style_from_tx_body(tx_body: &Element) -> LevelStylesMap {
    child_local(tx_body, "lstStyle", ns::A)
        .map(parse_lst_style_levels)
        .unwrap_or_default()
}

pub fn scan_sp_tree_placeholder_level_styles(
    sp_tree: &Element,
) -> BTreeMap<String, LevelStylesMap> {
    let mut out = BTreeMap::new();
    collect_ph_defaults(sp_tree, &mut out);
    out
}

fn collect_ph_defaults(sp_tree: &Element, out: &mut BTreeMap<String, LevelStylesMap>) {
    for sp in children_local(sp_tree, "sp", ns::P) {
        let nv_pr = child_local(sp, "nvSpPr", ns::P).and_then(|n| child_local(n, "nvPr", ns::P));
        let ph = nv_pr.and_then(|n| child_local(n, "ph", ns::P));
        if ph.is_none() {
            continue;
        }
        let ph_info = crate::inheritance::extract_placeholder_info(nv_pr);
        let key = ph_info
            .as_ref()
            .map(crate::placeholder_defaults::placeholder_defaults_key)
            .unwrap_or_default();
        if key.is_empty() {
            continue;
        }
        let tx_body = child_local(sp, "txBody", ns::P);
        if let Some(tx) = tx_body {
            let levels = extract_lst_style_from_tx_body(tx);
            if !levels.is_empty() {
                out.insert(key, levels);
            }
        }
    }
    for grp in children_local(sp_tree, "grpSp", ns::P) {
        collect_ph_defaults(grp, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nitro_xml_opc::xml::parse_xml_str;

    // Catches: layout `lvl1pPr` that only overrides `sz` dropping the master's `defRPr` colour.
    #[test]
    fn merge_level_style_keeps_master_color_when_override_only_sets_size() {
        let master = parse_level_style_from_ppr(
            &parse_xml_str(
                r#"<a:lvl1pPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
                  <a:defRPr sz="4400"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:defRPr>
                </a:lvl1pPr>"#,
            )
            .unwrap()
            .root,
        )
        .unwrap();
        let layout = parse_level_style_from_ppr(
            &parse_xml_str(
                r#"<a:lvl1pPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
                  <a:defRPr sz="6000"/>
                </a:lvl1pPr>"#,
            )
            .unwrap()
            .root,
        )
        .unwrap();
        let merged = merge_level_style(&master, &layout);
        assert_eq!(merged.text_color_hex.as_deref(), Some("#FFFFFF"));
        assert!((merged.font_size_px.unwrap() - 80.0).abs() < 0.05);
    }

    // Catches: master `marL`/`indent`/`buChar` lost when layout level only tweaks alignment.
    #[test]
    fn merge_level_style_keeps_master_bullet_and_margins() {
        let master = parse_level_style_from_ppr(
            &parse_xml_str(
                r#"<a:lvl2pPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
 marL="914400" indent="-342900">
                  <a:buChar char="•"/>
                </a:lvl2pPr>"#,
            )
            .unwrap()
            .root,
        )
        .unwrap();
        let layout = parse_level_style_from_ppr(
            &parse_xml_str(
                r#"<a:lvl2pPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" algn="l"/>"#,
            )
            .unwrap()
            .root,
        );
        assert!(layout.is_none());
        let merged = merge_level_style(&master, &PlaceholderTextLevelStyle::default());
        assert_eq!(merged.bullet_char.as_deref(), Some("•"));
        assert!((merged.margin_left_px.unwrap() - 96.0).abs() < 0.05);
        assert!((merged.indent_px.unwrap() + 36.0).abs() < 0.05);
    }
}
