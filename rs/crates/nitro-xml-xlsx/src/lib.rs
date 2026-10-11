// SPDX-License-Identifier: MIT
//! Excel (.xlsx / .xlsm / .xltx / .xltm) workbook model, built on [`nitro_xml_opc`].
//!
//! Stage 3 (#15): workbook part, shared strings, styles and theme. Worksheet cell parse is a
//! later issue. Number formats live in [`numfmt`] (#17). Grid painters (#18) encode through
//! [`nitro_xml_render`].

pub mod address;
pub mod builtin_formats;
pub mod model;
pub mod numfmt;
pub mod password_hash;
pub mod shared_strings;
pub mod style_parts;
pub mod styles;
pub mod theme;
pub mod workbook;
pub mod xml_util;

pub use model::WorkbookSlice;
pub use workbook::load_workbook;
