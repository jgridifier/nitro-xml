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
    /// Ribbon/shell/tabs/find/overlay changed; formula-bar-only edits must not set this.
    pub dirty_chrome_rest: bool,
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
            dirty_chrome_rest: true,
        }
    }
}

/// ASCII-only case fold. Length and char boundaries match JS `for...of` folding
/// of `A`–`Z`, so docx/pptx shell find stays aligned with the TS oracle.
pub fn fold_ascii(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_uppercase() {
                c.to_ascii_lowercase()
            } else {
                c
            }
        })
        .collect()
}

/// First `max_chars` Unicode scalars.
pub fn cap_chars(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        out.push(c);
    }
    out
}

/// Record substring hits. `row` is a format-specific ordinal (cell row, paragraph, element).
pub fn push_text_hits(
    matches: &mut Vec<FindMatch>,
    sheet: u32,
    row: u32,
    text: &str,
    query: &str,
    case_sensitive: bool,
    match_entire: bool,
) {
    if query.is_empty() || text.is_empty() {
        return;
    }
    if match_entire {
        let ok = if case_sensitive {
            text == query
        } else {
            fold_ascii(text) == fold_ascii(query)
        };
        if ok {
            matches.push(FindMatch {
                sheet,
                row,
                col: 0,
                text: cap_chars(text, 180),
            });
        }
        return;
    }
    let hay = if case_sensitive {
        text.to_owned()
    } else {
        fold_ascii(text)
    };
    let needle = if case_sensitive {
        query.to_owned()
    } else {
        fold_ascii(query)
    };
    if needle.is_empty() {
        return;
    }
    let mut start = 0usize;
    while let Some(rel) = hay[start..].find(&needle) {
        let s = start + rel;
        let e = s + needle.len();
        if text.is_char_boundary(s) && text.is_char_boundary(e) {
            // Scalar index, not a byte offset, so the TS oracle's code-point walk agrees.
            let col = text[..s].chars().count() as u32;
            matches.push(FindMatch {
                sheet,
                row,
                col,
                text: cap_chars(&text[s..e], 180),
            });
        }
        start = s + needle.len().max(1);
        if start >= hay.len() {
            break;
        }
    }
}

impl ShellState {
    pub fn touch_ui(&mut self) {
        self.dirty_ui = true;
    }

    pub fn touch_chrome_rest(&mut self) {
        self.dirty_ui = true;
        self.dirty_chrome_rest = true;
    }

    pub fn clamp_zoom(z: f64) -> f64 {
        if !z.is_finite() || z <= 0.0 {
            return 100.0;
        }
        let percent = if z <= 4.0 { z * 100.0 } else { z };
        percent.clamp(10.0, 400.0)
    }

    pub fn set_zoom(&mut self, z: f64) {
        self.zoom_pct = Self::clamp_zoom(z);
        self.touch_chrome_rest();
    }

    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom_pct + 10.0);
    }

    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom_pct - 10.0);
    }

    pub fn open_find(&mut self) {
        self.find.open = true;
        self.touch_chrome_rest();
    }

    pub fn close_find(&mut self) {
        self.find.open = false;
        self.touch_chrome_rest();
    }

    pub fn toggle_diagnostics(&mut self) {
        self.diagnostics_open = !self.diagnostics_open;
        self.touch_chrome_rest();
    }

    pub fn set_tabs(&mut self, tabs: Vec<(u32, String)>, active: u32) {
        self.tabs = tabs;
        self.nav_index = active;
        self.touch_chrome_rest();
    }

    pub fn take_pending_hyperlink(&mut self) -> Option<PendingHyperlink> {
        self.touch_chrome_rest();
        self.pending_hyperlink.take()
    }
}
