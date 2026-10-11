// SPDX-License-Identifier: MIT
//! Word (.docx) styles, numbering, theme and settings (Stage 4 port of ChristopherVR/ooxml@e66aaec).

mod borders;
mod default_styles;
pub mod dump;
mod json_util;
mod ligatures;
mod numbering;
mod paragraph_alignment;
mod paragraph_scan;
mod run_properties;
mod settings;
mod simple_types;
mod styles;
mod theme;
mod xml_util;

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
pub use theme::{diagram_color_theme, parse_color_scheme_mapping, parse_theme};
