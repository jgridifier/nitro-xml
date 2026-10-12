// SPDX-License-Identifier: MIT
//! Word (.docx) styles, numbering, theme and settings (Stage 4 port of ChristopherVR/ooxml@e66aaec).

#![allow(dead_code)] // Upstream-shaped layout API; not every helper is wired in the Rust viewer yet (#22).
#![allow(
    clippy::too_many_arguments,
    clippy::module_inception,
    clippy::large_enum_variant,
    clippy::manual_strip,
    clippy::manual_is_multiple_of,
    clippy::match_like_matches_macro,
    clippy::unnecessary_wraps,
    clippy::redundant_closure,
    clippy::single_char_pattern,
    clippy::if_same_then_else,
    clippy::unnecessary_lazy_evaluations,
    clippy::manual_unwrap_or,
    clippy::unnecessary_unwrap
)]

mod body;
mod borders;
mod breaks;
mod chart_host;
mod chart_paint;
mod comments;
mod default_styles;
mod drawing;
pub mod dump;
pub mod edit;
mod equation;
mod field_runs;
mod fields;
mod hyperlink;
mod json_util;
mod layout;
mod ligatures;
mod model_dump;
mod note_format;
mod notes;
mod numbering;
mod paragraph_alignment;
mod paragraph_scan;
mod resolve_table;
mod review_document_formatting;
mod review_formatting_display;
mod revisions;
mod run;
mod run_formatting;
mod run_properties;
mod sections;
mod settings;
mod simple_types;
mod styles;
mod tab_stops;
mod table;
mod table_styles;
mod table_visuals;
mod theme;
mod theme_color;
mod warnings;
mod watermark;
pub mod write;
mod xml_util;

pub use body::{
    get_media, parse_blocks_from_container, read_package, DocumentModel, LoadedDocument,
};
pub use write::{save_docx, save_docx_with_options, DocxSaveOptions};

/// Built-in `word/styles.xml` when the package has no styles part (upstream `DEFAULT_STYLES_XML`).
pub use default_styles::default_styles_xml;
pub use numbering::{
    compute_list_labels, display_list_label, format_list_number, parse_numbering_catalog,
    resolve_numbering_level, resolve_paragraph_numbering,
};
pub use settings::{
    parse_settings, read_page_setup, validate_page_setup, PageSetupValues, SectionProps,
};
pub use styles::{
    default_style_catalogs, parse_formatting, parse_outline_level, parse_paragraph_style_catalog,
    parse_run_style_catalog, resolve_paragraph_formatting, resolve_style_numbering, ParagraphProps,
    PAGINATION_KEYS,
};
pub use table_styles::parse_table_style_catalog;
pub use theme::{diagram_color_theme, parse_color_scheme_mapping, parse_theme};

pub mod render;
pub use layout::{
    apply_uax9_result, create_fake_measurer, layout_document, layout_document_model,
    layout_document_model_cached, layout_until_measured, measure_font_css, relayout_incremental,
    relayout_incremental_model, reorder_line_fragments_uax9, CachedLayout, CachedMeasurer,
    DirtyHint, LayoutResult, LayoutSession, MeasureCache, MeasureKey, MeasureSample, PendingKind,
    PendingMeasure, TickStatus,
};
pub use render::{
    line_numbering_from_section, page_borders_from_section, paint_print_layout,
    suppressed_line_number_ids, PageBorders, PrintLayoutOptions, PrintLineNumbering, PrintNow,
    PrintSession, StringIntern as PrintStringIntern,
};
