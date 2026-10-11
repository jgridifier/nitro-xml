// SPDX-License-Identifier: MIT
//! PowerPoint (.pptx) presentation model, built on [`nitro_xml_opc`].
//!
//! Ports ChristopherVR/ooxml@e66aaec `src/core/pptx/core/` presentation load path (issue #24).
//! Ported modules carry Apache-2.0 headers; see `PROVENANCE.md`.

mod inheritance;
mod presentation;
mod slide;
mod theme;

pub use presentation::{Error, Presentation, Slide};

pub mod render;
