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
mod incremental;
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
pub use incremental::{relayout_incremental, relayout_incremental_model, DirtyHint};
pub use layout::{
    layout_document, layout_document_model, layout_document_model_cached, layout_until_measured,
    prepare_layout_document_input, CachedLayout,
};
pub use measure::{
    create_fake_measurer, measure_font_css, CachedMeasurer, MeasureCache, MeasureKey,
    MeasureSample, PendingKind, PendingMeasure,
};
pub use result::{strip_layout_ir_chart_part_names, LayoutResult};
pub use review_display::LayoutReviewOptions;
pub use session::{LayoutSession, TickStatus};

// Reachable from the Print Layout painter (`render/print`) without widening the public dump API.
pub(crate) use adapt_floats::paragraph_floats;
pub(crate) use floats::float_position;
pub(crate) use fonts::css_font_stack;
pub(crate) use input::{
    LayoutBorder, LayoutChart, LayoutDocumentInput, LayoutFloat, LayoutParagraphBorder,
};
pub(crate) use layout_ligatures::ligature_css;
pub(crate) use page_fields::{
    field_display_text, page_number_values, page_numbers, PageFieldContext,
};
pub(crate) use result::{
    LayoutBlockBox, LayoutCellGeometry, LayoutColumnBox, LayoutFragment, LayoutLine, LayoutPageBox,
    LayoutParagraphBox, LayoutParagraphFrame, LayoutTableBox, LayoutTableRowBox,
};
pub(crate) use units::DEFAULT_FONT_SIZE_PT;
