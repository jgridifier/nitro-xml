// SPDX-License-Identifier: MIT
//! Shared render crate for Nitro XML.
//!
//! Holds the per-frame command-buffer types and writer, shared drawing
//! primitives, measure / post-apply layout read-back batches, the shared
//! painters **S1** (chart SVG) and **S2** (SmartArt), and **S3** (EMF/WMF
//! metafile). Format crates and `nitro-xml-wasm` depend on this crate;
//! DrawingML **model** stays in `nitro-xml-opc` (see `docs/adr/0002-renderer-port.md`).
//!
//! Painter ports land in later issues; this crate currently exposes the seed
//! command-buffer wire format (byte-identical to the loader in
//! `packages/nitro-xml/loader`) plus module stubs for primitives, S1–S3.

pub mod chart;
pub mod command_buffer;
pub mod measure;
pub mod metafile;
pub mod primitives;
pub mod smartart;

pub use command_buffer::{opcode, CommandBuffer, StringTable};
pub use measure::{
    LayoutReadBatch, LayoutReadError, LayoutReadKind, MeasureBatch, MeasureError,
    LAYOUT_RESULT_LANES,
};
