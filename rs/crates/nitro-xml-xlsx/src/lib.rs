// SPDX-License-Identifier: MIT
//! Excel (.xlsx / .xlsm / .xltx / .xltm) workbook model, built on [`nitro_xml_opc`].
//!
//! Stage 3 (#15): workbook part, shared strings, styles and theme. Stage 4 (#16): active worksheet
//! cell parse. Number formats and typed input live in [`numfmt`] (#17, #130); formatted cell
//! text in [`display_text`] (#130). Grid painters (#18) encode through [`nitro_xml_render`].

pub mod address;
pub mod builtin_formats;
pub mod cells;
pub mod dates;
pub mod display_text;
pub mod formula_text;
pub mod formula_translate;
pub mod metadata;
pub mod model;
pub mod number_format;
pub mod numfmt;
pub mod password_hash;
pub mod shared_strings;
pub mod sheet_data;
pub mod sheet_data_source;
pub mod sheet_props;
pub mod style_parts;
pub mod styles;
pub mod theme;
pub mod workbook;
pub mod worksheet;
pub mod xml_util;

pub use display_text::{display_text, Cell};
pub use model::{stringify_json, WorkbookSlice};
pub use workbook::load_workbook;
