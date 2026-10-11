// SPDX-License-Identifier: MIT
//! JSON dump shape for the #20 + #21 differential gate.

use serde_json::{json, Value};

use crate::body::read_package;
use crate::layout::{create_fake_measurer, layout_document_model};
use crate::layout::LayoutReviewOptions;
use crate::model_dump::model_to_dump_json;
use crate::numbering::{compute_list_labels, display_list_label, resolve_paragraph_numbering};
use crate::paragraph_scan::scan_paragraphs;
use crate::settings::parse_settings;
use crate::styles::resolve_paragraph_formatting;
use crate::xml_util::first_body;

pub fn dump_docx_styles(bytes: &[u8]) -> Result<String, String> {
    let mut loaded = read_package(bytes).map_err(|e| e.to_string())?;
    let model = &loaded.model;
    let settings = parse_settings(
        loaded
            .package
            .text("word/settings.xml")
            .map_err(|e| e.to_string())?,
    );
    let paragraph_styles = model
        .paragraph_styles
        .clone()
        .unwrap_or_else(|| json!({ "styles": {}, "warnings": [] }));
    let numbering_catalog = model.numbering_catalog.clone();

    let doc_xml = loaded
        .package
        .text("word/document.xml")
        .map_err(|e| e.to_string())?
        .ok_or("missing word/document.xml")?;
    let document = nitro_xml_opc::xml::parse_xml_str(doc_xml).map_err(|e| e.to_string())?;
    let body = first_body(&document.root).ok_or("DOCX document.xml has no w:body")?;

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

    let mut out = model_to_dump_json(model, settings);
    if let Some(obj) = out.as_object_mut() {
        obj.insert("resolvedParagraphs".into(), json!(resolved_paragraphs));
        obj.insert("listLabels".into(), json!(list_labels));
    }
    Ok(serde_json::to_string(&out).unwrap())
}

pub fn dump_docx_layout(bytes: &[u8]) -> Result<String, String> {
    let loaded = read_package(bytes).map_err(|e| e.to_string())?;
    let result = layout_document_model(&loaded.model, &create_fake_measurer(), LayoutReviewOptions::default());
    let out = serde_json::json!({
        "format": "docx",
        "pages": result.pages,
        "approximations": result.approximations,
    });
    Ok(serde_json::to_string(&out).unwrap())
}
