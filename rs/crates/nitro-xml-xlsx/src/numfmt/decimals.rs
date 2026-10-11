// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/numfmt/decimals.ts; changes listed in PROVENANCE.md.

use crate::numfmt::general::format_general;
use crate::numfmt::tokenizer::{split_sections, tokenize_section};
use crate::numfmt::types::{CellValue, SectionKind};

fn shown_decimals(text: &str) -> (bool, usize) {
    let mut chars = text.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_digit()) {
        return (false, plain_shown(text));
    }
    let rest: String = chars.collect();
    if let Some(after) = rest.strip_prefix('.') {
        let n = after.chars().take_while(|c| c.is_ascii_digit()).count();
        if n > 0 && after[n..].starts_with('E') {
            return (true, n);
        }
    } else if rest.starts_with('E') {
        return (true, 0);
    }
    (false, plain_shown(text))
}

fn plain_shown(text: &str) -> usize {
    let Some(i) = text.find('.') else {
        return 0;
    };
    text[i + 1..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .count()
}

fn from_general(delta: i32, sample: Option<&CellValue>) -> String {
    let text = match sample {
        Some(CellValue::Number(n)) => format_general(n.abs(), None),
        _ => String::new(),
    };
    let (sci, shown) = shown_decimals(&text);
    let next = (shown as i32 + delta).max(0) as usize;
    let body = if next == 0 {
        "0".to_string()
    } else {
        format!("0.{}", "0".repeat(next))
    };
    if sci {
        format!("{body}E+00")
    } else {
        body
    }
}

fn strip_quotes(section: &str) -> String {
    let cs: Vec<char> = section.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '"' {
            if let Some(rel) = cs[i + 1..].iter().position(|c| *c == '"') {
                i += rel + 2;
                continue;
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

fn looks_like_fraction(section: &str) -> bool {
    let cs: Vec<char> = strip_quotes(section).chars().collect();
    for i in 0..cs.len() {
        if cs[i] == '?' {
            let mut j = i + 1;
            while j < cs.len() && cs[j].is_whitespace() {
                j += 1;
            }
            if j < cs.len() && cs[j] == '/' {
                return true;
            }
        }
        if cs[i] == '/' {
            let mut j = i + 1;
            while j < cs.len() && cs[j].is_whitespace() {
                j += 1;
            }
            if j < cs.len() && matches!(cs[j], '?' | '#' | '0'..='9') {
                return true;
            }
        }
    }
    false
}

fn scan(section: &str) -> (isize, isize) {
    let cs: Vec<char> = section.chars().collect();
    let mut quoted = false;
    let mut bracket = false;
    let mut last_digit = -1isize;
    let mut dot = -1isize;
    let mut i = 0;
    while i < cs.len() {
        let ch = cs[i];
        if quoted {
            if ch == '"' {
                quoted = false;
            }
            i += 1;
            continue;
        }
        if bracket {
            if ch == ']' {
                bracket = false;
            }
            i += 1;
            continue;
        }
        if ch == '\\' || ch == '_' || ch == '*' {
            i += 2;
            continue;
        }
        if ch == '"' {
            quoted = true;
        } else if ch == '[' {
            bracket = true;
        } else if ch == '0' || ch == '#' || ch == '?' {
            last_digit = i as isize;
        } else if ch == '.' && dot < 0 {
            dot = i as isize;
        } else if (ch == 'e' || ch == 'E') && last_digit >= 0 {
            break;
        }
        i += 1;
    }
    let dot = if dot >= 0 && dot < last_digit {
        dot
    } else {
        -1
    };
    (last_digit, dot)
}

fn step_section(section: &str, delta: i32) -> String {
    if tokenize_section(section).kind != SectionKind::Number || looks_like_fraction(section) {
        return section.to_string();
    }
    let cs: Vec<char> = section.chars().collect();
    let (last_digit, dot) = scan(section);
    if last_digit < 0 {
        return section.to_string();
    }
    let last = last_digit as usize;
    let head: String = cs[..=last].iter().collect();
    let tail: String = cs[last + 1..].iter().collect();
    if delta > 0 {
        return if dot >= 0 {
            format!("{head}0{tail}")
        } else {
            format!("{head}.0{tail}")
        };
    }
    if dot < 0 {
        return section.to_string();
    }
    if last_digit - dot <= 1 {
        let mut out: String = cs[..dot as usize].iter().collect();
        out.push_str(&tail);
        return out;
    }
    let mut out: String = cs[..last].iter().collect();
    out.push_str(&tail);
    out
}

/// The format after Excel's Increase Decimal (`delta` > 0) or Decrease Decimal.
pub fn step_decimals(format: &str, delta: i32, sample: Option<&CellValue>) -> String {
    let code = if format.trim().is_empty() {
        "General"
    } else {
        format
    };
    if code.trim().eq_ignore_ascii_case("general") {
        return from_general(delta, sample);
    }
    split_sections(code)
        .iter()
        .map(|section| step_section(section, delta))
        .collect::<Vec<_>>()
        .join(";")
}
