// SPDX-License-Identifier: MIT
//! Word print-layout engine (port of upstream `src/core/docx/layout`).

mod adapt_drop_cap;
mod adapt_floats;
mod adapt_notes;
mod adapt_paragraph_box;
mod adapt_table;
mod adapter;
mod balance_columns;
mod flow_paragraph;
mod flow_table;
mod floats;
mod fonts;
mod input;
mod keep_rules;
mod layout;
mod layout_ligatures;
mod line_metrics;
mod measure;
mod page_cursor;
mod page_flow;
mod page_fields;
mod paragraph_fragments;
mod paragraph_layout;
mod paragraph_tokens;
mod result;
mod review_display;
mod section_flow;
mod tab_stops;
mod table_layout;
mod text_breaks;
mod units;
mod util;
mod wrap;

pub use layout::{layout_document, layout_document_model};
pub use measure::create_fake_measurer;
pub use result::LayoutResult;
pub use review_display::LayoutReviewOptions;
