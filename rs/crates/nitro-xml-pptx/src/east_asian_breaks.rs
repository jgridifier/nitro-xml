// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/text-east-asian-breaks.ts; changes in PROVENANCE.md.

/// East Asian line-break behaviour for one paragraph (`a:pPr` flags).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EastAsianBreakOptions {
    pub hanging_punctuation: bool,
    /// `a:pPr/@eaLnBrk="0"`: break between any two East Asian characters.
    pub break_anywhere: bool,
    pub latin_line_break: bool,
    pub keep_hangul_words: bool,
}

#[derive(Debug, Clone)]
pub struct EastAsianPiece {
    pub text: String,
    pub inline_block_zero_width: bool,
    pub word_spacing_px: Option<f64>,
    /// Hanging-punctuation glyph whose advance the following space piece carries.
    pub hang_glyph: Option<char>,
}

const WORD_JOINER: char = '\u{2060}';
const ZERO_WIDTH_SPACE: char = '\u{200B}';

static HANGING: &[char] = &[
    '\u{3001}', '\u{3002}', '\u{FF0C}', '\u{FF0E}', '\u{FF61}', '\u{FF64}',
];

fn is_hanging(ch: char) -> bool {
    HANGING.contains(&ch)
}

fn no_start(ch: char) -> bool {
    matches!(
        ch,
        '）' | '」'
            | '』'
            | '】'
            | '〕'
            | '〉'
            | '》'
            | '］'
            | '｝'
            | '〙'
            | '〗'
            | '〟'
            | '’'
            | '”'
    ) || is_hanging(ch)
}

pub fn resolve_east_asian_break_options(
    hanging_punct: Option<bool>,
    ea_ln_brk: Option<bool>,
    latin_ln_brk: Option<bool>,
) -> Option<EastAsianBreakOptions> {
    let hanging_punctuation = hanging_punct == Some(true);
    let break_anywhere = ea_ln_brk == Some(false);
    if !hanging_punctuation && !break_anywhere {
        return None;
    }
    Some(EastAsianBreakOptions {
        hanging_punctuation,
        break_anywhere,
        latin_line_break: latin_ln_brk == Some(true),
        keep_hangul_words: false,
    })
}

pub fn east_asian_breaks_for_run(
    options: Option<EastAsianBreakOptions>,
    language: Option<&str>,
) -> Option<EastAsianBreakOptions> {
    let opts = options?;
    if opts.latin_line_break || !is_korean_language(language) {
        return Some(opts);
    }
    Some(EastAsianBreakOptions {
        hanging_punctuation: opts.hanging_punctuation,
        break_anywhere: opts.break_anywhere,
        latin_line_break: false,
        keep_hangul_words: true,
    })
}

fn is_korean_language(language: Option<&str>) -> bool {
    let Some(lang) = language else {
        return false;
    };
    let lower = lang.to_ascii_lowercase();
    lower.starts_with("ko") && (lower.len() == 2 || lower.as_bytes().get(2) == Some(&b'-'))
}

pub fn is_east_asian_char(ch: char) -> bool {
    let cp = ch as u32;
    (0x1100..=0x11FF).contains(&cp)
        || (0x2E80..=0x9FFF).contains(&cp)
        || (0xA960..=0xA97F).contains(&cp)
        || (0xAC00..=0xD7FF).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0xFE30..=0xFE4F).contains(&cp)
        || (0xFF00..=0xFFEF).contains(&cp)
        || (0x20000..=0x3FFFF).contains(&cp)
}

fn is_hangul_char(ch: char) -> bool {
    let cp = ch as u32;
    (0x1100..=0x11FF).contains(&cp)
        || (0x3130..=0x318F).contains(&cp)
        || (0xA960..=0xA97F).contains(&cp)
        || (0xAC00..=0xD7FF).contains(&cp)
        || (0xFFA0..=0xFFDC).contains(&cp)
}

fn hang_space_word_spacing(ch: char, font_size_px: f64) -> f64 {
    let glyph = if ch >= '\u{FF61}' {
        font_size_px / 2.0
    } else {
        font_size_px
    };
    let space = font_size_px / 4.0;
    let ws = glyph - space;
    (ws * 1000.0).round() / 1000.0
}

fn breaks_between(ch: char, next: Option<char>, options: &EastAsianBreakOptions) -> bool {
    let breaks_around =
        |c: char| is_east_asian_char(c) && !(options.keep_hangul_words && is_hangul_char(c));
    options.break_anywhere
        && next.is_some()
        && (breaks_around(ch) || breaks_around(next.unwrap()))
        && !ch.is_whitespace()
        && !next.unwrap().is_whitespace()
        && !(options.hanging_punctuation && is_hanging(next.unwrap()))
}

/// Split run text for hanging punctuation / kinsoku-off breaks (upstream `splitEastAsianBreaks`).
pub fn split_east_asian_breaks(
    text: &str,
    options: Option<&EastAsianBreakOptions>,
    font_size_px: f64,
    following: Option<char>,
) -> Vec<EastAsianPiece> {
    if options.is_none() || text.is_empty() {
        return vec![EastAsianPiece {
            text: text.to_owned(),
            inline_block_zero_width: false,
            word_spacing_px: None,
            hang_glyph: None,
        }];
    }
    let options = options.unwrap();
    let chars: Vec<char> = text.chars().collect();
    let mut pieces: Vec<EastAsianPiece> = Vec::new();
    let mut current = String::new();

    let flush = |buf: &mut String, out: &mut Vec<EastAsianPiece>| {
        if !buf.is_empty() {
            out.push(EastAsianPiece {
                text: buf.clone(),
                inline_block_zero_width: false,
                word_spacing_px: None,
                hang_glyph: None,
            });
            buf.clear();
        }
    };

    let following_char = following;
    for i in 0..chars.len() {
        let ch = chars[i];
        let after = if i + 1 < chars.len() {
            Some(chars[i + 1])
        } else {
            following_char
        };
        let hangs = options.hanging_punctuation
            && is_hanging(ch)
            && (options.break_anywhere || after.is_none() || !no_start(after.unwrap()));
        if hangs {
            current.push(WORD_JOINER);
            flush(&mut current, &mut pieces);
            pieces.push(EastAsianPiece {
                text: ch.to_string(),
                inline_block_zero_width: true,
                word_spacing_px: None,
                hang_glyph: None,
            });
            pieces.push(EastAsianPiece {
                text: " ".to_owned(),
                inline_block_zero_width: false,
                word_spacing_px: Some(hang_space_word_spacing(ch, font_size_px)),
                hang_glyph: Some(ch),
            });
            continue;
        }
        current.push(ch);
        if breaks_between(ch, after, options) {
            current.push(ZERO_WIDTH_SPACE);
        }
    }
    flush(&mut current, &mut pieces);
    if pieces.is_empty() {
        vec![EastAsianPiece {
            text: text.to_owned(),
            inline_block_zero_width: false,
            word_spacing_px: None,
            hang_glyph: None,
        }]
    } else {
        pieces
    }
}

pub fn following_run_char(runs: &[crate::text_layout::TextPaintRun], index: usize) -> Option<char> {
    runs[(index + 1)..]
        .iter()
        .find_map(|run| run.text.trim_start().chars().next())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: kinsoku-on hanging `。」` wrapping as one unit instead of hanging the mark alone.
    #[test]
    fn hanging_mark_before_closing_bracket_wraps_together_under_kinsoku() {
        let opts = resolve_east_asian_break_options(Some(true), Some(true), None).unwrap();
        let pieces = split_east_asian_breaks("あい。」う", Some(&opts), 20.0, None);
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].text, "あい。」う");
    }

    // Catches: eaLnBrk off inserting break opportunities between East Asian chars.
    #[test]
    fn break_anywhere_inserts_zwsp_between_cjk() {
        let opts = resolve_east_asian_break_options(None, Some(false), None).unwrap();
        let pieces = split_east_asian_breaks("用」ッ。", Some(&opts), 20.0, None);
        assert!(pieces[0].text.contains(ZERO_WIDTH_SPACE));
    }
}
