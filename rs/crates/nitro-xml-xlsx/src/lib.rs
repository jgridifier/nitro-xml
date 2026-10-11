// SPDX-License-Identifier: MIT
//! Excel (.xlsx) workbook model, built on [`nitro_xml_opc`].
//!
//! Will port ChristopherVR/ooxml@e66aaec `src/core/xlsx/` (notably `read/`, `model.ts`,
//! `workbook.ts`, `cells.ts`, `styles.ts` and `layout/`). Number formats live in [`numfmt`].
//! Grid painters (#18) encode through [`nitro_xml_render`].

pub mod numfmt;
