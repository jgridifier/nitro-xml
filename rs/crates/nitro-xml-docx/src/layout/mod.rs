// SPDX-License-Identifier: MIT
//! Word print-layout engine (port of upstream `src/core/docx/layout`).

mod adapt_drop_cap;
mod adapt_floats;
mod adapt_notes;
mod adapt_paragraph_box;
mod adapt_table;
mod adapter;
mod balance_columns;
mod bidi;
mod floats;
mod flow_paragraph;
mod flow_table;
mod fonts;
mod input;
mod keep_rules;
mod layout;
mod layout_ligatures;
mod line_metrics;
mod measure;
mod page_cursor;
mod page_fields;
mod page_flow;
mod paragraph_fragments;
mod paragraph_layout;
mod paragraph_tokens;
mod result;
mod review_display;
mod section_flow;
mod session;
mod tab_stops;
mod table_layout;
mod text_breaks;
mod units;
mod util;
mod wrap;

pub use bidi::{apply_uax9_result, reorder_line_fragments_uax9};
pub use layout::{
    layout_document, layout_document_model, layout_document_model_cached, layout_until_measured,
    CachedLayout,
};
pub use measure::{
    create_fake_measurer, CachedMeasurer, MeasureCache, MeasureKey, MeasureSample, PendingKind,
    PendingMeasure,
};
pub use result::LayoutResult;
pub use review_display::LayoutReviewOptions;
pub use session::{LayoutSession, TickStatus};
