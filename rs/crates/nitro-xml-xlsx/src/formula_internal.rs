// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/tokenizer.ts, lexemes.ts; changes listed in
// PROVENANCE.md.
//! Internal formula tokenizer/rewriter for `translate_formula`.

use crate::address::{column_index, column_label, MAX_COL, MAX_ROW};

const ERROR_CODES: &[&str] = &[
    "#GETTING_DATA",
    "#DIV/0!",
    "#NAME?",
    "#NULL!",
    "#NUM!",
    "#REF!",
    "#SPILL!",
    "#VALUE!",
    "#CALC!",
    "#N/A",
];

#[derive(Clone, Copy)]
struct RefCorner {
    row: i32,
    col: i32,
    row_abs: bool,
    col_abs: bool,
}

#[derive(Clone)]
struct RefSpec {
    kind: RefKind,
    start: RefCorner,
    end: RefCorner,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RefKind {
    Cell,
    Area,
    Cols,
    Rows,
}

#[derive(Clone)]
struct SheetPrefix {
    text: String,
}

#[derive(Clone)]
struct Token {
    kind: TokenKind,
    text: String,
    #[allow(dead_code)]
    start: usize,
    prefix: Option<SheetPrefix>,
    ref_spec: Option<RefSpec>,
    spill: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Ws,
    Ref,
    Other,
}

pub fn translate_formula(formula: &str, d_row: i32, d_col: i32) -> String {
    let mut tokens = match safe_tokens(formula) {
        Some(t) => t,
        None => return formula.to_owned(),
    };
    for token in tokens.iter_mut() {
        if token.kind != TokenKind::Ref {
            continue;
        }
        let spec = token.ref_spec.clone();
        if spec.is_none() {
            continue;
        }
        let spec = spec.unwrap();
        let move_corner = |c: RefCorner| -> Option<RefCorner> {
            let row = if spec.kind == RefKind::Cols || c.row_abs {
                c.row
            } else {
                c.row + d_row
            };
            let col = if spec.kind == RefKind::Rows || c.col_abs {
                c.col
            } else {
                c.col + d_col
            };
            if row < 0 || row > MAX_ROW as i32 || col < 0 || col > MAX_COL as i32 {
                return None;
            }
            Some(RefCorner {
                row,
                col,
                row_abs: c.row_abs,
                col_abs: c.col_abs,
            })
        };
        let start = move_corner(spec.start);
        let end = move_corner(spec.end);
        let next = if let (Some(s), Some(e)) = (start, end) {
            Some(RefSpec {
                kind: spec.kind,
                start: s,
                end: e,
            })
        } else {
            None
        };
        if let Some(n) = &next {
            if same_spec(n, &spec) {
                continue;
            }
        }
        token.text = rewrite_ref(&token.prefix, token.spill, next.as_ref());
    }
    join_tokens(&tokens)
}

fn same_spec(a: &RefSpec, b: &RefSpec) -> bool {
    a.kind == b.kind
        && a.start.row == b.start.row
        && a.start.col == b.start.col
        && a.end.row == b.end.row
        && a.end.col == b.end.col
}

fn safe_tokens(formula: &str) -> Option<Vec<Token>> {
    tokenize(formula).ok()
}

fn tokenize(formula: &str) -> Result<Vec<Token>, ()> {
    let source = formula;
    let mut tokens: Vec<Token> = Vec::new();
    let mut i = if source.starts_with('=') { 1 } else { 0 };
    if source.starts_with('=') {
        tokens.push(Token {
            kind: TokenKind::Ws,
            text: "=".to_owned(),
            start: 0,
            prefix: None,
            ref_spec: None,
            spill: false,
        });
    }
    while i < source.len() {
        let start = i;
        let ch = source[i..].chars().next().unwrap();
        if ch.is_whitespace() {
            let mut j = i + 1;
            while j < source.len() && source[j..].chars().next().unwrap().is_whitespace() {
                j += 1;
            }
            tokens.push(other_token(source, i, j));
            i = j;
            continue;
        }
        if ch == '"' {
            let mut j = i + 1;
            while j < source.len() {
                if source.as_bytes()[j] == b'"' {
                    if source.as_bytes().get(j + 1) == Some(&b'"') {
                        j += 2;
                        continue;
                    }
                    j += 1;
                    break;
                }
                j += 1;
            }
            if j > source.len() {
                return Err(());
            }
            tokens.push(other_token(source, i, j));
            i = j;
            continue;
        }
        if ch == '#' {
            let code = match_error(source, i);
            if code.is_none() {
                return Err(());
            }
            let code = code.unwrap();
            let len = code.len();
            if code == "#REF!" {
                tokens.push(Token {
                    kind: TokenKind::Ref,
                    text: source[i..i + len].to_owned(),
                    start,
                    prefix: None,
                    ref_spec: None,
                    spill: false,
                });
            } else {
                tokens.push(other_token(source, i, i + len));
            }
            i += len;
            continue;
        }
        if let Some((prefix, len)) = read_prefix(source, i) {
            let at = i + len;
            let tok = read_after_prefix(source, at, start, prefix)?;
            i = start + tok.text.len();
            tokens.push(tok);
            continue;
        }
        if let Some((spec, len)) = read_reference(source, i) {
            let mut stop = i + len;
            let mut spill = false;
            if spec.kind == RefKind::Cell
                && source.as_bytes().get(stop) == Some(&b'#')
                && match_error(source, stop).is_none()
            {
                spill = true;
                stop += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Ref,
                text: source[i..stop].to_owned(),
                start,
                prefix: None,
                ref_spec: Some(spec),
                spill,
            });
            i = stop;
            continue;
        }
        tokens.push(other_token(source, i, i + 1));
        i += 1;
    }
    Ok(tokens)
}

fn other_token(source: &str, start: usize, end: usize) -> Token {
    Token {
        kind: TokenKind::Other,
        text: source[start..end].to_owned(),
        start,
        prefix: None,
        ref_spec: None,
        spill: false,
    }
}

fn read_after_prefix(
    source: &str,
    at: usize,
    start: usize,
    prefix: SheetPrefix,
) -> Result<Token, ()> {
    if source[at..].to_ascii_uppercase().starts_with("#REF!") {
        return Ok(Token {
            kind: TokenKind::Ref,
            text: source[start..at + 5].to_owned(),
            start,
            prefix: Some(prefix),
            ref_spec: None,
            spill: false,
        });
    }
    if let Some((spec, len)) = read_reference(source, at) {
        let mut stop = at + len;
        let mut spill = false;
        if spec.kind == RefKind::Cell
            && source.as_bytes().get(stop) == Some(&b'#')
            && match_error(source, stop).is_none()
        {
            spill = true;
            stop += 1;
        }
        return Ok(Token {
            kind: TokenKind::Ref,
            text: source[start..stop].to_owned(),
            start,
            prefix: Some(prefix),
            ref_spec: Some(spec),
            spill,
        });
    }
    Err(())
}

fn join_tokens(tokens: &[Token]) -> String {
    tokens.iter().map(|t| t.text.as_str()).collect()
}

fn rewrite_ref(prefix: &Option<SheetPrefix>, spill: bool, spec: Option<&RefSpec>) -> String {
    let p = prefix.as_ref().map(|x| x.text.as_str()).unwrap_or("");
    if spec.is_none() {
        return format!("{p}#REF!");
    }
    let body = format_ref_spec(spec.unwrap());
    if spill {
        format!("{p}{body}#")
    } else {
        format!("{p}{body}")
    }
}

fn format_ref_spec(spec: &RefSpec) -> String {
    let a = corner_text(&spec.start, spec.kind);
    if spec.kind == RefKind::Cell {
        return a;
    }
    format!("{}:{}", a, corner_text(&spec.end, spec.kind))
}

fn corner_text(c: &RefCorner, kind: RefKind) -> String {
    match kind {
        RefKind::Cols => format!(
            "{}{}",
            if c.col_abs { "$" } else { "" },
            column_label(c.col as u32)
        ),
        RefKind::Rows => format!("{}{}", if c.row_abs { "$" } else { "" }, c.row + 1),
        _ => format!(
            "{}{}{}{}",
            if c.col_abs { "$" } else { "" },
            column_label(c.col as u32),
            if c.row_abs { "$" } else { "" },
            c.row + 1
        ),
    }
}

fn match_error(source: &str, at: usize) -> Option<&'static str> {
    let upper = source[at..].to_ascii_uppercase();
    ERROR_CODES
        .iter()
        .find(|&code| upper.starts_with(code))
        .map(|v| v as _)
}

fn read_prefix(source: &str, at: usize) -> Option<(SheetPrefix, usize)> {
    let bytes = source.as_bytes();
    if at >= bytes.len() {
        return None;
    }
    if bytes[at] == b'\'' {
        let mut i = at + 1;
        let mut name = String::new();
        while i < bytes.len() {
            if bytes[i] == b'\'' {
                if bytes.get(i + 1) == Some(&b'\'') {
                    name.push('\'');
                    i += 2;
                    continue;
                }
                if bytes.get(i + 1) != Some(&b'!') {
                    return None;
                }
                let len = i + 2 - at;
                return Some((
                    SheetPrefix {
                        text: source[at..at + len].to_owned(),
                    },
                    len,
                ));
            }
            name.push(bytes[i] as char);
            i += 1;
        }
        return None;
    }
    let mut i = at;
    if bytes[i] == b'[' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == at + 1 || bytes.get(i) != Some(&b']') {
            return None;
        }
        i += 1;
    }
    let sheet_from = i;
    while i < bytes.len() && is_sheet_char(bytes[i]) {
        i += 1;
    }
    if i == sheet_from {
        return None;
    }
    if bytes.get(i) == Some(&b':') {
        let last_from = i + 1;
        i = last_from;
        while i < bytes.len() && is_sheet_char(bytes[i]) {
            i += 1;
        }
        if i == last_from {
            return None;
        }
    }
    if bytes.get(i) != Some(&b'!') {
        return None;
    }
    if bytes[at] != b'[' && bytes[sheet_from].is_ascii_digit() {
        return None;
    }
    let len = i + 1 - at;
    Some((
        SheetPrefix {
            text: source[at..at + len].to_owned(),
        },
        len,
    ))
}

fn is_sheet_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b >= 0xA1
}

fn read_reference(source: &str, at: usize) -> Option<(RefSpec, usize)> {
    if let Some(cell) = match_cell(source, at) {
        let (start, len1) = cell;
        let length = len1;
        if source.as_bytes().get(at + length) == Some(&b':') {
            if let Some((end, len2)) = match_cell(source, at + length + 1) {
                let total = length + 1 + len2;
                if boundary(source, at + total) {
                    return Some((
                        RefSpec {
                            kind: RefKind::Area,
                            start,
                            end,
                        },
                        total,
                    ));
                }
            }
        }
        if boundary(source, at + length) {
            return Some((
                RefSpec {
                    kind: RefKind::Cell,
                    start,
                    end: start,
                },
                length,
            ));
        }
    }
    if range_start_cols(source, at) {
        if let Some((spec, len)) = match_cols(source, at) {
            if boundary(source, at + len)
                && !source[at + len..]
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
            {
                return Some((spec, len));
            }
        }
    }
    if range_start_rows(source, at) {
        if let Some((spec, len)) = match_rows(source, at) {
            if boundary(source, at + len) && !source[at + len..].starts_with('.') {
                return Some((spec, len));
            }
        }
    }
    None
}

fn boundary(source: &str, end: usize) -> bool {
    let next = source[end..].chars().next();
    !matches!(next, Some(c) if c.is_ascii_alphanumeric() || c == '_' || c == '(' || c == '!' || c == '[')
}

fn match_cell(source: &str, at: usize) -> Option<(RefCorner, usize)> {
    let s = &source[at..];
    let re = regex_cell(s)?;
    Some((re.0, re.1))
}

fn regex_cell(s: &str) -> Option<(RefCorner, usize)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let col_abs = if bytes.get(i) == Some(&b'$') {
        i += 1;
        true
    } else {
        false
    };
    let col_start = i;
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i == col_start || i - col_start > 3 {
        return None;
    }
    let col = column_index(&s[col_start..i])? as i32;
    let row_abs = if bytes.get(i) == Some(&b'$') {
        i += 1;
        true
    } else {
        false
    };
    let row_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == row_start || i - row_start > 7 {
        return None;
    }
    let row: i32 = s[row_start..i].parse::<i32>().ok()? - 1;
    if row < 0 || row > MAX_ROW as i32 || col < 0 || col > MAX_COL as i32 {
        return None;
    }
    Some((
        RefCorner {
            row,
            col,
            row_abs,
            col_abs,
        },
        i,
    ))
}

fn range_start_cols(source: &str, at: usize) -> bool {
    let mut i = at;
    if source.as_bytes().get(i) == Some(&b'$') {
        i += 1;
    }
    let from = i;
    while i < source.len() && source[i..].chars().next().unwrap().is_ascii_alphabetic() {
        i += 1;
    }
    i > from && i - from <= 3 && source.as_bytes().get(i) == Some(&b':')
}

fn range_start_rows(source: &str, at: usize) -> bool {
    let mut i = at;
    if source.as_bytes().get(i) == Some(&b'$') {
        i += 1;
    }
    let from = i;
    while i < source.len() && source[i..].chars().next().unwrap().is_ascii_digit() {
        i += 1;
    }
    i > from && i - from <= 7 && source.as_bytes().get(i) == Some(&b':')
}

fn match_cols(source: &str, at: usize) -> Option<(RefSpec, usize)> {
    let bytes = source.as_bytes();
    let mut i = at;
    let c1_abs = bytes.get(i) == Some(&b'$');
    if c1_abs {
        i += 1;
    }
    let s1 = i;
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i == s1 {
        return None;
    }
    let col1 = column_index(&source[s1..i])? as i32;
    if bytes.get(i) != Some(&b':') {
        return None;
    }
    i += 1;
    let c2_abs = bytes.get(i) == Some(&b'$');
    if c2_abs {
        i += 1;
    }
    let s2 = i;
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    let col2 = column_index(&source[s2..i])? as i32;
    Some((
        RefSpec {
            kind: RefKind::Cols,
            start: RefCorner {
                row: 0,
                col: col1,
                row_abs: false,
                col_abs: c1_abs,
            },
            end: RefCorner {
                row: MAX_ROW as i32,
                col: col2,
                row_abs: false,
                col_abs: c2_abs,
            },
        },
        i - at,
    ))
}

fn match_rows(source: &str, at: usize) -> Option<(RefSpec, usize)> {
    let bytes = source.as_bytes();
    let mut i = at;
    let r1_abs = bytes.get(i) == Some(&b'$');
    if r1_abs {
        i += 1;
    }
    let s1 = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let r1: i32 = source[s1..i].parse::<i32>().ok()? - 1;
    if bytes.get(i) != Some(&b':') {
        return None;
    }
    i += 1;
    let r2_abs = bytes.get(i) == Some(&b'$');
    if r2_abs {
        i += 1;
    }
    let s2 = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let r2: i32 = source[s2..i].parse::<i32>().ok()? - 1;
    Some((
        RefSpec {
            kind: RefKind::Rows,
            start: RefCorner {
                row: r1,
                col: 0,
                row_abs: r1_abs,
                col_abs: false,
            },
            end: RefCorner {
                row: r2,
                col: MAX_COL as i32,
                row_abs: r2_abs,
                col_abs: false,
            },
        },
        i - at,
    ))
}
