// SPDX-License-Identifier: MIT
//! Bidirectional caret movement for the single-line text surface (#57).
//!
//! Matches a `<textarea dir="auto" style="unicode-bidi: plaintext">` on Chromium:
//! paragraph embedding level from UAX #9 controls whether ArrowLeft/ArrowRight
//! move by logical previous/next grapheme or swap (RTL paragraph). Selection
//! collapse on arrow without Shift uses the visual edge in RTL paragraphs.
//! Copy and deletion stay in logical storage order (#55).

use unicode_bidi::BidiInfo;

use super::caret::{move_left, move_right};
use super::selection::TextField;

/// True when the first paragraph resolves to an RTL embedding level (UAX #9).
pub fn paragraph_is_rtl(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let info = BidiInfo::new(text, None);
    info.paragraphs
        .first()
        .map(|p| p.level.is_rtl())
        .unwrap_or(false)
}

/// Text needs bidi-aware arrow handling (RTL paragraph). Pure LTR strings use #55 paths.
pub fn needs_bidi(text: &str) -> bool {
    paragraph_is_rtl(text)
}

/// ArrowLeft: visual move to the left on screen.
pub fn arrow_left(field: &mut TextField, extend: bool, rtl_para: bool) {
    if rtl_para {
        move_right(field, extend);
    } else {
        move_left(field, extend);
    }
}

/// ArrowRight: visual move to the right on screen.
pub fn arrow_right(field: &mut TextField, extend: bool, rtl_para: bool) {
    if rtl_para {
        move_left(field, extend);
    } else {
        move_right(field, extend);
    }
}

/// Collapse a non-empty selection when pressing an arrow without Shift.
pub fn collapse_selection_on_arrow(field: &mut TextField, key: &str, rtl_para: bool) {
    let start = field.selection().start();
    let end = field.selection().end();
    let at = match key {
        "ArrowLeft" if rtl_para => end,
        "ArrowRight" if rtl_para => start,
        "ArrowLeft" => start,
        "ArrowRight" => end,
        _ => field.focus(),
    };
    field.set_caret(at);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::keys::{apply_key, KeyAction, KeyEvent};

    fn key(key: &str, shift: bool) -> KeyEvent {
        KeyEvent {
            key: key.to_string(),
            shift,
            ctrl: false,
            meta: false,
            alt: false,
            is_mac: false,
        }
    }

    #[test]
    fn catches_rtl_paragraph_swaps_arrow_motion_not_ltr_step() {
        // Catches: RTL paragraph must move ArrowRight toward lower logical offsets (visual right).
        let mut f = TextField::new("مرحبا");
        assert!(paragraph_is_rtl(f.text()));
        f.set_caret(utf16_to_byte_approx(f.text(), 3));
        let before = f.focus();
        assert_eq!(
            apply_key(&mut f, &key("ArrowRight", false)),
            KeyAction::Handled
        );
        assert!(
            f.focus() < before,
            "ArrowRight should decrease logical offset in RTL"
        );
    }

    #[test]
    fn catches_ltr_ascii_skips_bidi_arrow_swap() {
        // Catches: pure LTR must keep logical ArrowRight (negative control for #57).
        let mut f = TextField::new("hello");
        assert!(!needs_bidi(f.text()));
        f.set_caret(2);
        apply_key(&mut f, &key("ArrowRight", false));
        assert_eq!(f.focus(), 3);
    }

    /// Helper: rough utf16 index for Arabic tests (all BMP in fixture strings).
    fn utf16_to_byte_approx(text: &str, u16: usize) -> usize {
        crate::text::selection::utf16_to_byte(text, u16)
    }
}
