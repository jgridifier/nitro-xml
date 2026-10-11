// SPDX-License-Identifier: MIT
//! Stage 6 viewer shell (#29): zoom, navigation, find, copy, hyperlinks, diagnostics.
//!
//! Format-agnostic shell state lives here. Xlsx is wired now; docx/pptx adapters
//! plug in when their painters land (#23, #27). UI chrome is bound by a thin
//! loader layer from [`ShellUi`] snapshots — logic stays in Rust/WASM.

mod actions;
mod diagnostics;
mod state;
mod ui;

pub use actions::{apply_action, ShellAction};
pub use state::{FindMatch, FormatKind, PendingHyperlink, ShellState};
pub use ui::ShellUi;
