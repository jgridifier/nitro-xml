// SPDX-License-Identifier: MIT
//! Shared drawing primitives reused by format painters.
//!
//! Emit command-buffer ops; never call JS per element. Ports land with the
//! first painter that needs them (#18 / #23 / #27).

pub mod fill;
pub mod image;
pub mod line;
pub mod text_run;
