// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/layout.ts; changes listed in PROVENANCE.md.

use std::collections::HashSet;

use crate::body::DocumentModel;
use crate::layout::adapter::adapt_document_model;
use crate::layout::floats::{position_floats, FLOAT_WRAP_NOTE};
use crate::layout::input::LayoutDocumentInput;
use crate::layout::measure::{create_fake_measurer, FakeMeasurer, TextMeasurer};
use crate::layout::page_flow::layout_sections;
use crate::layout::result::LayoutResult;
use crate::layout::review_display::LayoutReviewOptions;
use crate::layout::wrap::{same_exclusions, wrap_exclusions};
use crate::review_document_formatting::review_document_formatting;

pub fn layout_document(input: &LayoutDocumentInput, measurer: &FakeMeasurer) -> LayoutResult {
    let mut result = layout_sections(input, measurer, None);
    if !position_floats(input, &mut result.pages) {
        return result;
    }
    let mut exclusions = wrap_exclusions(&result.pages);
    for pass in 0..3 {
        if exclusions.is_empty() {
            break;
        }
        result = layout_sections(input, measurer, Some(exclusions.clone()));
        position_floats(input, &mut result.pages);
        let next = wrap_exclusions(&result.pages);
        if same_exclusions(&next, &exclusions) {
            break;
        }
        exclusions = next;
    }
    let mut approx = result.approximations;
    approx.push(FLOAT_WRAP_NOTE.to_string());
    result.approximations = approx.into_iter().collect::<HashSet<_>>().into_iter().collect();
    result
}

pub fn layout_document_model(
    model: &DocumentModel,
    measurer: &FakeMeasurer,
    options: LayoutReviewOptions,
) -> LayoutResult {
    let mode = match options.review_display_mode.unwrap_or(crate::review_formatting_display::ReviewDisplayMode::All) {
        crate::review_formatting_display::ReviewDisplayMode::All => "all",
        crate::review_formatting_display::ReviewDisplayMode::Simple => "simple",
        crate::review_formatting_display::ReviewDisplayMode::Final => "final",
        crate::review_formatting_display::ReviewDisplayMode::Original => "original",
    };
    let projected = review_document_formatting(model, mode);
    let mut approximations: Vec<String> = projected
        .diagnostics
        .iter()
        .map(|d| {
            format!(
                "Original formatting unavailable in paragraph {}{}: {}",
                d.paragraph_id,
                d
                    .run_index
                    .map(|i| format!(", run {}", i))
                    .unwrap_or_default(),
                d.message
            )
        })
        .collect();
    let mut layout_note = |message: String| {
        approximations.push(message);
    };
    let input = adapt_document_model(&projected.model, &mut layout_note, &options);
    let result = layout_document(&input, measurer);
    let mut merged = HashSet::new();
    for a in approximations {
        merged.insert(a);
    }
    for a in result.approximations {
        merged.insert(a);
    }
    LayoutResult {
        pages: result.pages,
        approximations: merged.into_iter().collect(),
    }
}
