// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/adapt-notes.ts; changes listed in PROVENANCE.md.

use serde_json::Value;

use crate::body::DocumentModel;
use crate::note_format::{format_note_number, number_notes_in_order};

use super::input::{LayoutNote, LayoutParagraph};

pub fn note_labels(model: &DocumentModel) -> impl Fn(&str, &str) -> String + '_ {
    let footnote_order = number_notes_in_order(&model.blocks, "footnote");
    let endnote_order = number_notes_in_order(&model.blocks, "endnote");
    move |kind: &str, id: &str| {
        let number = match kind {
            "footnote" => footnote_order.get(id).copied(),
            "endnote" => endnote_order.get(id).copied(),
            _ => None,
        };
        number
            .map(|n| {
                format_note_number(
                    n,
                    if kind == "footnote" {
                        Some(model.footnote_num_fmt.as_deref().unwrap_or("decimal"))
                    } else {
                        Some(model.endnote_num_fmt.as_deref().unwrap_or("lowerRoman"))
                    },
                )
            })
            .unwrap_or_default()
    }
}

pub fn paragraph_footnotes(
    paragraph: &Value,
    model: &DocumentModel,
    label: &dyn Fn(&str, &str) -> String,
    adapt: &mut dyn FnMut(&Value, Option<&str>) -> LayoutParagraph,
) -> Vec<LayoutNote> {
    let mut notes = Vec::new();
    if let Some(runs) = paragraph.get("runs").and_then(|v| v.as_array()) {
        for run in runs {
            let reference = run.get("noteReference");
            if reference
                .and_then(|r| r.get("kind"))
                .and_then(|v| v.as_str())
                != Some("footnote")
            {
                continue;
            }
            let id = reference.and_then(|r| r.get("id")).and_then(|v| v.as_str());
            let Some(id) = id else { continue };
            let note = model.footnotes.as_ref().and_then(|notes| {
                notes
                    .iter()
                    .find(|n| n.get("id").and_then(|v| v.as_str()) == Some(id))
            });
            if let Some(note) = note {
                notes.push(LayoutNote {
                    id: id.to_string(),
                    paragraphs: adapt_note(note, &label("footnote", id), adapt),
                });
            }
        }
    }
    notes
}

fn adapt_note(
    note: &Value,
    label: &str,
    adapt: &mut dyn FnMut(&Value, Option<&str>) -> LayoutParagraph,
) -> Vec<LayoutParagraph> {
    note.get("blocks")
        .and_then(|v| v.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|block| {
                    if block.get("type").and_then(|v| v.as_str()) == Some("paragraph") {
                        Some(adapt(block, Some(label)))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn endnote_paragraphs(
    model: &DocumentModel,
    label: &dyn Fn(&str, &str) -> String,
    adapt: &mut dyn FnMut(&Value, Option<&str>) -> LayoutParagraph,
) -> Vec<LayoutParagraph> {
    let order = number_notes_in_order(&model.blocks, "endnote");
    order
        .keys()
        .flat_map(|id| {
            model
                .endnotes
                .as_ref()
                .and_then(|notes| {
                    notes
                        .iter()
                        .find(|n| n.get("id").and_then(|v| v.as_str()) == Some(id))
                })
                .map(|note| adapt_note(note, &label("endnote", id), adapt))
        })
        .flatten()
        .collect()
}
