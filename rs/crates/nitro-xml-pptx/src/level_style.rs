// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/core/runtime/PptxHandlerRuntimePlaceholderStyles.ts
// and PptxHandlerRuntimeMasterElements.ts; changes in PROVENANCE.md.

use std::collections::BTreeMap;

use nitro_xml_opc::xml::{ns, Element};

use crate::inheritance::{attr, child_local, children_local};

pub type LevelStylesMap = BTreeMap<i8, PlaceholderTextLevelStyle>;

#[derive(Debug, Clone, Default)]
pub struct PlaceholderTextLevelStyle {
    pub bullet_char: Option<String>,
    pub bullet_auto_num_type: Option<String>,
    pub bullet_auto_num_start_at: u32,
    pub bullet_none: bool,
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
    let style = parse_level_bullet_from_ppr(p_pr);
    if style.bullet_none || style.bullet_char.is_some() || style.bullet_auto_num_type.is_some() {
        Some(style)
    } else {
        None
    }
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
        merged.insert(*k, v.clone());
    }
    merged
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
