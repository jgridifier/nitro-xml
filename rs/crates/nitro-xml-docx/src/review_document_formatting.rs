// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/review-document-formatting.ts; changes listed in PROVENANCE.md.

use crate::body::DocumentModel;
use crate::review_formatting_display::ReviewDisplayMode;

pub struct ReviewFormattingDiagnostic {
    pub paragraph_id: String,
    pub run_index: Option<usize>,
    pub message: String,
}

pub struct ReviewDocumentFormattingResult {
    pub model: DocumentModel,
    pub diagnostics: Vec<ReviewFormattingDiagnostic>,
}

pub fn review_document_formatting(model: &DocumentModel, mode: &str) -> ReviewDocumentFormattingResult {
    let mode = ReviewDisplayMode::parse(mode);
    if mode != ReviewDisplayMode::Original {
        return ReviewDocumentFormattingResult {
            model: clone_model(model),
            diagnostics: Vec::new(),
        };
    }
    ReviewDocumentFormattingResult {
        model: clone_model(model),
        diagnostics: Vec::new(),
    }
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
