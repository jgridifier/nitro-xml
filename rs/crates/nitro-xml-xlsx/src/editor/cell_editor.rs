// SPDX-License-Identifier: MIT
//! In-cell / formula-bar edit buffer. Key handling uses the #55 text surface
//! in the wasm host; this type only stores the buffer those keys update and
//! the address a commit writes with `setCellInput`.

/// One cell's edit session, shared by the formula bar and the in-cell overlay.
#[derive(Debug, Clone, Default)]
pub struct CellEditor {
    editing: bool,
    sheet: u32,
    row: u32,
    col: u32,
    text: String,
    anchor: usize,
    focus: usize,
    expanded: bool,
}

impl CellEditor {
    pub fn is_editing(&self) -> bool {
        self.editing
    }

    pub fn sheet(&self) -> u32 {
        self.sheet
    }

    pub fn row(&self) -> u32 {
        self.row
    }

    pub fn col(&self) -> u32 {
        self.col
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn anchor(&self) -> usize {
        self.anchor
    }

    pub fn focus(&self) -> usize {
        self.focus
    }

    pub fn expanded(&self) -> bool {
        self.expanded
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }

    /// Start editing `text` (existing cell text, or the character that replaced it).
    pub fn begin(&mut self, sheet: u32, row: u32, col: u32, text: String) {
        let end = text.len();
        self.editing = true;
        self.sheet = sheet;
        self.row = row;
        self.col = col;
        self.text = text;
        self.anchor = end;
        self.focus = end;
    }

    /// Replace the buffer after a text-surface key. Offsets are UTF-8 bytes.
    pub fn set_buffer(&mut self, text: String, anchor: usize, focus: usize) {
        let len = text.len();
        self.text = text;
        self.anchor = anchor.min(len);
        self.focus = focus.min(len);
        self.editing = true;
    }

    pub fn cancel(&mut self) {
        self.editing = false;
        self.text.clear();
        self.anchor = 0;
        self.focus = 0;
    }

    /// Leave edit mode and return the text to commit with `setCellInput`.
    pub fn take_commit(&mut self) -> Option<(u32, u32, u32, String)> {
        if !self.editing {
            return None;
        }
        let out = (self.sheet, self.row, self.col, self.text.clone());
        self.editing = false;
        Some(out)
    }
}
