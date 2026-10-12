// SPDX-License-Identifier: MIT
//! Caret motion and geometry for the single-line text surface (#55).
//!
//! Geometry uses a caller-supplied width callback so host measurement (#14
//! measure batch / Liberation metrics) can drive ±1 CSS px parity (#43).

use unicode_segmentation::UnicodeSegmentation;

use super::selection::{
    next_grapheme, next_word_boundary, prev_grapheme, prev_word_boundary, TextField,
};

/// Axis-aligned CSS-px rectangle (origin top-left of the field content box).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaretRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Width of a UTF-8 substring in CSS px (left-to-right single line).
pub type WidthFn<'a> = dyn Fn(&str) -> f32 + 'a;

pub fn move_left(field: &mut TextField, extend: bool) {
    let at = field.focus();
    let Some(prev) = prev_grapheme(field.text(), at) else {
        if !extend {
            field.set_caret(at);
        }
        return;
    };
    if extend {
        field.extend_to(prev);
    } else {
        field.set_caret(prev);
    }
}

pub fn move_right(field: &mut TextField, extend: bool) {
    let at = field.focus();
    let Some(next) = next_grapheme(field.text(), at) else {
        if !extend {
            field.set_caret(at);
        }
        return;
    };
    if extend {
        field.extend_to(next);
    } else {
        field.set_caret(next);
    }
}

pub fn move_word_left(field: &mut TextField, extend: bool) {
    let at = prev_word_boundary(field.text(), field.focus());
    if extend {
        field.extend_to(at);
    } else {
        field.set_caret(at);
    }
}

pub fn move_word_right(field: &mut TextField, extend: bool) {
    let at = next_word_boundary(field.text(), field.focus());
    if extend {
        field.extend_to(at);
    } else {
        field.set_caret(at);
    }
}

pub fn move_line_start(field: &mut TextField, extend: bool) {
    // Single-line surface: Home → buffer start (Alt+Enter newlines are rare soft breaks).
    let at = line_start(field.text(), field.focus());
    if extend {
        field.extend_to(at);
    } else {
        field.set_caret(at);
    }
}

pub fn move_line_end(field: &mut TextField, extend: bool) {
    let at = line_end(field.text(), field.focus());
    if extend {
        field.extend_to(at);
    } else {
        field.set_caret(at);
    }
}

fn line_start(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    text[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

fn line_end(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    text[byte..]
        .find('\n')
        .map(|i| byte + i)
        .unwrap_or(text.len())
}

/// X position of the caret (left edge) at `byte` on a single line.
pub fn x_at_offset(text: &str, byte: usize, width_of: &WidthFn<'_>) -> f32 {
    let byte = byte.min(text.len());
    let line_begin = line_start(text, byte);
    width_of(&text[line_begin..byte])
}

/// Byte offset nearest to content-x `x` on the line containing `hint_byte` (or 0).
pub fn offset_at_x(text: &str, x: f32, width_of: &WidthFn<'_>) -> usize {
    offset_at_x_on_line(text, 0, x, width_of)
}

pub fn offset_at_x_on_line(text: &str, line_begin: usize, x: f32, width_of: &WidthFn<'_>) -> usize {
    let line_begin = line_begin.min(text.len());
    let line_stop = line_end(text, line_begin);
    let line = &text[line_begin..line_stop];
    if x <= 0.0 {
        return line_begin;
    }
    let mut best = line_begin;
    let mut best_dist = x.abs();
    // Boundary at line start.
    for (i, g) in line.grapheme_indices(true) {
        let before = line_begin + i;
        let bx = width_of(&text[line_begin..before]);
        let dist_b = (bx - x).abs();
        if dist_b < best_dist {
            best = before;
            best_dist = dist_b;
        }
        let abs = before + g.len();
        let gx = width_of(&text[line_begin..abs]);
        let dist = (gx - x).abs();
        if dist < best_dist {
            best = abs;
            best_dist = dist;
        }
    }
    best
}

pub fn caret_rect(
    text: &str,
    byte: usize,
    y: f32,
    line_height: f32,
    width_of: &WidthFn<'_>,
) -> CaretRect {
    CaretRect {
        x: x_at_offset(text, byte, width_of),
        y,
        w: 1.0,
        h: line_height,
    }
}

/// Selection highlight rects (one per soft line intersecting the range).
pub fn selection_rects(
    text: &str,
    anchor: usize,
    focus: usize,
    y: f32,
    line_height: f32,
    width_of: &WidthFn<'_>,
) -> Vec<CaretRect> {
    let start = anchor.min(focus).min(text.len());
    let end = anchor.max(focus).min(text.len());
    if start == end {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut line_y = y;
    let mut line_begin = 0usize;
    while line_begin <= text.len() {
        let line_stop = line_end(text, line_begin);
        let seg_start = start.max(line_begin);
        let seg_end = end.min(line_stop);
        if seg_start < seg_end {
            let x0 = width_of(&text[line_begin..seg_start]);
            let x1 = width_of(&text[line_begin..seg_end]);
            out.push(CaretRect {
                x: x0,
                y: line_y,
                w: (x1 - x0).max(0.0),
                h: line_height,
            });
        }
        if line_stop >= text.len() {
            break;
        }
        line_begin = line_stop + 1; // skip '\n'
        line_y += line_height;
        if line_begin > text.len() {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::selection::TextField;

    fn mono(s: &str) -> f32 {
        s.chars().count() as f32 * 8.0
    }

    #[test]
    fn catches_hit_test_picks_nearest_grapheme_boundary() {
        // Catches: click x must resolve to nearest grapheme boundary, not char mid-point drift > 1px.
        let text = "abc";
        let off = offset_at_x(text, 12.0, &mono); // between b(8-16) center ~12 → after 'b' or before
        assert!(off == 1 || off == 2, "off={off}");
        let caret = caret_rect(text, off, 0.0, 16.0, &mono);
        assert!((caret.x - 8.0).abs() <= 1.0 || (caret.x - 16.0).abs() <= 1.0);
    }

    #[test]
    fn catches_home_end_are_line_not_buffer_with_newline() {
        // Catches: Home/End must stop at the current soft line when Alt+Enter inserted '\n'.
        let mut f = TextField::new("ab\ncd");
        f.set_caret(f.text().len());
        move_line_start(&mut f, false);
        assert_eq!(f.focus(), 3);
        move_line_end(&mut f, false);
        assert_eq!(f.focus(), 5);
    }
}
