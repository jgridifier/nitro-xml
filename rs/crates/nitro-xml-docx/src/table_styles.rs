// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/table-styles.ts; changes listed in PROVENANCE.md.

use serde_json::{json, Map, Value};

use nitro_xml_opc::xml::{parse_xml_str, Document, Element};

use crate::borders::{parse_shading_fill, parse_table_borders};
use crate::run_properties::parse_run_properties;
use crate::simple_types::parse_on_off;
use crate::xml_util::{children_w, first_w, get_w, named_w, ElementExt};

const REGIONS: [&str; 9] = [
    "wholeTable",
    "firstRow",
    "lastRow",
    "firstCol",
    "lastCol",
    "band1Horz",
    "band2Horz",
    "band1Vert",
    "band2Vert",
];

fn table_style_elements(document: &Document) -> Vec<&Element> {
    let mut out = Vec::new();
    collect_table_styles(&document.root, &mut out);
    out
}

fn collect_table_styles<'a>(el: &'a Element, out: &mut Vec<&'a Element>) {
    if named_w(el, "style") && get_w(Some(el), "type").as_deref() == Some("table") {
        out.push(el);
    }
    for child in el.element_children() {
        collect_table_styles(child, out);
    }
}

fn conditional_formatting(block: &Element) -> Value {
    let mut result = Map::new();
    let tc_pr = first_w(Some(block), "tcPr");
    if let Some(borders) = parse_table_borders(first_w(tc_pr, "tcBorders")) {
        result.insert("borders".into(), borders);
    }
    if let Some(fill) = parse_shading_fill(first_w(tc_pr, "shd")) {
        result.insert("shadingFill".into(), json!(fill));
    }
    let run = parse_run_properties(first_w(Some(block), "rPr"));
    if run.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
        result.insert("run".into(), run);
    }
    Value::Object(result)
}

pub fn parse_table_style_catalog(xml: &str) -> Value {
    let document = parse_xml_str(xml).expect("styles xml");
    let mut styles = Map::new();
    for element in table_style_elements(&document) {
        let id = get_w(Some(element), "styleId");
        if id.is_none() {
            continue;
        }
        let id = id.unwrap();
        let mut style = Map::new();
        style.insert("id".into(), json!(id));
        if let Some(name) = get_w(first_w(Some(element), "name"), "val") {
            style.insert("name".into(), json!(name));
        }
        if let Some(based_on) = get_w(first_w(Some(element), "basedOn"), "val") {
            style.insert("basedOn".into(), json!(based_on));
        }
        if let Some(is_default) = parse_on_off(get_w(Some(element), "default").as_deref()) {
            style.insert("isDefault".into(), json!(is_default));
        }
        let tbl_pr = first_w(Some(element), "tblPr");
        if let Some(borders) = parse_table_borders(first_w(tbl_pr, "tblBorders")) {
            style.insert("borders".into(), borders);
        }
        if let Some(fill) = parse_shading_fill(first_w(tbl_pr, "shd")) {
            style.insert("shadingFill".into(), json!(fill));
        }
        let mut conditional = Map::new();
        for block in children_w(element, "tblStylePr") {
            let region = get_w(Some(block), "type");
            if let Some(region) = region {
                if REGIONS.contains(&region.as_str()) {
                    conditional.insert(region, conditional_formatting(block));
                }
            }
        }
        if !conditional.is_empty() {
            style.insert("conditional".into(), Value::Object(conditional));
        }
        styles.insert(id, Value::Object(style));
    }
    let warnings = table_style_warnings(&styles);
    json!({
        "styles": styles,
        "warnings": warnings
    })
}

fn table_style_warnings(styles: &Map<String, Value>) -> Vec<String> {
    let mut warnings = Vec::new();
    for style in styles.values() {
        let id = style.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let mut seen = std::collections::HashSet::from([id.to_owned()]);
        let mut parent = style.get("basedOn").and_then(|v| v.as_str());
        while let Some(p) = parent {
            if seen.contains(p) {
                warnings.push(format!(
                    "Table style inheritance cycle detected at \"{p}\"; cyclic inheritance is ignored."
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
    let mut seen = std::collections::HashSet::new();
    warnings
        .into_iter()
        .filter(|w| seen.insert(w.clone()))
        .collect()
}
