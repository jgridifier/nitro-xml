// SPDX-License-Identifier: MIT
//! JSON dump shape for the #20 differential gate.

use serde_json::{json, Value};

use nitro_xml_opc::opc::{AllowedFormat, Package};
use nitro_xml_opc::xml::parse_xml_str;

use crate::numbering::{compute_list_labels, display_list_label, resolve_paragraph_numbering};
use crate::paragraph_scan::scan_paragraphs;
use crate::settings::parse_settings;
use crate::styles::{
    default_style_catalogs, parse_paragraph_style_catalog, parse_run_style_catalog,
    resolve_paragraph_formatting,
};
use crate::theme::{parse_color_scheme_mapping, parse_theme};
use crate::xml_util::first_body;

pub fn dump_docx_styles(bytes: &[u8]) -> Result<String, String> {
    let mut pkg = Package::open(bytes).map_err(|e| e.to_string())?;
    let format = match pkg.format() {
        AllowedFormat::Docx => "docx",
        AllowedFormat::Docm => "docm",
        AllowedFormat::Dotx => "dotx",
        AllowedFormat::Dotm => "dotm",
        other => return Err(format!("not a word package: {}", other.as_str())),
    };
    let doc_xml = pkg
        .text("word/document.xml")
        .map_err(|e| e.to_string())?
        .ok_or("missing word/document.xml")?;
    let document = parse_xml_str(doc_xml).map_err(|e| e.to_string())?;
    let body = first_body(&document.root);
    if body.is_none() {
        return Err("DOCX document.xml has no w:body".into());
    }
    let body = body.unwrap();

    let (default_p, default_c) = default_style_catalogs();
    let paragraph_styles =
        if let Some(xml) = pkg.text("word/styles.xml").map_err(|e| e.to_string())? {
            parse_paragraph_style_catalog(xml)
        } else {
            default_p
        };
    let character_styles =
        if let Some(xml) = pkg.text("word/styles.xml").map_err(|e| e.to_string())? {
            parse_run_style_catalog(xml)
        } else {
            default_c
        };
    let numbering_catalog = pkg
        .text("word/numbering.xml")
        .map_err(|e| e.to_string())?
        .map(crate::numbering::parse_numbering_catalog);
    let mut theme = if let Some(xml) = pkg
        .text("word/theme/theme1.xml")
        .map_err(|e| e.to_string())?
    {
        parse_theme(xml)
    } else {
        json!({ "colors": {}, "colorMapping": {}, "fonts": { "major": {}, "minor": {} } })
    };
    if theme.get("colorMapping").is_some() {
        if let Some(settings_xml) = pkg.text("word/settings.xml").map_err(|e| e.to_string())? {
            if let Some(obj) = theme.as_object_mut() {
                obj.insert(
                    "colorMapping".into(),
                    parse_color_scheme_mapping(settings_xml),
                );
            }
        }
    }
    let settings = parse_settings(pkg.text("word/settings.xml").map_err(|e| e.to_string())?);

    let paragraphs = scan_paragraphs(Some(body));
    let resolved_paragraphs: Vec<Value> = paragraphs
        .iter()
        .map(|p| {
            let numbering =
                resolve_paragraph_numbering(p, Some(&paragraph_styles), numbering_catalog.as_ref());
            json!({
                "id": p.id,
                "resolved": resolve_paragraph_formatting(p, &paragraph_styles),
                "numbering": match numbering {
                    Some((num_id, level)) => json!({ "numId": num_id, "level": level }),
                    None => Value::Null,
                }
            })
        })
        .collect();
    let labels_map = compute_list_labels(
        &paragraphs,
        Some(&paragraph_styles),
        numbering_catalog.as_ref(),
    );
    let list_labels: Vec<Value> = paragraphs
        .iter()
        .filter_map(|p| {
            labels_map.get(&p.id).map(|label| {
                let text = label
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(display_list_label)
                    .unwrap_or_default();
                json!({
                    "paragraphId": p.id,
                    "numId": label.get("numId").cloned().unwrap_or(json!("")),
                    "level": label.get("level").cloned().unwrap_or(json!(0)),
                    "text": text,
                    "suffix": label.get("suffix").cloned().unwrap_or(json!("tab")),
                })
            })
        })
        .collect();

    let out = json!({
        "format": format,
        "paragraphStyles": paragraph_styles,
        "characterStyles": character_styles,
        "numberingCatalog": numbering_catalog.unwrap_or(json!({ "abstractNums": {}, "nums": {}, "warnings": [] })),
        "theme": theme,
        "settings": settings,
        "resolvedParagraphs": resolved_paragraphs,
        "listLabels": list_labels,
    });
    Ok(serde_json::to_string(&out).unwrap())
}
