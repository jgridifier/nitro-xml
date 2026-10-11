// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/text/wrap-styled-runs.ts

//! Format-neutral horizontal flow, retaining run properties and explicit empty lines.
//! A word can span formatting runs; automatic line edges discard separating spaces.
//! Nonbreaking spaces remain inside words. The host owns font measurement and units.

use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, Default)]
pub struct WrapStyledRunsOptions {
    /// Break an oversized word at grapheme boundaries. Otherwise it overflows intact.
    pub break_words: bool,
}

/// Runs that carry replaceable text for wrap output.
pub trait StyledRun: Clone {
    fn text(&self) -> &str;
    fn with_text(&self, text: &str) -> Self;
}

#[derive(Clone, Debug)]
struct Piece {
    source: usize,
    text: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Word,
    Space,
    Break,
}

struct Group {
    kind: Kind,
    pieces: Vec<Piece>,
}

/// Tokenize like upstream `split(/(\r\n|[\r\n]|[ \t]+)/u).filter(Boolean)`.
fn tokens(text: &str) -> Vec<(Kind, String)> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let ch = text[i..].chars().next().unwrap();
        let ch_len = ch.len_utf8();
        if ch == '\r' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                out.push((Kind::Break, "\r\n".to_string()));
                i += 2;
            } else {
                out.push((Kind::Break, "\r".to_string()));
                i += 1;
            }
            continue;
        }
        if ch == '\n' {
            out.push((Kind::Break, "\n".to_string()));
            i += 1;
            continue;
        }
        if ch == ' ' || ch == '\t' {
            let start = i;
            i += ch_len;
            while i < bytes.len() {
                let c = text[i..].chars().next().unwrap();
                if c == ' ' || c == '\t' {
                    i += c.len_utf8();
                } else {
                    break;
                }
            }
            out.push((Kind::Space, text[start..i].to_string()));
            continue;
        }
        let start = i;
        i += ch_len;
        while i < bytes.len() {
            let c = text[i..].chars().next().unwrap();
            if c == '\r' || c == '\n' || c == ' ' || c == '\t' {
                break;
            }
            i += c.len_utf8();
        }
        out.push((Kind::Word, text[start..i].to_string()));
    }
    out
}

fn merge(left: &[Piece], right: &[Piece]) -> Vec<Piece> {
    let mut result: Vec<Piece> = left.to_vec();
    for piece in right {
        if let Some(last) = result.last_mut() {
            if last.source == piece.source {
                last.text.push_str(&piece.text);
                continue;
            }
        }
        result.push(piece.clone());
    }
    result
}

fn width_of<T, F>(pieces: &[Piece], runs: &[T], measure: &mut F) -> f64
where
    F: FnMut(&str, &T) -> f64,
{
    pieces.iter().fold(0.0, |sum, piece| {
        let advance = measure(&piece.text, &runs[piece.source]);
        sum + if advance.is_finite() && advance >= 0.0 {
            advance
        } else {
            0.0
        }
    })
}

/// Upstream `wrapStyledRuns`.
pub fn wrap_styled_runs<T, F>(
    runs: &[T],
    max_width: f64,
    measure: &mut F,
    options: WrapStyledRunsOptions,
) -> Vec<Vec<T>>
where
    T: StyledRun,
    F: FnMut(&str, &T) -> f64,
{
    let mut groups: Vec<Group> = Vec::new();
    for (source, run) in runs.iter().enumerate() {
        for (kind, token) in tokens(run.text()) {
            let need_new = match groups.last() {
                None => true,
                Some(g) if g.kind != kind || kind == Kind::Break => true,
                _ => false,
            };
            if need_new {
                groups.push(Group {
                    kind,
                    pieces: Vec::new(),
                });
            }
            if kind != Kind::Break {
                groups.last_mut().unwrap().pieces.push(Piece {
                    source,
                    text: token,
                });
            }
        }
    }

    let mut result: Vec<Vec<T>> = Vec::new();
    let mut line: Vec<Piece> = Vec::new();
    let mut spaces: Vec<Piece> = Vec::new();

    let emit = |line: &mut Vec<Piece>, spaces: &mut Vec<Piece>, result: &mut Vec<Vec<T>>| {
        result.push(
            line.iter()
                .map(|piece| runs[piece.source].with_text(&piece.text))
                .collect(),
        );
        line.clear();
        spaces.clear();
    };

    for group in groups {
        if group.kind == Kind::Break {
            emit(&mut line, &mut spaces, &mut result);
            continue;
        }
        if group.kind == Kind::Space {
            spaces = group.pieces;
            continue;
        }
        let separator = if line.is_empty() {
            Vec::new()
        } else {
            spaces.clone()
        };
        let word_width = width_of(&group.pieces, runs, measure);
        if !line.is_empty() {
            let candidate = merge(&line, &merge(&separator, &group.pieces));
            if width_of(&candidate, runs, measure) > max_width {
                emit(&mut line, &mut spaces, &mut result);
            } else {
                line = merge(&line, &separator);
            }
        }
        spaces.clear();
        if options.break_words && word_width > max_width {
            for piece in &group.pieces {
                for grapheme in piece.text.graphemes(true) {
                    let next = Piece {
                        source: piece.source,
                        text: grapheme.to_string(),
                    };
                    if !line.is_empty()
                        && width_of(&merge(&line, std::slice::from_ref(&next)), runs, measure)
                            > max_width
                    {
                        emit(&mut line, &mut spaces, &mut result);
                    }
                    line = merge(&line, std::slice::from_ref(&next));
                }
            }
        } else {
            line = merge(&line, &group.pieces);
        }
    }
    emit(&mut line, &mut spaces, &mut result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct Run {
        text: String,
        bold: Option<bool>,
        color: Option<&'static str>,
    }

    impl StyledRun for Run {
        fn text(&self) -> &str {
            &self.text
        }
        fn with_text(&self, text: &str) -> Self {
            Self {
                text: text.to_string(),
                bold: self.bold,
                color: self.color,
            }
        }
    }

    fn measure(text: &str, _: &Run) -> f64 {
        // Match upstream vitest measurer: `[...text].length` (Unicode code points).
        text.chars().count() as f64
    }

    fn strings(lines: &[Vec<Run>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| line.iter().map(|r| r.text.as_str()).collect::<String>())
            .collect()
    }

    #[test]
    fn keeps_words_crossing_style_boundaries_and_does_not_mutate_input() {
        // Catches: wrapStyledRuns splitting mid-word across run boundaries incorrectly.
        let runs = [
            Run {
                text: "a Hel".into(),
                bold: Some(true),
                color: None,
            },
            Run {
                text: "lo world".into(),
                bold: Some(false),
                color: None,
            },
        ];
        let original = runs.clone();
        let mut m = measure;
        let lines = wrap_styled_runs(&runs, 5.0, &mut m, WrapStyledRunsOptions::default());
        assert_eq!(strings(&lines), vec!["a", "Hello", "world"]);
        assert_eq!(
            lines[1],
            vec![
                Run {
                    text: "Hel".into(),
                    bold: Some(true),
                    color: None,
                },
                Run {
                    text: "lo".into(),
                    bold: Some(false),
                    color: None,
                },
            ]
        );
        assert_eq!(runs, original);
    }

    #[test]
    fn measures_combined_fragments_to_retain_kerning_at_word_boundaries() {
        // Catches: measuring word parts separately so "A V" wraps when combined width fits.
        let runs = [Run {
            text: "A V".into(),
            bold: None,
            color: None,
        }];
        let mut m = |text: &str, _: &Run| {
            if text == "A V" {
                2.0
            } else {
                text.len() as f64
            }
        };
        let lines = wrap_styled_runs(&runs, 2.0, &mut m, WrapStyledRunsOptions::default());
        assert_eq!(strings(&lines), vec!["A V"]);
    }

    #[test]
    fn retains_explicit_blank_lines_and_treats_nbsp_as_word() {
        // Catches: hard breaks collapsing or NBSP treated as a wrap opportunity.
        let runs = [Run {
            text: "\na\r\n\nb\n".into(),
            bold: None,
            color: None,
        }];
        let mut m = measure;
        assert_eq!(
            strings(&wrap_styled_runs(
                &runs,
                10.0,
                &mut m,
                WrapStyledRunsOptions::default()
            )),
            vec!["", "a", "", "b", ""]
        );
        let runs = [Run {
            text: "a\u{00a0}b c".into(),
            bold: None,
            color: None,
        }];
        let mut m = measure;
        assert_eq!(
            strings(&wrap_styled_runs(
                &runs,
                2.0,
                &mut m,
                WrapStyledRunsOptions::default()
            )),
            vec!["a\u{00a0}b", "c"]
        );
    }

    #[test]
    fn splits_oversized_words_only_when_requested_and_keeps_graphemes() {
        // Catches: breakWords false overflowing vs grapheme-safe split when true.
        let runs = [Run {
            text: "ab😀e\u{0301}cd".into(),
            bold: None,
            color: Some("red"),
        }];
        let mut m = measure;
        assert_eq!(
            strings(&wrap_styled_runs(
                &runs,
                2.0,
                &mut m,
                WrapStyledRunsOptions::default()
            )),
            vec!["ab😀e\u{0301}cd"]
        );
        let mut m = measure;
        let lines = wrap_styled_runs(
            &runs,
            2.0,
            &mut m,
            WrapStyledRunsOptions { break_words: true },
        );
        assert_eq!(strings(&lines), vec!["ab", "😀", "e\u{0301}", "cd"]);
        assert!(lines.iter().flatten().all(|r| r.color == Some("red")));
    }

    #[test]
    fn allows_one_oversized_grapheme_and_trims_separators_at_line_edges() {
        // Catches: leading/trailing spaces surviving automatic wraps; zero-width grapheme flush.
        let runs = [Run {
            text: "  abc   de  ".into(),
            bold: None,
            color: None,
        }];
        let mut m = measure;
        assert_eq!(
            strings(&wrap_styled_runs(
                &runs,
                2.0,
                &mut m,
                WrapStyledRunsOptions { break_words: true }
            )),
            vec!["ab", "c", "de"]
        );
        let runs = [Run {
            text: "😀".into(),
            bold: None,
            color: None,
        }];
        let mut m = measure;
        assert_eq!(
            strings(&wrap_styled_runs(
                &runs,
                0.0,
                &mut m,
                WrapStyledRunsOptions { break_words: true }
            )),
            vec!["😀"]
        );
    }
}
