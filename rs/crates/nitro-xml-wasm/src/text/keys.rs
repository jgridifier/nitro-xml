// SPDX-License-Identifier: MIT
//! Keyboard mapping for the single-line text surface (#55).
//!
//! Mirrors textarea / upstream `grid-keys.ts` + `formula-input.ts` editing keys.
//! Commit / cancel / point-mode are host concerns (`TODO(#69)`).

use super::bidi::{arrow_left, arrow_right, collapse_selection_on_arrow, needs_bidi};
use super::caret::{move_line_end, move_line_start, move_word_left, move_word_right};
use super::selection::TextField;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: String,
    pub shift: bool,
    pub ctrl: bool,
    pub meta: bool,
    pub alt: bool,
    pub is_mac: bool,
}

impl KeyEvent {
    fn mod_key(&self) -> bool {
        if self.is_mac {
            self.meta
        } else {
            self.ctrl
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    Handled,
    Unhandled,
    Copy,
    Cut,
    Paste,
    /// Enter / Tab — host commits (`TODO(#69)`).
    Commit,
    /// Escape — host cancels (`TODO(#69)`).
    Cancel,
}

/// Apply a keydown to `field`. Printable characters are not synthesized here;
/// the host feeds them via [`TextField::insert`] / Input events (#56).
pub fn apply_key(field: &mut TextField, ev: &KeyEvent) -> KeyAction {
    let mod_key = ev.mod_key();
    let shift = ev.shift;
    let key = ev.key.as_str();

    // Clipboard chords
    if mod_key && !ev.alt {
        match key {
            "a" | "A" => {
                field.select_all();
                return KeyAction::Handled;
            }
            "c" | "C" => return KeyAction::Copy,
            "x" | "X" => return KeyAction::Cut,
            "v" | "V" => return KeyAction::Paste,
            _ => {}
        }
    }

    let rtl_para = needs_bidi(field.text());

    match key {
        "ArrowLeft" => {
            if mod_key {
                move_word_left(field, shift);
            } else if !shift && !field.selection().is_collapsed() {
                collapse_selection_on_arrow(field, "ArrowLeft", rtl_para);
            } else {
                arrow_left(field, shift, rtl_para);
            }
            KeyAction::Handled
        }
        "ArrowRight" => {
            if mod_key {
                move_word_right(field, shift);
            } else if !shift && !field.selection().is_collapsed() {
                collapse_selection_on_arrow(field, "ArrowRight", rtl_para);
            } else {
                arrow_right(field, shift, rtl_para);
            }
            KeyAction::Handled
        }
        "Home" => {
            move_line_start(field, shift);
            KeyAction::Handled
        }
        "End" => {
            move_line_end(field, shift);
            KeyAction::Handled
        }
        "Backspace" => {
            if mod_key {
                field.delete_word_backward();
            } else {
                field.delete_backward();
            }
            KeyAction::Handled
        }
        "Delete" => {
            if mod_key {
                field.delete_word_forward();
            } else {
                field.delete_forward();
            }
            KeyAction::Handled
        }
        "Enter" if ev.alt => {
            field.insert("\n");
            KeyAction::Handled
        }
        "Enter" | "Tab" => KeyAction::Commit,
        "Escape" => KeyAction::Cancel,
        _ => KeyAction::Unhandled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_mod_left_skips_word_not_grapheme() {
        // Catches: Ctrl/Meta+Left must jump a word, not a single grapheme.
        let mut f = TextField::new("hello world");
        f.set_caret(f.text().len());
        let ev = KeyEvent {
            key: "ArrowLeft".into(),
            shift: false,
            ctrl: true,
            meta: false,
            alt: false,
            is_mac: false,
        };
        assert_eq!(apply_key(&mut f, &ev), KeyAction::Handled);
        assert_eq!(f.focus(), 6); // start of "world"
        assert_eq!(f.selected_text(), "");
    }

    #[test]
    fn catches_arrow_without_shift_collapses_selection_to_edge() {
        // Catches: Left on a range must collapse to start (textarea), not shrink by one.
        let mut f = TextField::new("abcd");
        f.set_caret(1);
        f.extend_to(3);
        let ev = KeyEvent {
            key: "ArrowLeft".into(),
            shift: false,
            ctrl: false,
            meta: false,
            alt: false,
            is_mac: false,
        };
        assert_eq!(apply_key(&mut f, &ev), KeyAction::Handled);
        assert!(f.selection().is_collapsed());
        assert_eq!(f.focus(), 1);
    }
}
