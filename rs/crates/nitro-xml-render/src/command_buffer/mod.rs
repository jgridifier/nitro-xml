// SPDX-License-Identifier: MIT
//! Per-frame command buffer: typed draw/DOM ops the loader applies in one pass.
//!
//! Editor chrome uses the same buffer (Head of Product, R0). Opcode gaps for
//! SVG NS, styles, classList, insert/remove and markup parsing are listed in
//! `docs/boundary.md` and filled as painters land.

mod ops;
mod writer;

pub use ops::opcode;
pub use writer::CommandBuffer;
