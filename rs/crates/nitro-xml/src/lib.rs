// SPDX-License-Identifier: MIT
//! Nitro XML for native Rust users: re-exports the format crates.
//!
//! Mirrors the upstream entry point ChristopherVR/ooxml@e66aaec `src/core/index.ts`, restricted to
//! .xlsx, .docx and .pptx.

pub use nitro_xml_docx as docx;
pub use nitro_xml_opc as opc;
pub use nitro_xml_pptx as pptx;
pub use nitro_xml_xlsx as xlsx;
