// SPDX-License-Identifier: MIT
//! Keyboard mapping for the rich multi-paragraph surface (#61).
//!
//! TODO(#56): composition keys · TODO(#57): bidi Home/End.

use super::caret::{
    move_left, move_para_end, move_para_start, move_right, move_word_left, move_word_right,
};
use super::insert::{delete_selection, insert_text, split_paragraph};
use super::model::RichDocument;
use super::position::{clamp_pos, paragraph_at};
use super::selection::RichSelection;
use crate::text::keys::KeyEvent;
use crate::text::selection::{next_grapheme, prev_grapheme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RichKeyAction {
    Handled,
    Unhandled,
    Copy,
    Cut,
    Paste,
}

pub fn apply_rich_key(
    doc: &mut RichDocument,
    sel: &mut RichSelection,
    ev: &KeyEvent,
) -> RichKeyAction {
    let mod_key = if ev.is_mac { ev.meta } else { ev.ctrl };
    let shift = ev.shift;
    let key = ev.key.as_str();

    if mod_key && !ev.alt {
        match key {
            "a" | "A" => {
                // Select all paragraphs in document.
                let paths = super::position::iter_para_paths(doc);
                if let (Some(first), Some(last)) = (paths.first(), paths.last()) {
                    let end_off = paragraph_at(doc, last)
                        .map(|p| p.plain_text().len())
                        .unwrap_or(0);
                    sel.anchor = super::position::RichPos {
                        path: first.clone(),
                        offset: 0,
                    };
                    sel.focus = super::position::RichPos {
                        path: last.clone(),
                        offset: end_off,
                    };
                }
                return RichKeyAction::Handled;
            }
            "c" | "C" => return RichKeyAction::Copy,
            "x" | "X" => return RichKeyAction::Cut,
            "v" | "V" => return RichKeyAction::Paste,
            "b" | "B" => {
                // Style toggle deferred to host/commands — mark handled for chord ownership.
                return RichKeyAction::Handled;
            }
            _ => {}
        }
    }

    match key {
        "ArrowLeft" => {
            if mod_key {
                move_word_left(doc, sel, shift);
            } else if !shift && !sel.is_collapsed() {
                *sel = RichSelection::collapsed(sel.start().clone());
            } else {
                move_left(doc, sel, shift);
            }
            RichKeyAction::Handled
        }
        "ArrowRight" => {
            if mod_key {
                move_word_right(doc, sel, shift);
            } else if !shift && !sel.is_collapsed() {
                *sel = RichSelection::collapsed(sel.end().clone());
            } else {
                move_right(doc, sel, shift);
            }
            RichKeyAction::Handled
        }
        "Home" => {
            move_para_start(doc, sel, shift);
            RichKeyAction::Handled
        }
        "End" => {
            move_para_end(doc, sel, shift);
            RichKeyAction::Handled
        }
        "Backspace" => {
            if sel.is_collapsed() {
                let at = clamp_pos(doc, &sel.focus);
                if let Some(p) = paragraph_at(doc, &at.path) {
                    let text = p.plain_text();
                    if at.offset == 0 {
                        // Join with previous paragraph.
                        move_left(doc, sel, true);
                        if !sel.is_collapsed() {
                            // Actually merge: delete the paragraph break by selecting back and deleting.
                            // Simpler: move left then delete forward across boundary via delete_selection
                            // after extending — already extended by move_left extend.
                        }
                    } else if let Some(prev) = prev_grapheme(&text, at.offset) {
                        sel.anchor = super::position::RichPos {
                            path: at.path.clone(),
                            offset: prev,
                        };
                        sel.focus = at;
                    }
                }
            }
            delete_selection(doc, sel);
            RichKeyAction::Handled
        }
        "Delete" => {
            if sel.is_collapsed() {
                let at = clamp_pos(doc, &sel.focus);
                if let Some(p) = paragraph_at(doc, &at.path) {
                    let text = p.plain_text();
                    if let Some(next) = next_grapheme(&text, at.offset) {
                        sel.focus = super::position::RichPos {
                            path: at.path.clone(),
                            offset: next,
                        };
                        sel.anchor = at;
                    } else {
                        move_right(doc, sel, true);
                    }
                }
            }
            delete_selection(doc, sel);
            RichKeyAction::Handled
        }
        "Enter" if !ev.shift => {
            split_paragraph(doc, sel);
            RichKeyAction::Handled
        }
        _ => RichKeyAction::Unhandled,
    }
}

/// Insert printable text (host feeds Input events; #56 owns composition).
pub fn insert_printable(doc: &mut RichDocument, sel: &mut RichSelection, text: &str) {
    insert_text(doc, sel, text);
}
