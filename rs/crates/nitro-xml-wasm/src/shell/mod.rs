// SPDX-License-Identifier: MIT
//! Stage 6 viewer shell (#29): zoom, navigation, find, copy, hyperlinks, diagnostics.
//!
//! Format-agnostic shell state lives here. Xlsx, docx and pptx all drive zoom,
//! tabs, find, copy and diagnostics through [`crate::handle::Handle`]. UI chrome
//! is bound by a thin loader layer from [`ShellUi`] snapshots — logic stays in Rust/WASM.

mod actions;
mod chrome;
mod diagnostics;
mod state;
mod ui;

pub use actions::{apply_action, ShellAction};
pub use chrome::chrome_rest_html;
#[cfg(test)]
pub use chrome::{chrome_rest_html_calls, chrome_rest_html_calls_reset};
pub use diagnostics::open_diagnostic_code;
pub use state::{cap_chars, push_text_hits, FindMatch, FormatKind, PendingHyperlink, ShellState};
pub use ui::ShellUi;
