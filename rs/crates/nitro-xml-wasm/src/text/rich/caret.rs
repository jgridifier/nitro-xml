// SPDX-License-Identifier: MIT
//! Caret motion across runs, paragraphs and table cells (#61).
//!
//! TODO(#57): bidi-aware caret · TODO(#56): IME composition caret.

use super::model::RichDocument;
use super::position::{clamp_pos, next_para_path, paragraph_at, prev_para_path, RichPos};
use super::selection::RichSelection;
use crate::text::selection::{
    next_grapheme, next_word_boundary, prev_grapheme, prev_word_boundary,
};

pub fn move_left(doc: &RichDocument, sel: &mut RichSelection, extend: bool) {
    let at = clamp_pos(doc, &sel.focus);
    let Some(p) = paragraph_at(doc, &at.path) else {
        return;
    };
    let text = p.plain_text();
    let next_focus = if let Some(prev) = prev_grapheme(&text, at.offset) {
        RichPos {
            path: at.path,
            offset: prev,
        }
    } else if let Some(prev_path) = prev_para_path(doc, &at.path) {
        let prev_p = paragraph_at(doc, &prev_path).unwrap();
        let t = prev_p.plain_text();
        RichPos {
            path: prev_path,
            offset: t.len(),
        }
    } else {
        at
    };
    if extend {
        sel.focus = next_focus;
    } else {
        *sel = RichSelection::collapsed(next_focus);
    }
}

pub fn move_right(doc: &RichDocument, sel: &mut RichSelection, extend: bool) {
    let at = clamp_pos(doc, &sel.focus);
    let Some(p) = paragraph_at(doc, &at.path) else {
        return;
    };
    let text = p.plain_text();
    let next_focus = if let Some(next) = next_grapheme(&text, at.offset) {
        RichPos {
            path: at.path,
            offset: next,
        }
    } else if let Some(next_path) = next_para_path(doc, &at.path) {
        RichPos {
            path: next_path,
            offset: 0,
        }
    } else {
        at
    };
    if extend {
        sel.focus = next_focus;
    } else {
        *sel = RichSelection::collapsed(next_focus);
    }
}

pub fn move_word_left(doc: &RichDocument, sel: &mut RichSelection, extend: bool) {
    let at = clamp_pos(doc, &sel.focus);
    let Some(p) = paragraph_at(doc, &at.path) else {
        return;
    };
    let text = p.plain_text();
    let off = prev_word_boundary(&text, at.offset);
    let next_focus = if off < at.offset {
        RichPos {
            path: at.path,
            offset: off,
        }
    } else if let Some(prev_path) = prev_para_path(doc, &at.path) {
        let prev_p = paragraph_at(doc, &prev_path).unwrap();
        let t = prev_p.plain_text();
        RichPos {
            path: prev_path,
            offset: t.len(),
        }
    } else {
        at
    };
    if extend {
        sel.focus = next_focus;
    } else {
        *sel = RichSelection::collapsed(next_focus);
    }
}

pub fn move_word_right(doc: &RichDocument, sel: &mut RichSelection, extend: bool) {
    let at = clamp_pos(doc, &sel.focus);
    let Some(p) = paragraph_at(doc, &at.path) else {
        return;
    };
    let text = p.plain_text();
    let off = next_word_boundary(&text, at.offset);
    let next_focus = if off > at.offset {
        RichPos {
            path: at.path,
            offset: off,
        }
    } else if let Some(next_path) = next_para_path(doc, &at.path) {
        RichPos {
            path: next_path,
            offset: 0,
        }
    } else {
        at
    };
    if extend {
        sel.focus = next_focus;
    } else {
        *sel = RichSelection::collapsed(next_focus);
    }
}

pub fn move_para_start(doc: &RichDocument, sel: &mut RichSelection, extend: bool) {
    let at = clamp_pos(doc, &sel.focus);
    let next = RichPos {
        path: at.path,
        offset: 0,
    };
    if extend {
        sel.focus = next;
    } else {
        *sel = RichSelection::collapsed(next);
    }
}

pub fn move_para_end(doc: &RichDocument, sel: &mut RichSelection, extend: bool) {
    let at = clamp_pos(doc, &sel.focus);
    let Some(p) = paragraph_at(doc, &at.path) else {
        return;
    };
    let next = RichPos {
        path: at.path,
        offset: p.plain_text().len(),
    };
    if extend {
        sel.focus = next;
    } else {
        *sel = RichSelection::collapsed(next);
    }
}

/// Synthetic caret rect: y advances one line_height per paragraph index in document order.
pub fn caret_rect(
    doc: &RichDocument,
    pos: &RichPos,
    char_w: f32,
    line_h: f32,
) -> crate::text::caret::CaretRect {
    let paths = super::position::iter_para_paths(doc);
    let line = paths.iter().position(|p| p == &pos.path).unwrap_or(0) as f32;
    let x = paragraph_at(doc, &pos.path)
        .map(|p| {
            let text = p.plain_text();
            let prefix = &text[..pos.offset.min(text.len())];
            prefix.chars().count() as f32 * char_w
        })
        .unwrap_or(0.0);
    crate::text::caret::CaretRect {
        x,
        y: line * line_h,
        w: 1.0,
        h: line_h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::rich::model::RichDocument;

    #[test]
    fn catches_arrow_right_crosses_paragraph_boundary() {
        // Catches: ArrowRight at end of ¶0 must land at start of ¶1, not stick.
        let doc = RichDocument::from_paragraphs(&["ab", "cd"]);
        let mut sel = RichSelection::collapsed(RichPos {
            path: super::super::position::ParaPath::Block { block: 0 },
            offset: 2,
        });
        move_right(&doc, &mut sel, false);
        assert_eq!(
            sel.focus.path,
            super::super::position::ParaPath::Block { block: 1 }
        );
        assert_eq!(sel.focus.offset, 0);
    }
}
