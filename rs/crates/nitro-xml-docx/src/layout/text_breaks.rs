// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/text-breaks.ts; changes listed in PROVENANCE.md.

use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BreakToken {
    Word {
        text: String,
        run_index: usize,
        source_start: usize,
    },
    Space {
        text: String,
        run_index: usize,
        source_start: usize,
    },
    Tab {
        run_index: usize,
        source_start: usize,
    },
    Object {
        run_index: usize,
        source_start: usize,
    },
    LineBreak {
        run_index: usize,
        source_start: usize,
    },
    PageBreak {
        run_index: usize,
        source_start: usize,
    },
    ColumnBreak {
        run_index: usize,
        source_start: usize,
    },
}

pub fn is_east_asian_char(ch: char) -> bool {
    let cp = ch as u32;
    (0x1100..=0x11ff).contains(&cp)
        || (0x2e80..=0x9fff).contains(&cp)
        || (0xa960..=0xa97f).contains(&cp)
        || (0xac00..=0xd7ff).contains(&cp)
        || (0xf900..=0xfaff).contains(&cp)
        || (0xfe30..=0xfe4f).contains(&cp)
        || (0xff00..=0xffef).contains(&cp)
        || (0x20000..=0x3ffff).contains(&cp)
}

pub fn tokenize_run(text: &str, run_index: usize) -> Vec<BreakToken> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut word_start = 0usize;
    let flush_word = |tokens: &mut Vec<BreakToken>, word: &mut String, word_start: usize| {
        if !word.is_empty() {
            tokens.push(BreakToken::Word {
                text: word.clone(),
                run_index,
                source_start: word_start,
            });
            word.clear();
        }
    };
    let mut offset = 0usize;
    for ch in text.graphemes(true) {
        if ch == "\n" {
            flush_word(&mut tokens, &mut word, word_start);
            tokens.push(BreakToken::LineBreak {
                run_index,
                source_start: offset,
            });
        } else if ch == "\t" {
            flush_word(&mut tokens, &mut word, word_start);
            tokens.push(BreakToken::Tab {
                run_index,
                source_start: offset,
            });
        } else if ch == " " {
            flush_word(&mut tokens, &mut word, word_start);
            tokens.push(BreakToken::Space {
                text: ch.to_string(),
                run_index,
                source_start: offset,
            });
        } else if ch.chars().next().is_some_and(is_east_asian_char) {
            flush_word(&mut tokens, &mut word, word_start);
            tokens.push(BreakToken::Word {
                text: ch.to_string(),
                run_index,
                source_start: offset,
            });
        } else {
            if word.is_empty() {
                word_start = offset;
            }
            word.push_str(ch);
            if ch == "-" {
                flush_word(&mut tokens, &mut word, word_start);
            }
        }
        // Upstream uses JS string `.length` (UTF-16 code units), not UTF-8 bytes.
        offset += ch.encode_utf16().count();
    }
    flush_word(&mut tokens, &mut word, word_start);
    tokens
}

/// JS `String.length` / UTF-16 code-unit count (parity with upstream source ranges).
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

pub fn append_break_marker(
    tokens: &mut Vec<BreakToken>,
    run_index: usize,
    source_start: usize,
    kind: &str,
) {
    tokens.push(if kind == "page" {
        BreakToken::PageBreak {
            run_index,
            source_start,
        }
    } else {
        BreakToken::ColumnBreak {
            run_index,
            source_start,
        }
    });
}
