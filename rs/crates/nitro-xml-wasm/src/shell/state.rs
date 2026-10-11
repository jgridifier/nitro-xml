// SPDX-License-Identifier: MIT
//! Shared shell state for all viewer formats (#29).

use super::diagnostics::DiagnosticsState;

/// Which document format the shell is driving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FormatKind {
    #[default]
    None,
    Xlsx,
    Docx,
    Pptx,
}

impl FormatKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FormatKind::None => "",
            FormatKind::Xlsx => "xlsx",
            FormatKind::Docx => "docx",
            FormatKind::Pptx => "pptx",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FindMatch {
    pub sheet: u32,
    pub row: u32,
    pub col: u32,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct FindState {
    pub open: bool,
    pub query: String,
    pub case_sensitive: bool,
    pub match_entire: bool,
    pub matches: Vec<FindMatch>,
    /// Index into `matches`, or `None` when empty.
    pub index: Option<usize>,
}

impl FindState {
    pub fn status(&self) -> String {
        if !self.open {
            return String::new();
        }
        if self.query.is_empty() {
            return String::new();
        }
        let n = self.matches.len();
        if n == 0 {
            return "0 results".into();
        }
        let i = self.index.map(|i| i + 1).unwrap_or(0);
        format!("{i} of {n}")
    }

    pub fn current(&self) -> Option<&FindMatch> {
        self.index.and_then(|i| self.matches.get(i))
    }
}

#[derive(Debug, Clone, Default)]
pub struct PendingHyperlink {
    pub target: Option<String>,
    pub location: Option<String>,
    pub open: bool,
    pub tooltip: Option<String>,
}

/// Viewer shell controller state (format-agnostic).
#[derive(Debug, Clone)]
pub struct ShellState {
    pub format: FormatKind,
    /// Zoom percent in \[10, 400\], matching upstream `zoom_percent`.
    pub zoom_pct: f64,
    pub find: FindState,
    pub diagnostics: DiagnosticsState,
    pub diagnostics_open: bool,
    /// Active sheet / page / slide index (format-specific meaning).
    pub nav_index: u32,
    /// Visible tab labels for the chrome (xlsx sheets; later pages/slides).
    pub tabs: Vec<(u32, String)>,
    /// Selection anchor for copy (xlsx A1-style; empty = A1).
    pub selection_ref: String,
    pub pending_hyperlink: Option<PendingHyperlink>,
    /// Last TSV produced by copy (for tests / parity dumps).
    pub last_copy: String,
    pub dirty_ui: bool,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            format: FormatKind::None,
            zoom_pct: 100.0,
            find: FindState::default(),
            diagnostics: DiagnosticsState::default(),
            diagnostics_open: false,
            nav_index: 0,
            tabs: Vec::new(),
            selection_ref: "A1".into(),
            pending_hyperlink: None,
            last_copy: String::new(),
            dirty_ui: true,
        }
    }
}

impl ShellState {
    pub fn clamp_zoom(z: f64) -> f64 {
        if !z.is_finite() || z <= 0.0 {
            return 100.0;
        }
        let percent = if z <= 4.0 { z * 100.0 } else { z };
        percent.clamp(10.0, 400.0)
    }

    pub fn set_zoom(&mut self, z: f64) {
        self.zoom_pct = Self::clamp_zoom(z);
        self.dirty_ui = true;
    }

    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom_pct + 10.0);
    }

    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom_pct - 10.0);
    }

    pub fn open_find(&mut self) {
        self.find.open = true;
        self.dirty_ui = true;
    }

    pub fn close_find(&mut self) {
        self.find.open = false;
        self.dirty_ui = true;
    }

    pub fn toggle_diagnostics(&mut self) {
        self.diagnostics_open = !self.diagnostics_open;
        self.dirty_ui = true;
    }

    pub fn set_tabs(&mut self, tabs: Vec<(u32, String)>, active: u32) {
        self.tabs = tabs;
        self.nav_index = active;
        self.dirty_ui = true;
    }

    pub fn take_pending_hyperlink(&mut self) -> Option<PendingHyperlink> {
        self.dirty_ui = true;
        self.pending_hyperlink.take()
    }
}
