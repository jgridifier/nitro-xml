// SPDX-License-Identifier: MIT
//! Mobile virtual keyboard + touch selection handles for the text surface (#60).
//!
//! Host chrome keeps a hidden `<input>` (or contenteditable bridge) in sync via
//! [`MobileSurface::host_snapshot`]: `inputmode`, `enterkeyhint`, value and
//! UTF-16 selection. Incoming `beforeinput` / `input` `inputType` values are
//! applied to the owned [`TextField`]. Autocorrect / predictive commits use
//! `insertReplacementText` (same insert path as IME commit in #56 when available).

use super::caret::{caret_rect, offset_at_x, CaretRect, WidthFn};
use super::selection::{byte_to_utf16, utf16_to_byte, TextField};

/// Which native selection handle is being dragged (UTF-16 indices).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionHandle {
    Start,
    End,
}

/// Mobile keyboard / hidden-input bridge configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MobileConfig {
    /// When false (desktop), mobile-only rewrites and handles are suppressed.
    pub mobile_enabled: bool,
    pub readonly: bool,
    /// Maps to the hidden input `inputmode` attribute (`text`, `decimal`, …).
    pub input_mode: String,
    /// Maps to `enterkeyhint` (`done`, `enter`, `search`, …).
    pub enter_key_hint: String,
    /// Maps to `type` on the bridge (`text`, `search`, …).
    pub input_type: String,
}

impl Default for MobileConfig {
    fn default() -> Self {
        Self {
            mobile_enabled: true,
            readonly: false,
            input_mode: "text".to_string(),
            enter_key_hint: "done".to_string(),
            input_type: "text".to_string(),
        }
    }
}

/// Owned text field plus mobile bridge state for the cell editor / formula bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MobileSurface {
    pub field: TextField,
    config: MobileConfig,
}

impl MobileSurface {
    /// Host constructor when the caret starts at end-of-text.
    #[allow(dead_code)] // Public host API; headless eval uses `from_utf16_selection`.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            field: TextField::new(text),
            config: MobileConfig::default(),
        }
    }

    pub fn from_utf16_selection(text: &str, anchor_u16: usize, focus_u16: usize) -> Self {
        Self {
            field: TextField::from_utf16_selection(text, anchor_u16, focus_u16),
            config: MobileConfig::default(),
        }
    }

    pub fn config(&self) -> &MobileConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: MobileConfig) {
        self.config = config;
    }

    pub fn mobile_active(&self) -> bool {
        self.config.mobile_enabled && !self.config.readonly
    }

    /// Apply a `beforeinput`-style `inputType` (+ optional `data`).
    ///
    /// Returns `true` when the op mutated the field. Readonly and desktop mode
    /// reject autocorrect (`insertReplacementText`) and other edits.
    pub fn apply_before_input(&mut self, input_type: &str, data: Option<&str>) -> bool {
        if self.config.readonly {
            return false;
        }
        if input_type == "insertReplacementText" && !self.config.mobile_enabled {
            return false;
        }
        match input_type {
            "insertText" | "insertFromPaste" | "insertFromDrop" => {
                if let Some(t) = data {
                    self.field.insert(t);
                    true
                } else {
                    false
                }
            }
            "insertReplacementText" => {
                if let Some(t) = data {
                    self.field.insert(t);
                    true
                } else {
                    false
                }
            }
            "insertCompositionText" => {
                if let Some(t) = data {
                    apply_composition_commit(&mut self.field, t);
                    true
                } else {
                    false
                }
            }
            "deleteContentBackward" => {
                self.field.delete_backward();
                true
            }
            "deleteContentForward" => {
                self.field.delete_forward();
                true
            }
            "deleteWordBackward" => {
                self.field.delete_word_backward();
                true
            }
            "deleteWordForward" => {
                self.field.delete_word_forward();
                true
            }
            "insertLineBreak" => {
                self.field.insert("\n");
                true
            }
            _ => false,
        }
    }

    /// Drag a selection handle to a UTF-16 caret index (touch handles).
    pub fn drag_handle(&mut self, which: SelectionHandle, utf16: usize) -> bool {
        if !self.mobile_active() {
            return false;
        }
        match which {
            SelectionHandle::Start => {
                let text = self.field.text();
                let focus_u16 = byte_to_utf16(text, self.field.focus());
                self.field = TextField::from_utf16_selection(text, utf16, focus_u16);
            }
            SelectionHandle::End => {
                let byte = utf16_to_byte(self.field.text(), utf16);
                self.field.extend_to(byte);
            }
        }
        true
    }

    /// Pointer hit-test on the field content box (x in CSS px) while extending selection.
    pub fn pointer_at_x(&mut self, x: f32, extend: bool, width_of: &WidthFn<'_>) {
        if !self.mobile_active() {
            return;
        }
        let off = offset_at_x(self.field.text(), x, width_of);
        if extend {
            self.field.extend_to(off);
        } else {
            self.field.set_caret(off);
        }
    }

    /// JSON-friendly snapshot for the host hidden input + handle layer.
    pub fn host_snapshot(
        &self,
        _char_width: f32,
        line_height: f32,
        width_of: &WidthFn<'_>,
    ) -> serde_json::Value {
        let (anchor16, focus16) = self.field.utf16_selection();
        let active = self.mobile_active();
        let show_handles = active;
        let start_byte = self.field.selection().start();
        let end_byte = self.field.selection().end();
        let y = 0.0f32;
        let caret = caret_rect(
            self.field.text(),
            self.field.focus(),
            y,
            line_height,
            width_of,
        );
        let mut handles = serde_json::json!({});
        if show_handles {
            let start_rect = caret_rect(self.field.text(), start_byte, y, line_height, width_of);
            let end_rect = caret_rect(self.field.text(), end_byte, y, line_height, width_of);
            handles = serde_json::json!({
                "start": rect_json(&start_rect),
                "end": rect_json(&end_rect),
            });
        }
        let has_selection = anchor16 != focus16;
        let (bridge_value, sel_start, sel_end) = bridge_value_utf16(&self.field);
        serde_json::json!({
            "text": self.field.text(),
            "bridgeValue": bridge_value,
            "selectionStart": sel_start,
            "selectionEnd": sel_end,
            "anchor": anchor16,
            "focus": focus16,
            "selected": self.field.selected_text(),
            "mobileActive": active,
            "showHandles": show_handles,
            "readonly": self.config.readonly,
            "inputMode": self.config.input_mode,
            "inputType": self.config.input_type,
            "enterKeyHint": self.config.enter_key_hint,
            "autocomplete": "off",
            "autocorrect": if active { "on" } else { "off" },
            "spellcheck": active,
            "caret": rect_json(&caret),
            "handles": handles,
            "contextMenu": {
                "copy": !self.config.readonly,
                "cut": active && has_selection,
                "paste": active,
                "selectAll": !self.config.readonly,
            },
        })
    }
}

fn rect_json(r: &CaretRect) -> serde_json::Value {
    serde_json::json!({ "x": r.x, "y": r.y, "w": r.w, "h": r.h })
}

fn apply_composition_commit(field: &mut TextField, text: &str) {
    // TODO(#56): delegate to `text::ime` composition commit when PR #202 lands on main.
    field.insert(text);
}

/// Re-sync hidden-input value/selection after a Rust-side mutation.
pub fn bridge_value_utf16(field: &TextField) -> (String, usize, usize) {
    let (a, f) = field.utf16_selection();
    (field.text().to_string(), a, f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mono(s: &str) -> f32 {
        s.chars().count() as f32 * 8.0
    }

    #[test]
    fn catches_autocorrect_replacement_inserts_at_selection() {
        // Catches: mobile autocorrect must replace the active range, not append at EOL.
        let mut s = MobileSurface::from_utf16_selection("teh", 0, 3);
        assert!(s.apply_before_input("insertReplacementText", Some("the")));
        assert_eq!(s.field.text(), "the");
        assert_eq!(s.field.utf16_selection(), (3, 3));
    }

    #[test]
    fn catches_desktop_rejects_insert_replacement_text() {
        // Catches: desktop mode must not apply autocorrect rewrite (negative control).
        let mut s = MobileSurface::from_utf16_selection("teh", 0, 3);
        s.config.mobile_enabled = false;
        assert!(!s.apply_before_input("insertReplacementText", Some("the")));
        assert_eq!(s.field.text(), "teh");
    }

    #[test]
    fn catches_handle_drag_moves_anchor_in_utf16() {
        // Catches: dragging the start handle must move anchor in UTF-16 space (surrogate-safe).
        let mut s = MobileSurface::from_utf16_selection("abcd", 1, 3);
        s.drag_handle(SelectionHandle::Start, 0);
        assert_eq!(s.field.utf16_selection(), (0, 3));
        s.drag_handle(SelectionHandle::End, 4);
        assert_eq!(s.field.utf16_selection(), (0, 4));
    }

    #[test]
    fn catches_readonly_hides_handles_in_snapshot() {
        // Catches: readonly must not advertise touch handles or accept deletes.
        let mut s = MobileSurface::new("x");
        s.config.readonly = true;
        assert!(!s.apply_before_input("deleteContentBackward", None));
        let snap = s.host_snapshot(8.0, 16.0, &mono);
        assert_eq!(snap["showHandles"], false);
        assert_eq!(snap["mobileActive"], false);
    }
}
