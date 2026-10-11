// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/paragraph-styles.ts and
// character-styles.ts; changes listed in PROVENANCE.md.

use serde_json::{json, Map, Value};

use nitro_xml_opc::xml::{parse_xml_str, Document, Element};

use crate::borders::{parse_paragraph_borders, parse_shading_fill};
use crate::json_util::{insert_bool, insert_i64, insert_str, omit_undefined_object};
use crate::paragraph_alignment::{align_from_justification, parse_justification};
use crate::run_properties::parse_run_properties;
use crate::simple_types::{
    on_off_element, parse_integer, parse_on_off, parse_signed_twips, parse_twips,
};
use crate::xml_util::{first_w, get_w, named_w, ElementExt};

pub const PAGINATION_KEYS: [&str; 5] = [
    "keepNext",
    "keepLines",
    "widowControl",
    "contextualSpacing",
    "suppressLineNumbers",
];

pub fn parse_outline_level(element: Option<&Element>) -> Option<i64> {
    let raw = get_w(element, "val");
    let value = parse_integer(raw.as_deref())?;
    if !(0..=9).contains(&value) {
        return None;
    }
    Some(if value == 9 { 0 } else { value + 1 })
}

pub fn parse_formatting(p_pr: Option<&Element>) -> Value {
    let mut result = Map::new();
    let parsed = parse_justification(p_pr);
    insert_str(&mut result, "justification", parsed.justification);
    if let Some(a) = parsed.align {
        result.insert("align".into(), json!(a));
    }
    let bidi = on_off_element(first_w(p_pr, "bidi"));
    if let Some(b) = bidi {
        result.insert("direction".into(), json!(if b { "rtl" } else { "ltr" }));
    }
    insert_bool(
        &mut result,
        "pageBreakBefore",
        on_off_element(first_w(p_pr, "pageBreakBefore")),
    );
    for key in PAGINATION_KEYS {
        insert_bool(&mut result, key, on_off_element(first_w(p_pr, key)));
    }
    if let Some(o) = parse_outline_level(first_w(p_pr, "outlineLvl")) {
        insert_i64(&mut result, "outlineLevel", Some(o));
    }
    if let Some(b) = parse_paragraph_borders(first_w(p_pr, "pBdr")) {
        result.insert("borders".into(), b);
    }
    if let Some(s) = parse_shading_fill(first_w(p_pr, "shd")) {
        result.insert("shadingFill".into(), json!(s));
    }
    let spacing = first_w(p_pr, "spacing");
    insert_i64(
        &mut result,
        "spacingBeforeTwips",
        parse_twips(get_w(spacing, "before").as_deref()).map(|n| n as i64),
    );
    insert_i64(
        &mut result,
        "spacingAfterTwips",
        parse_twips(get_w(spacing, "after").as_deref()).map(|n| n as i64),
    );
    insert_i64(
        &mut result,
        "lineSpacingTwips",
        parse_signed_twips(get_w(spacing, "line").as_deref()),
    );
    let rule = get_w(spacing, "lineRule");
    if matches!(
        rule.as_deref(),
        Some("auto") | Some("exact") | Some("atLeast")
    ) {
        insert_str(&mut result, "lineSpacingRule", rule);
    } else if result.contains_key("lineSpacingTwips") {
        result.insert("lineSpacingRule".into(), json!("auto"));
    }
    let ind = first_w(p_pr, "ind");
    for (xml, key) in [
        ("left", "indentLeftTwips"),
        ("right", "indentRightTwips"),
        ("start", "indentStartTwips"),
        ("end", "indentEndTwips"),
    ] {
        insert_i64(
            &mut result,
            key,
            parse_signed_twips(get_w(ind, xml).as_deref()),
        );
    }
    for (xml, key) in [("firstLine", "firstLineTwips"), ("hanging", "hangingTwips")] {
        insert_i64(
            &mut result,
            key,
            parse_twips(get_w(ind, xml).as_deref()).map(|n| n as i64),
        );
    }
    omit_undefined_object(result)
}

fn parse_style_numbering(p_pr: Option<&Element>) -> Option<Value> {
    let num_pr = first_w(p_pr, "numPr")?;
    let num_id = parse_integer(get_w(first_w(Some(num_pr), "numId"), "val").as_deref())?;
    let level = parse_integer(get_w(first_w(Some(num_pr), "ilvl"), "val").as_deref()).unwrap_or(0);
    Some(json!({ "numId": num_id, "level": level }))
}

fn paragraph_style_elements(document: &Document) -> Vec<&Element> {
    let mut out = Vec::new();
    collect_styles(&document.root, &mut out, "paragraph");
    out
}

fn collect_styles<'a>(el: &'a Element, out: &mut Vec<&'a Element>, ty: &str) {
    if named_w(el, "style") && get_w(Some(el), "type").as_deref() == Some(ty) {
        out.push(el);
    }
    for child in el.element_children() {
        collect_styles(child, out, ty);
    }
}

pub fn parse_paragraph_style_catalog(xml: &str) -> Value {
    let document = parse_xml_str(xml).expect("styles xml");
    let defaults = first_w(first_w(Some(&document.root), "docDefaults"), "pPrDefault");
    let p_defaults = first_w(defaults, "pPr");
    let mut styles = Map::new();
    for element in paragraph_style_elements(&document) {
        let id = get_w(Some(element), "styleId");
        if id.is_none() {
            continue;
        }
        let id = id.unwrap();
        let mut style = Map::new();
        style.insert("id".into(), json!(id));
        insert_str(
            &mut style,
            "name",
            get_w(first_w(Some(element), "name"), "val"),
        );
        insert_str(
            &mut style,
            "basedOn",
            get_w(first_w(Some(element), "basedOn"), "val"),
        );
        let is_default = parse_on_off(get_w(Some(element), "default").as_deref());
        insert_bool(&mut style, "isDefault", is_default);
        if let Some(n) = parse_style_numbering(first_w(Some(element), "pPr")) {
            style.insert("numbering".into(), n);
        }
        style.insert(
            "formatting".into(),
            parse_formatting(first_w(Some(element), "pPr")),
        );
        styles.insert(id, Value::Object(style));
    }
    let warnings = paragraph_style_warnings(&styles);
    json!({
        "docDefaults": parse_formatting(p_defaults),
        "styles": styles,
        "warnings": warnings
    })
}

fn paragraph_style_warnings(styles: &Map<String, Value>) -> Vec<String> {
    let mut warnings = Vec::new();
    for style in styles.values() {
        let id = style.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let mut seen = std::collections::HashSet::from([id.to_owned()]);
        let mut parent = style.get("basedOn").and_then(|v| v.as_str());
        while let Some(p) = parent {
            if seen.contains(p) {
                warnings.push(format!(
                    "Paragraph style inheritance cycle detected at “{p}”; cyclic inheritance is ignored."
                ));
                break;
            }
            seen.insert(p.to_owned());
            parent = styles
                .get(p)
                .and_then(|s| s.get("basedOn"))
                .and_then(|v| v.as_str());
        }
    }
    dedupe(warnings)
}

pub fn parse_run_style_catalog(xml: &str) -> Value {
    let document = parse_xml_str(xml).expect("styles xml");
    let defaults = first_w(first_w(Some(&document.root), "docDefaults"), "rPrDefault");
    let mut styles = Map::new();
    let mut out = Vec::new();
    collect_character_styles(&document.root, &mut out);
    for element in out {
        let id = get_w(Some(element), "styleId");
        if id.is_none() {
            continue;
        }
        let id = id.unwrap();
        let ty = if get_w(Some(element), "type").as_deref() == Some("character") {
            "character"
        } else {
            "paragraph"
        };
        let mut style = Map::new();
        style.insert("id".into(), json!(id));
        style.insert("type".into(), json!(ty));
        insert_str(
            &mut style,
            "name",
            get_w(first_w(Some(element), "name"), "val"),
        );
        insert_str(
            &mut style,
            "basedOn",
            get_w(first_w(Some(element), "basedOn"), "val"),
        );
        insert_str(
            &mut style,
            "linkedStyle",
            get_w(first_w(Some(element), "link"), "val"),
        );
        insert_bool(
            &mut style,
            "isDefault",
            parse_on_off(get_w(Some(element), "default").as_deref()),
        );
        style.insert(
            "formatting".into(),
            parse_run_properties(first_w(Some(element), "rPr")),
        );
        styles.insert(id, Value::Object(style));
    }
    let warnings = character_style_warnings(&styles);
    json!({
        "docDefaults": parse_run_properties(first_w(defaults, "rPr")),
        "styles": styles,
        "warnings": warnings
    })
}

fn collect_character_styles<'a>(el: &'a Element, out: &mut Vec<&'a Element>) {
    if named_w(el, "style") {
        if let Some(t) = get_w(Some(el), "type") {
            if t == "character" || t == "paragraph" {
                out.push(el);
            }
        }
    }
    for child in el.element_children() {
        collect_character_styles(child, out);
    }
}

fn character_style_warnings(styles: &Map<String, Value>) -> Vec<String> {
    let mut warnings = Vec::new();
    for style in styles.values() {
        let id = style.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let mut seen = std::collections::HashSet::from([id.to_owned()]);
        let mut parent = style.get("basedOn").and_then(|v| v.as_str());
        while let Some(p) = parent {
            if seen.contains(p) {
                warnings.push(format!(
                    "Character style inheritance cycle detected at \"{p}\"; cyclic inheritance is ignored."
                ));
                break;
            }
            seen.insert(p.to_owned());
            parent = styles
                .get(p)
                .and_then(|s| s.get("basedOn"))
                .and_then(|v| v.as_str());
        }
    }
    dedupe(warnings)
}

fn dedupe(mut warnings: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    warnings.retain(|w| seen.insert(w.clone()));
    warnings
}

pub struct ParagraphProps {
    pub id: String,
    pub style: Option<String>,
    pub numbering: Option<(i64, i64)>,
    pub direct: Value,
}

pub fn resolve_paragraph_formatting(paragraph: &ParagraphProps, catalog: &Value) -> Value {
    let doc_defaults = catalog.get("docDefaults").cloned().unwrap_or(json!({}));
    let mut result = object_to_map(doc_defaults);
    let styles = catalog.get("styles").and_then(|s| s.as_object());
    let explicit_style = paragraph.style.as_deref();
    let default_style = styles.and_then(|m| {
        m.values()
            .find(|s| s.get("isDefault") == Some(&json!(true)))
            .and_then(|s| s.get("id"))
            .and_then(|v| v.as_str())
    });
    apply_style(explicit_style.or(default_style), styles, &mut result);
    let direct = object_to_map(paragraph.direct.clone());
    for key in direct.keys() {
        if key == "style" || key == "numbering" {
            continue;
        }
        if let Some(v) = direct.get(key) {
            result.insert(key.clone(), v.clone());
        }
    }
    if paragraph.direct.get("align").is_some() && paragraph.direct.get("justification").is_none() {
        result.remove("justification");
    }
    let just = result.get("justification").and_then(|v| v.as_str());
    let dir_rtl = result.get("direction").and_then(|v| v.as_str()) == Some("rtl");
    if matches!(just, Some("start") | Some("end")) {
        let p_align = paragraph.direct.get("align");
        let p_just = paragraph
            .direct
            .get("justification")
            .and_then(|v| v.as_str());
        let p_dir_rtl = paragraph.direct.get("direction").and_then(|v| v.as_str()) == Some("rtl");
        let implied = p_just.and_then(|j| align_from_justification(j, p_dir_rtl));
        let needs = p_align.is_none()
            || (p_just.is_some() && p_align.cloned() == implied.map(|a| json!(a)));
        if needs {
            if let Some(a) = align_from_justification(just.unwrap(), dir_rtl) {
                result.insert("align".into(), json!(a));
            } else {
                result.remove("align");
            }
        }
    }
    Value::Object(result)
}

fn apply_style(
    style_id: Option<&str>,
    styles: Option<&Map<String, Value>>,
    out: &mut Map<String, Value>,
) {
    let Some(styles) = styles else {
        return;
    };
    let mut chain = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut current = style_id;
    while let Some(id) = current {
        if visited.contains(id) {
            break;
        }
        visited.insert(id);
        let Some(style) = styles.get(id) else {
            break;
        };
        chain.push(style);
        current = style.get("basedOn").and_then(|v| v.as_str());
    }
    for style in chain.iter().rev() {
        if let Some(fmt) = style.get("formatting") {
            merge_objects(out, object_to_map(fmt.clone()));
        }
    }
}

fn merge_objects(target: &mut Map<String, Value>, source: Map<String, Value>) {
    for (k, v) in source {
        target.insert(k, v);
    }
}

fn object_to_map(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

pub fn resolve_style_numbering(style_id: Option<&str>, catalog: &Value) -> Option<(i64, i64)> {
    let styles = catalog.get("styles").and_then(|s| s.as_object())?;
    let mut chain = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut current = style_id;
    while let Some(id) = current {
        if visited.contains(id) {
            break;
        }
        visited.insert(id);
        let style = styles.get(id)?;
        chain.push(style);
        current = style.get("basedOn").and_then(|v| v.as_str());
    }
    for style in chain {
        if let Some(n) = style.get("numbering") {
            let num_id = n.get("numId").and_then(|v| v.as_i64())?;
            let level = n.get("level").and_then(|v| v.as_i64()).unwrap_or(0);
            return Some((num_id, level));
        }
    }
    None
}

pub fn default_style_catalogs() -> (Value, Value) {
    let xml = crate::default_styles::default_styles_xml();
    let paragraph = parse_paragraph_style_catalog(&xml);
    let character = parse_run_style_catalog(&xml);
    (paragraph, character)
}
