// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/layout.ts; changes listed in PROVENANCE.md.

use std::collections::HashSet;

use crate::body::DocumentModel;
use crate::layout::adapter::adapt_document_model;
use crate::layout::bidi::apply_uax9_result;
use crate::layout::floats::{position_floats, FLOAT_WRAP_NOTE};
use crate::layout::input::LayoutDocumentInput;
use crate::layout::measure::{
    create_fake_measurer, CachedMeasurer, MeasureCache, MeasureSample, PendingMeasure, TextMeasurer,
};
use crate::layout::page_flow::layout_sections;
use crate::layout::result::LayoutResult;
use crate::layout::review_display::LayoutReviewOptions;
use crate::layout::wrap::{same_exclusions, wrap_exclusions};
use crate::review_document_formatting::review_document_formatting;

pub fn layout_document(input: &LayoutDocumentInput, measurer: &dyn TextMeasurer) -> LayoutResult {
    let result = layout_sections(input, measurer, None);
    finalize_floats(input, measurer, result)
}

/// Float exclusion passes shared by eager layout and [`super::session::LayoutSession`].
pub(crate) fn finalize_floats(
    input: &LayoutDocumentInput,
    measurer: &dyn TextMeasurer,
    mut result: LayoutResult,
) -> LayoutResult {
    if !position_floats(input, &mut result.pages) {
        return result;
    }
    let mut exclusions = wrap_exclusions(&result.pages);
    for _pass in 0..3 {
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
    result.approximations = approx
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    result
}

/// Adapt the document model to layout input (review projection + adapter).
pub fn prepare_layout_document_input(
    model: &DocumentModel,
    options: LayoutReviewOptions,
) -> (LayoutDocumentInput, Vec<String>) {
    let mode = match options
        .review_display_mode
        .unwrap_or(crate::review_formatting_display::ReviewDisplayMode::All)
    {
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
                d.run_index
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
    (input, approximations)
}

pub fn layout_document_model(
    model: &DocumentModel,
    measurer: &dyn TextMeasurer,
    options: LayoutReviewOptions,
) -> LayoutResult {
    let (input, approximations) = prepare_layout_document_input(model, options);
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

pub struct CachedLayout {
    pub result: LayoutResult,
    pub pending: Vec<PendingMeasure>,
}

/// One layout pass against `cache`. When every probe hits, UAX #9 is applied
/// to fragment x positions (page breaks stay the same).
pub fn layout_document_model_cached(
    model: &DocumentModel,
    cache: &MeasureCache,
    options: LayoutReviewOptions,
) -> CachedLayout {
    let measurer = CachedMeasurer::new(cache);
    let mut result = layout_document_model(model, &measurer, options);
    let pending = measurer.take_pending();
    if pending.is_empty() {
        apply_uax9_result(&mut result);
    }
    CachedLayout { result, pending }
}

/// Re-layout until `resolve` has filled every miss recorded by [`CachedMeasurer`].
pub fn layout_until_measured(
    model: &DocumentModel,
    cache: &mut MeasureCache,
    options: LayoutReviewOptions,
    resolve: &mut dyn FnMut(&[PendingMeasure]) -> Vec<MeasureSample>,
) -> LayoutResult {
    let mut last = None;
    for _ in 0..48 {
        let measured = layout_document_model_cached(model, cache, options.clone());
        if measured.pending.is_empty() {
            return measured.result;
        }
        let samples = resolve(&measured.pending);
        cache.absorb(&measured.pending, &samples);
        last = Some(measured.result);
    }
    last.unwrap_or_else(|| layout_document_model(model, &create_fake_measurer(), options))
}
