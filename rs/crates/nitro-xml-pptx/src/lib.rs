// SPDX-License-Identifier: MIT
//! PowerPoint (.pptx) presentation model, built on [`nitro_xml_opc`].
//!
//! Ports ChristopherVR/ooxml@e66aaec `src/core/pptx/core/` presentation load path (issue #24).
//! Ported modules carry Apache-2.0 headers; see `PROVENANCE.md`.

mod auto_number;
mod inheritance;
mod level_style;
mod outer_shadow;
mod placeholder_defaults;
mod presentation;
mod slide;
mod table;
mod table_style;
mod text;
mod text_layout;
mod theme;
mod visual_3d_materials;

pub use presentation::{Error, Presentation, Slide};
pub use render::{
    dump_pptx_view_ir, dump_pptx_view_ir_string, paint_slide_frame, SlidePaintViewport,
};

pub mod render;
