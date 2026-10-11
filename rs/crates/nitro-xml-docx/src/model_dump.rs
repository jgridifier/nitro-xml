// SPDX-License-Identifier: MIT
//! JSON dump helpers for differential parity (#20 + #21).

use serde_json::{json, Map, Value};

use crate::body::DocumentModel;

fn strip_chart_deep(value: &mut Value) {
    if let Some(blocks) = value.as_array_mut() {
        for block in blocks {
            strip_chart_deep(block);
        }
        return;
    }
    if let Some(obj) = value.as_object_mut() {
        if obj.get("type").and_then(|v| v.as_str()) == Some("paragraph") {
            if let Some(runs) = obj.get_mut("runs").and_then(|v| v.as_array_mut()) {
                for run in runs {
                    if let Some(image) = run.get_mut("image") {
                        if let Some(chart) = image.get_mut("chart").and_then(|v| v.as_object_mut())
                        {
                            chart.remove("chartSpace");
                            chart.remove("formatting");
                            chart.remove("colorPalette");
                            chart.remove("workbookPartName");
                            chart.remove("title");
                        }
                    }
                }
            }
        } else if let Some(rows) = obj.get_mut("rows").and_then(|v| v.as_array_mut()) {
            for row in rows {
                if let Some(cells) = row.as_array_mut() {
                    for cell in cells {
                        if let Some(paragraphs) = cell.get_mut("paragraphs") {
                            strip_chart_deep(paragraphs);
                        }
                    }
                }
            }
        }
    }
}

pub fn model_to_dump_json(model: &DocumentModel, settings: Value) -> Value {
    let mut blocks = model.blocks.clone();
    let mut blocks_value = json!(blocks);
    strip_chart_deep(&mut blocks_value);
    blocks = blocks_value.as_array().cloned().unwrap_or(blocks);
    let mut out = Map::new();
    out.insert("format".into(), json!(model.format));
    if let Some(v) = &model.paragraph_styles {
        out.insert("paragraphStyles".into(), v.clone());
    }
    if let Some(v) = &model.character_styles {
        out.insert("characterStyles".into(), v.clone());
    }
    out.insert(
        "numberingCatalog".into(),
        model
            .numbering_catalog
            .clone()
            .unwrap_or(json!({ "abstractNums": {}, "nums": {}, "warnings": [] })),
    );
    out.insert(
        "theme".into(),
        model.theme.clone().unwrap_or(
            json!({ "colors": {}, "colorMapping": {}, "fonts": { "major": {}, "minor": {} } }),
        ),
    );
    out.insert("settings".into(), settings);
    out.insert("page".into(), model.page.clone());
    out.insert("blocks".into(), json!(blocks));
    if let Some(s) = &model.sections {
        out.insert("sections".into(), json!(s));
    }
    if let Some(n) = &model.footnotes {
        out.insert("footnotes".into(), json!(n));
    }
    if let Some(n) = &model.endnotes {
        out.insert("endnotes".into(), json!(n));
    }
    if let Some(c) = &model.comments {
        out.insert("comments".into(), json!(c));
    }
    if let Some(v) = model.track_changes {
        out.insert("trackChanges".into(), json!(v));
    }
    if let Some(v) = model.track_formatting {
        out.insert("trackFormatting".into(), json!(v));
    }
    if let Some(v) = model.track_moves {
        out.insert("trackMoves".into(), json!(v));
    }
    if let Some(v) = &model.footnote_num_fmt {
        out.insert("footnoteNumFmt".into(), json!(v));
    }
    if let Some(v) = &model.endnote_num_fmt {
        out.insert("endnoteNumFmt".into(), json!(v));
    }
    if model.even_and_odd_headers {
        out.insert("evenAndOddHeaders".into(), json!(true));
    }
    if model.auto_hyphenation {
        out.insert("autoHyphenation".into(), json!(true));
    }
    out.insert("mediaPartNames".into(), json!(model.media_part_names));
    out.insert("warnings".into(), json!(model.warnings));
    Value::Object(out)
}
