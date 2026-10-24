// SPDX-License-Identifier: MIT
//! JSON dump shape for the #20 + #21 differential gate.

use serde_json::{json, Value};

use crate::body::read_package;
use crate::layout::LayoutReviewOptions;
use crate::layout::{
    create_fake_measurer, layout_document_model, layout_document_model_cached, MeasureCache,
};
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

    let mut result = layout_document_model(
        &loaded.model,
        &create_fake_measurer(),
        LayoutReviewOptions::default(),
    );
    crate::layout::strip_layout_ir_chart_part_names(&mut result);
    let mut approximations = result.approximations;
    approximations.sort();
    let out = serde_json::json!({
        "format": "docx",
        "pages": result.pages,
        "approximations": approximations,
    });
    Ok(serde_json::to_string(&out).unwrap())
}

/// Apply an edit script, then dump layout IR via incremental relayout (#81).
///
/// Asserts incremental ≡ full before emitting JSON (same shape as [`dump_docx_layout`]).
pub fn dump_docx_layout_after_edit(bytes: &[u8], script_json: &str) -> Result<String, String> {
    use crate::body::DocumentModel;
    use crate::edit::host::DocxHost;
    use crate::edit::replay_docx_session;
    use crate::layout::{relayout_incremental_model, DirtyHint};

    let (_report, session) = replay_docx_session(bytes, script_json).map_err(|e| e.to_string())?;
    let host = &session.host;
    let model = DocumentModel {
        format: host.format.clone(),
        blocks: host.blocks.clone(),
        page: host.page.clone().unwrap_or(json!({})),
        warnings: Vec::new(),
        paragraph_styles: host.paragraph_styles.clone(),
        character_styles: host.character_styles.clone(),
        table_styles: None,
        numbering_catalog: host.numbering_catalog.clone(),
        theme: None,
        sections: host
            .sections
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        footnotes: None,
        endnotes: None,
        footnote_num_fmt: None,
        endnote_num_fmt: None,
        even_and_odd_headers: host.even_and_odd_headers,
        auto_hyphenation: false,
        comments: None,
        track_changes: None,
        track_formatting: None,
        track_moves: None,
        media_part_names: Vec::new(),
        source_document_xml: None,
    };
    let measurer = create_fake_measurer();
    let opts = LayoutReviewOptions::default();
    let prev = {
        let base = DocxHost::open(bytes).map_err(|e| e.to_string())?;
        let base_model = DocumentModel {
            format: base.format.clone(),
            blocks: base.blocks.clone(),
            page: base.page.clone().unwrap_or(json!({})),
            warnings: Vec::new(),
            paragraph_styles: base.paragraph_styles.clone(),
            character_styles: base.character_styles.clone(),
            table_styles: None,
            numbering_catalog: base.numbering_catalog.clone(),
            theme: None,
            sections: base
                .sections
                .as_ref()
                .and_then(|v| serde_json::from_value(v.clone()).ok()),
            footnotes: None,
            endnotes: None,
            footnote_num_fmt: None,
            endnote_num_fmt: None,
            even_and_odd_headers: base.even_and_odd_headers,
            auto_hyphenation: false,
            comments: None,
            track_changes: None,
            track_formatting: None,
            track_moves: None,
            media_part_names: Vec::new(),
            source_document_xml: None,
        };
        layout_document_model(&base_model, &measurer, opts.clone())
    };
    let dirty = DirtyHint::from_host(host);
    let full = layout_document_model(&model, &measurer, opts.clone());
    let mut inc = relayout_incremental_model(&prev, &model, &measurer, dirty, opts);
    let full_json = serde_json::to_value(&full.pages).map_err(|e| e.to_string())?;
    let inc_json = serde_json::to_value(&inc.pages).map_err(|e| e.to_string())?;
    if full_json != inc_json {
        return Err("incremental layout IR diverges from full after edit".into());
    }
    crate::layout::strip_layout_ir_chart_part_names(&mut inc);
    let mut approximations = inc.approximations;
    approximations.sort();
    let _ = full;
    let out = serde_json::json!({
        "format": "docx",
        "pages": inc.pages,
        "approximations": approximations,
    });
    Ok(serde_json::to_string(&out).unwrap())
}

/// One measured layout pass. `cache_json` is a map of measure keys to
/// `{width, lineHeight?, ascent?}`. Pending probes are emitted when the cache
/// cannot yet answer every call.
pub fn dump_docx_pagecount(bytes: &[u8], cache_json: Option<&str>) -> Result<String, String> {
    let loaded = read_package(bytes).map_err(|e| e.to_string())?;
    let mut cache = MeasureCache::new();
    if let Some(text) = cache_json {
        if !text.trim().is_empty() {
            cache.load_json(text)?;
        }
    }
    let measured =
        layout_document_model_cached(&loaded.model, &cache, LayoutReviewOptions::default());
    let out = if measured.pending.is_empty() {
        json!({
            "done": true,
            "format": "docx",
            "pageCount": measured.result.pages.len(),
        })
    } else {
        json!({
            "done": false,
            "pending": measured.pending.iter().map(|probe| probe.to_json()).collect::<Vec<_>>(),
        })
    };
    Ok(serde_json::to_string(&out).unwrap())
}
