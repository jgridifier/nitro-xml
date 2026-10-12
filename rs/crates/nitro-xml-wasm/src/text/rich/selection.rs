// SPDX-License-Identifier: MIT
//! Selection over a [`RichDocument`] (#61).

use super::model::RichDocument;
use super::position::{clamp_pos, cmp_pos, paragraph_at, ParaPath, RichPos};

/// Anchored selection (anchor may be after focus when selecting backwards).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichSelection {
    pub anchor: RichPos,
    pub focus: RichPos,
}

impl RichSelection {
    pub fn collapsed(at: RichPos) -> Self {
        Self {
            anchor: at.clone(),
            focus: at,
        }
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.focus
    }

    pub fn start(&self) -> &RichPos {
        if cmp_pos(&self.anchor, &self.focus) != std::cmp::Ordering::Greater {
            &self.anchor
        } else {
            &self.focus
        }
    }

    pub fn end(&self) -> &RichPos {
        if cmp_pos(&self.anchor, &self.focus) != std::cmp::Ordering::Greater {
            &self.focus
        } else {
            &self.anchor
        }
    }
}

/// Plain text covered by the selection (paragraphs joined with `\n`).
pub fn selected_plain(doc: &RichDocument, sel: &RichSelection) -> String {
    let start = sel.start();
    let end = sel.end();
    if start.path == end.path {
        let Some(p) = paragraph_at(doc, &start.path) else {
            return String::new();
        };
        let text = p.plain_text();
        let a = start.offset.min(text.len());
        let b = end.offset.min(text.len()).max(a);
        return text[a..b].to_string();
    }
    // Multi-paragraph: take suffix of start, full middle paras, prefix of end.
    let paths = super::position::iter_para_paths(doc);
    let mut out = String::new();
    let mut started = false;
    for path in paths {
        if path == start.path {
            started = true;
            if let Some(p) = paragraph_at(doc, &path) {
                let text = p.plain_text();
                out.push_str(&text[start.offset.min(text.len())..]);
            }
            continue;
        }
        if !started {
            continue;
        }
        if path == end.path {
            if let Some(p) = paragraph_at(doc, &path) {
                let text = p.plain_text();
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&text[..end.offset.min(text.len())]);
            }
            break;
        }
        if let Some(p) = paragraph_at(doc, &path) {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&p.plain_text());
        }
    }
    out
}

pub fn clamp_selection(doc: &RichDocument, sel: &RichSelection) -> RichSelection {
    RichSelection {
        anchor: clamp_pos(doc, &sel.anchor),
        focus: clamp_pos(doc, &sel.focus),
    }
}

/// True when selection spans more than one paragraph path.
pub fn is_cross_paragraph(sel: &RichSelection) -> bool {
    match (&sel.anchor.path, &sel.focus.path) {
        (ParaPath::Block { block: a }, ParaPath::Block { block: b }) => a != b,
        (a, b) => a != b,
    }
}
