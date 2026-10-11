// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/review-display.ts; changes listed in PROVENANCE.md.

use serde_json::Value;

use crate::body::DocumentModel;
use crate::review_formatting_display::{is_run_hidden_for_review, ReviewDisplayMode};

#[derive(Clone, Default)]
pub struct LayoutReviewOptions {
    pub review_display_mode: Option<ReviewDisplayMode>,
}

pub fn review_source_length(run: &Value) -> usize {
    if run.get("equation").is_some()
        || run.get("break").is_some()
        || run.get("fieldChar").is_some()
        || run.get("fieldCode").is_some()
        || run.get("noteReference").is_some()
        || run.get("image").is_some()
    {
        1
    } else {
        run.get("text").and_then(|v| v.as_str()).map(|s| s.len()).unwrap_or(0)
    }
}

pub fn visible_review_model(model: &DocumentModel, mode: ReviewDisplayMode) -> DocumentModel {
    if mode == ReviewDisplayMode::All {
        return clone_model(model);
    }
    let mut out = clone_model(model);
    out.blocks = map_block_paragraphs(&out.blocks, mode);
    out
}

fn clone_model(model: &DocumentModel) -> DocumentModel {
    DocumentModel {
        format: model.format.clone(),
        blocks: model.blocks.clone(),
        page: model.page.clone(),
        warnings: model.warnings.clone(),
        paragraph_styles: model.paragraph_styles.clone(),
        character_styles: model.character_styles.clone(),
        table_styles: model.table_styles.clone(),
        numbering_catalog: model.numbering_catalog.clone(),
        theme: model.theme.clone(),
        sections: model.sections.clone(),
        footnotes: model.footnotes.clone(),
        endnotes: model.endnotes.clone(),
        footnote_num_fmt: model.footnote_num_fmt.clone(),
        endnote_num_fmt: model.endnote_num_fmt.clone(),
        even_and_odd_headers: model.even_and_odd_headers,
        auto_hyphenation: model.auto_hyphenation,
        comments: model.comments.clone(),
        track_changes: model.track_changes,
        track_formatting: model.track_formatting,
        track_moves: model.track_moves,
        media_part_names: model.media_part_names.clone(),
    }
}

fn map_block_paragraphs(blocks: &[Value], mode: ReviewDisplayMode) -> Vec<Value> {
    blocks
        .iter()
        .map(|block| {
            if block.get("type").and_then(|v| v.as_str()) == Some("paragraph") {
                let mut paragraph = block.clone();
                if let Some(runs) = paragraph.get_mut("runs").and_then(|v| v.as_array_mut()) {
                    *runs = runs
                        .iter()
                        .filter(|run| !is_run_hidden_for_review(run, mode))
                        .cloned()
                        .collect();
                }
                paragraph
            } else {
                block.clone()
            }
        })
        .collect()
}
