// SPDX-License-Identifier: MIT
//! Text buffer + selection for the single-line editor surface (#55).
//!
//! Indices are UTF-8 byte offsets clamped to grapheme boundaries. The JS
//! harness compares UTF-16 code-unit indices (textarea `selectionStart/End`).

use unicode_segmentation::UnicodeSegmentation;

/// Inclusive-exclusive selection over a text buffer (`anchor` may be > `focus`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextSelection {
    pub anchor: usize,
    pub focus: usize,
}

impl TextSelection {
    pub fn collapsed(at: usize) -> Self {
        Self {
            anchor: at,
            focus: at,
        }
    }

    pub fn start(&self) -> usize {
        self.anchor.min(self.focus)
    }

    pub fn end(&self) -> usize {
        self.anchor.max(self.focus)
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.focus
    }
}

/// Single-line (soft-wrap capable) editable text with an anchored selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextField {
    text: String,
    sel: TextSelection,
}

impl TextField {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let end = text.len();
        Self {
            text,
            sel: TextSelection::collapsed(end),
        }
    }

    pub fn from_utf16_selection(text: &str, anchor_u16: usize, focus_u16: usize) -> Self {
        let mut f = Self::new(text.to_string());
        f.sel.anchor = utf16_to_byte(text, anchor_u16);
        f.sel.focus = utf16_to_byte(text, focus_u16);
        f.clamp_selection();
        f
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn selection(&self) -> &TextSelection {
        &self.sel
    }

    pub fn anchor(&self) -> usize {
        self.sel.anchor
    }

    pub fn focus(&self) -> usize {
        self.sel.focus
    }

    pub fn utf16_selection(&self) -> (usize, usize) {
        (
            byte_to_utf16(&self.text, self.sel.anchor),
            byte_to_utf16(&self.text, self.sel.focus),
        )
    }

    pub fn selected_text(&self) -> &str {
        &self.text[self.sel.start()..self.sel.end()]
    }

    pub fn set_text(&mut self, text: impl Into<String>, caret_byte: Option<usize>) {
        self.text = text.into();
        let at = caret_byte.unwrap_or(self.text.len()).min(self.text.len());
        let at = snap_grapheme(&self.text, at);
        self.sel = TextSelection::collapsed(at);
    }

    pub fn set_caret(&mut self, byte: usize) {
        let at = snap_grapheme(&self.text, byte.min(self.text.len()));
        self.sel = TextSelection::collapsed(at);
    }

    pub fn extend_to(&mut self, byte: usize) {
        let at = snap_grapheme(&self.text, byte.min(self.text.len()));
        self.sel.focus = at;
    }

    pub(crate) fn set_selection_range(&mut self, anchor: usize, focus: usize) {
        self.sel.anchor = snap_grapheme(&self.text, anchor.min(self.text.len()));
        self.sel.focus = snap_grapheme(&self.text, focus.min(self.text.len()));
        self.clamp_selection();
    }

    pub fn select_all(&mut self) {
        self.sel.anchor = 0;
        self.sel.focus = self.text.len();
    }

    /// Replace the selection with `insert` and collapse the caret after it.
    pub fn insert(&mut self, insert: &str) {
        let start = self.sel.start();
        let end = self.sel.end();
        let next = splice(&self.text, start, end, insert);
        self.text = next.text;
        self.sel = TextSelection::collapsed(next.caret);
    }

    pub fn delete_backward(&mut self) {
        if !self.sel.is_collapsed() {
            self.insert("");
            return;
        }
        if let Some(prev) = prev_grapheme(&self.text, self.sel.focus) {
            self.sel.anchor = prev;
            self.insert("");
        }
    }

    pub fn delete_forward(&mut self) {
        if !self.sel.is_collapsed() {
            self.insert("");
            return;
        }
        if let Some(next) = next_grapheme(&self.text, self.sel.focus) {
            self.sel.anchor = self.sel.focus;
            self.sel.focus = next;
            self.insert("");
        }
    }

    pub fn delete_word_backward(&mut self) {
        if !self.sel.is_collapsed() {
            self.insert("");
            return;
        }
        let prev = prev_word_boundary(&self.text, self.sel.focus);
        if prev < self.sel.focus {
            self.sel.anchor = prev;
            self.insert("");
        }
    }

    pub fn delete_word_forward(&mut self) {
        if !self.sel.is_collapsed() {
            self.insert("");
            return;
        }
        let at = self.sel.focus;
        let next = next_word_boundary(&self.text, at);
        if next > at {
            self.sel.anchor = at;
            self.sel.focus = next;
            self.insert("");
        }
    }

    fn clamp_selection(&mut self) {
        let len = self.text.len();
        self.sel.anchor = snap_grapheme(&self.text, self.sel.anchor.min(len));
        self.sel.focus = snap_grapheme(&self.text, self.sel.focus.min(len));
    }
}

/// Upstream `splice` from `editor-text.ts` (byte offsets; same string algebra).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpliceResult {
    pub text: String,
    pub caret: usize,
}

pub fn splice(text: &str, start: usize, end: usize, insert: &str) -> SpliceResult {
    let start = start.min(text.len());
    let end = end.min(text.len()).max(start);
    let mut out = String::with_capacity(text.len() - (end - start) + insert.len());
    out.push_str(&text[..start]);
    out.push_str(insert);
    out.push_str(&text[end..]);
    SpliceResult {
        caret: start + insert.len(),
        text: out,
    }
}

pub(crate) fn snap_grapheme(text: &str, byte: usize) -> usize {
    if byte >= text.len() {
        return text.len();
    }
    if text.is_char_boundary(byte) {
        // If we're mid-grapheme (e.g. after first codepoint of a cluster), snap back.
        for (i, g) in text.grapheme_indices(true) {
            let end = i + g.len();
            if byte <= i {
                return i;
            }
            if byte < end {
                return i;
            }
            if byte == end {
                return end;
            }
        }
        return text.len();
    }
    // Not on a char boundary: snap to previous char boundary then grapheme.
    let mut b = byte;
    while b > 0 && !text.is_char_boundary(b) {
        b -= 1;
    }
    snap_grapheme(text, b)
}

pub(crate) fn next_grapheme(text: &str, byte: usize) -> Option<usize> {
    let byte = snap_grapheme(text, byte);
    for (i, g) in text.grapheme_indices(true) {
        if i >= byte {
            return Some(i + g.len());
        }
    }
    None
}

pub(crate) fn prev_grapheme(text: &str, byte: usize) -> Option<usize> {
    let byte = snap_grapheme(text, byte);
    let mut prev = None;
    for (i, _) in text.grapheme_indices(true) {
        if i >= byte {
            return prev;
        }
        prev = Some(i);
    }
    prev
}

/// Excel/textarea-like word boundary: skip whitespace, then a run of word chars
/// (alphanumeric / underscore / most letters) or a single non-word glyph.
pub(crate) fn next_word_boundary(text: &str, byte: usize) -> usize {
    let byte = snap_grapheme(text, byte.min(text.len()));
    let rest = &text[byte..];
    let mut chars = rest.char_indices();
    let Some((mut i, mut ch)) = chars.next() else {
        return text.len();
    };
    // Skip whitespace
    while ch.is_whitespace() {
        match chars.next() {
            Some((ni, nc)) => {
                i = ni;
                ch = nc;
            }
            None => return text.len(),
        }
    }
    if is_word_char(ch) {
        let mut end = i + ch.len_utf8();
        for (ni, nc) in chars {
            if !is_word_char(nc) {
                return byte + ni;
            }
            end = ni + nc.len_utf8();
        }
        byte + end
    } else {
        // Single non-word grapheme/char
        next_grapheme(text, byte + i).unwrap_or(text.len())
    }
}

pub(crate) fn prev_word_boundary(text: &str, byte: usize) -> usize {
    let byte = snap_grapheme(text, byte.min(text.len()));
    if byte == 0 {
        return 0;
    }
    let prefix = &text[..byte];
    // Walk back over whitespace
    let mut end = byte;
    for (i, ch) in prefix.char_indices().rev() {
        if ch.is_whitespace() {
            end = i;
        } else {
            break;
        }
    }
    if end == 0 {
        return 0;
    }
    let prefix = &text[..end];
    let Some((last_i, last_ch)) = prefix.char_indices().next_back() else {
        return 0;
    };
    if is_word_char(last_ch) {
        let mut start = last_i;
        for (i, ch) in prefix.char_indices().rev() {
            if !is_word_char(ch) {
                break;
            }
            start = i;
        }
        start
    } else {
        prev_grapheme(text, end).unwrap_or(0)
    }
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

pub(crate) fn byte_to_utf16(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    let mut u16_len = 0;
    for (i, ch) in text.char_indices() {
        if i >= byte {
            break;
        }
        u16_len += ch.len_utf16();
    }
    u16_len
}

pub(crate) fn utf16_to_byte(text: &str, u16_index: usize) -> usize {
    let mut u16_at = 0;
    for (i, ch) in text.char_indices() {
        if u16_at >= u16_index {
            return i;
        }
        u16_at += ch.len_utf16();
        if u16_at > u16_index {
            // Landed inside a surrogate pair: snap to this char start.
            return i;
        }
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_zwj_emoji_grapheme_not_split_by_arrow() {
        // Catches: ArrowLeft must move one grapheme over ZWJ emoji, not one codepoint.
        let family = "👨‍👩‍👧‍👦";
        let mut f = TextField::new(format!("a{family}b"));
        f.set_caret(f.text().len());
        let prev = prev_grapheme(f.text(), f.focus()).unwrap();
        f.set_caret(prev); // before 'b'
        let prev2 = prev_grapheme(f.text(), f.focus()).unwrap();
        f.set_caret(prev2);
        assert_eq!(prev2, "a".len());
        assert_eq!(&f.text()[prev2..prev], family);
    }

    #[test]
    fn catches_shift_selection_keeps_anchor() {
        // Catches: Shift+Right must keep anchor and only move focus.
        let mut f = TextField::new("abcd");
        f.set_caret(1);
        let n = next_grapheme(f.text(), f.focus()).unwrap();
        f.extend_to(n);
        f.extend_to(next_grapheme(f.text(), f.focus()).unwrap());
        assert_eq!(f.anchor(), 1);
        assert_eq!(f.focus(), 3);
        assert_eq!(f.selected_text(), "bc");
    }

    #[test]
    fn catches_splice_replaces_range_like_upstream() {
        // Catches: splice must match editor-text.ts (replace [start,end) and caret after insert).
        let r = splice("hello", 1, 4, "i");
        assert_eq!(r.text, "hio");
        assert_eq!(r.caret, 2);
    }
}
