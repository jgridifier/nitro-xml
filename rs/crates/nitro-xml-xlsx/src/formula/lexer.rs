// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/tokenizer.ts, lexemes.ts;
// changes listed in PROVENANCE.md.
//! Formula lexer. Offsets are UTF-16 code units, matching JavaScript string indexes.

use std::sync::LazyLock;

use crate::address::{column_index, MAX_COL, MAX_ROW};
use crate::formula::ast::{FormulaError, RefCorner, RefKind, RefSpec, SheetPrefix};

/// Same list and order as upstream `ERROR_CODES`, then sorted longest-first (stable).
const ERROR_CODES: &[&str] = &[
    "#NULL!",
    "#DIV/0!",
    "#VALUE!",
    "#REF!",
    "#NAME?",
    "#NUM!",
    "#N/A",
    "#GETTING_DATA",
    "#SPILL!",
    "#CALC!",
];

static ERRORS_BY_LENGTH: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut codes = ERROR_CODES.to_vec();
    codes.sort_by_key(|code| std::cmp::Reverse(code.len()));
    codes
});

const OPERATORS: &[&str] = &[
    "<=", ">=", "<>", "<", ">", "=", "+", "-", "*", "/", "^", "&", "%", ":", "@",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Number,
    String,
    Bool,
    Error,
    Ref,
    Name,
    Func,
    Structured,
    Op,
    Open,
    Close,
    Comma,
    Semicolon,
    Lbrace,
    Rbrace,
    Ws,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenValue {
    Number(f64),
    Text(String),
    Bool(bool),
}

/// A lexical token. `text` is the exact source slice, so joining texts round-trips.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub start: usize,
    pub value: Option<TokenValue>,
    pub prefix: Option<SheetPrefix>,
    pub reference: Option<RefSpec>,
    pub spill: bool,
}

/// Splits a formula (with or without a leading `=`) into tokens, whitespace included.
pub fn tokenize(formula: &str) -> Result<Vec<Token>, FormulaError> {
    let src = Src::new(formula);
    let mut tokens = Vec::new();
    let mut i = 0;
    if src.unit(0) == Some(u16::from(b'=')) {
        tokens.push(make_token(
            TokenKind::Ws,
            src.slice(0, 1),
            0,
            None,
            None,
            None,
            false,
        ));
        i = 1;
    }
    while i < src.len() {
        let (token, next) = lex_at(&src, i)?;
        tokens.push(token);
        i = next;
    }
    Ok(tokens)
}

/// Reassembles tokens into formula text.
pub fn join_tokens(tokens: &[Token]) -> String {
    let mut out = String::new();
    for token in tokens {
        out.push_str(&token.text);
    }
    out
}

/// A number literal's value. Excel keeps only 15 significant digits of what was typed.
pub fn number_literal(text: &str) -> f64 {
    if utf16_len(text) <= 15 {
        return parse_js_number(text);
    }
    let lower = text.to_ascii_lowercase();
    let mut parts = lower.splitn(2, 'e');
    let mantissa = parts.next().unwrap_or("");
    let exponent = parts.next();
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let Some(first) = digits.chars().position(|c| ('1'..='9').contains(&c)) else {
        return parse_js_number(text);
    };
    if digits.chars().count() - first <= 15 {
        return parse_js_number(text);
    }
    let point = mantissa.find('.').unwrap_or(mantissa.chars().count());
    let kept: String = digits.chars().take(first + 15).collect();
    let kept_len = kept.chars().count();
    let truncated = if kept_len >= point {
        let (head, tail) = split_chars(&kept, point);
        format!("{head}.{tail}")
    } else {
        format!("{kept}{}", "0".repeat(point - kept_len))
    };
    let literal = match exponent {
        Some(exp) => format!("{truncated}e{exp}"),
        None => truncated,
    };
    parse_js_number(&literal)
}

fn lex_at(src: &Src<'_>, i: usize) -> Result<(Token, usize), FormulaError> {
    let start = i;
    let ch = src.unit(i).unwrap_or(0);
    if is_space(ch) {
        let end = skip_while(src, i, is_space);
        return Ok((ws_token(src, start, end), end));
    }
    if ch == u16::from(b'"') {
        return lex_string(src, start);
    }
    if ch == u16::from(b'#') {
        return lex_hash(src, start);
    }
    if let Some(kind) = single_kind(ch) {
        let end = start + 1;
        return Ok((plain(kind, src, start, end), end));
    }
    if ch == u16::from(b'[') && !is_book_index(src, i) {
        let len = read_brackets(src, i)?;
        let end = start + len;
        return Ok((structured_token(src, start, end, String::new()), end));
    }
    let name_start = is_name_start(ch);
    let digit = is_digit(ch);
    if ch == u16::from(b'\'') || ch == u16::from(b'[') || name_start || digit {
        if let Some((prefix, len)) = read_prefix(src, i)? {
            return read_after_prefix(src, i + len, start, prefix);
        }
        if ch == u16::from(b'\'') || ch == u16::from(b'[') {
            return Err(FormulaError::new("Malformed sheet reference", start as i64));
        }
    }
    let numeric = digit || ch == u16::from(b'.');
    if numeric || ch == u16::from(b'$') || name_start {
        if let Some((spec, len)) = read_reference(src, i) {
            return Ok(finish_ref(src, start, i + len, spec, None));
        }
        if numeric {
            let Some(end) = match_number(src, i) else {
                return Err(unexpected(src, start));
            };
            let text = src.slice(i, end);
            let value = number_literal(&text);
            return Ok((
                make_token(
                    TokenKind::Number,
                    text,
                    start,
                    Some(TokenValue::Number(value)),
                    None,
                    None,
                    false,
                ),
                end,
            ));
        }
        if ch != u16::from(b'$') {
            return read_name(src, i, start, None);
        }
    }
    if let Some(op) = OPERATORS.iter().find(|op| src.starts_with(i, op)) {
        let end = i + op.len();
        return Ok((plain(TokenKind::Op, src, start, end), end));
    }
    Err(unexpected(src, start))
}

fn lex_string(src: &Src<'_>, start: usize) -> Result<(Token, usize), FormulaError> {
    let mut j = start + 1;
    let mut value_units = Vec::new();
    loop {
        let Some(quote) = find_unit(src, j, u16::from(b'"')) else {
            return Err(FormulaError::new("Unterminated string", start as i64));
        };
        value_units.extend_from_slice(&src.units[j..quote]);
        if src.unit(quote + 1) == Some(u16::from(b'"')) {
            value_units.push(u16::from(b'"'));
            j = quote + 2;
            continue;
        }
        j = quote;
        break;
    }
    let end = j + 1;
    let value =
        String::from_utf16(&value_units).unwrap_or_else(|_| String::from_utf16_lossy(&value_units));
    Ok((
        make_token(
            TokenKind::String,
            src.slice(start, end),
            start,
            Some(TokenValue::Text(value)),
            None,
            None,
            false,
        ),
        end,
    ))
}

fn lex_hash(src: &Src<'_>, start: usize) -> Result<(Token, usize), FormulaError> {
    let Some(code) = match_error(src, start) else {
        return Err(FormulaError::new("Unexpected '#'", start as i64));
    };
    let end = start + code.len();
    let text = src.slice(start, end);
    if code == "#REF!" {
        Ok((
            make_token(
                TokenKind::Ref,
                text,
                start,
                Some(TokenValue::Text(code.to_owned())),
                None,
                None,
                false,
            ),
            end,
        ))
    } else {
        Ok((
            make_token(
                TokenKind::Error,
                text,
                start,
                Some(TokenValue::Text(code.to_owned())),
                None,
                None,
                false,
            ),
            end,
        ))
    }
}

fn read_after_prefix(
    src: &Src<'_>,
    at: usize,
    start: usize,
    prefix: SheetPrefix,
) -> Result<(Token, usize), FormulaError> {
    if src
        .slice(at, (at + 5).min(src.len()))
        .eq_ignore_ascii_case("#REF!")
        && src.len().saturating_sub(at) >= 5
    {
        let end = at + 5;
        return Ok((
            make_token(
                TokenKind::Ref,
                src.slice(start, end),
                start,
                None,
                Some(prefix),
                None,
                false,
            ),
            end,
        ));
    }
    if let Some((spec, len)) = read_reference(src, at) {
        return Ok(finish_ref(src, start, at + len, spec, Some(prefix)));
    }
    if src.unit(at).is_some_and(is_name_start) {
        return read_name(src, at, start, Some(prefix));
    }
    Err(FormulaError::new(
        "Invalid reference after sheet name",
        at as i64,
    ))
}

fn read_name(
    src: &Src<'_>,
    at: usize,
    start: usize,
    prefix: Option<SheetPrefix>,
) -> Result<(Token, usize), FormulaError> {
    let Some(end) = match_name(src, at) else {
        return Err(FormulaError::new("Invalid name", at as i64));
    };
    let name = src.slice(at, end);
    if src.unit(end) == Some(u16::from(b'[')) && prefix.is_none() {
        let stop = end + read_brackets(src, end)?;
        return Ok((structured_token(src, start, stop, name), stop));
    }
    if src.unit(end) == Some(u16::from(b'(')) {
        return Ok((
            make_token(
                TokenKind::Func,
                src.slice(start, end),
                start,
                Some(TokenValue::Text(name)),
                None,
                None,
                false,
            ),
            end,
        ));
    }
    if prefix.is_none() && (name.eq_ignore_ascii_case("TRUE") || name.eq_ignore_ascii_case("FALSE"))
    {
        let value = name.eq_ignore_ascii_case("TRUE");
        return Ok((
            make_token(
                TokenKind::Bool,
                name,
                start,
                Some(TokenValue::Bool(value)),
                None,
                None,
                false,
            ),
            end,
        ));
    }
    Ok((
        make_token(
            TokenKind::Name,
            src.slice(start, end),
            start,
            Some(TokenValue::Text(name)),
            prefix,
            None,
            false,
        ),
        end,
    ))
}

fn finish_ref(
    src: &Src<'_>,
    start: usize,
    end: usize,
    spec: RefSpec,
    prefix: Option<SheetPrefix>,
) -> (Token, usize) {
    let mut stop = end;
    let mut spill = false;
    if spec.kind == RefKind::Cell
        && src.unit(end) == Some(u16::from(b'#'))
        && match_error(src, end).is_none()
    {
        spill = true;
        stop = end + 1;
    }
    (
        make_token(
            TokenKind::Ref,
            src.slice(start, stop),
            start,
            None,
            prefix,
            Some(spec),
            spill,
        ),
        stop,
    )
}

fn match_error(src: &Src<'_>, at: usize) -> Option<&'static str> {
    let end = (at + 14).min(src.len());
    let window = src.slice(at, end).to_uppercase();
    ERRORS_BY_LENGTH
        .iter()
        .copied()
        .find(|code| window.starts_with(code))
}

fn read_reference(src: &Src<'_>, at: usize) -> Option<(RefSpec, usize)> {
    if let Some(cell) = match_cell(src, at) {
        if let Some(start) = corner(cell.col_abs, &cell.col, cell.row_abs, &cell.row) {
            let length = cell.end - at;
            if src.unit(at + length) == Some(u16::from(b':')) {
                if let Some(second) = match_cell(src, at + length + 1) {
                    if let Some(end) =
                        corner(second.col_abs, &second.col, second.row_abs, &second.row)
                    {
                        let total = length + 1 + (second.end - (at + length + 1));
                        if boundary(src, at + total) {
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
            }
            if boundary(src, at + length) {
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
    }
    if range_start_at(src, at, true, 3) {
        if let Some(cols) = match_cols(src, at) {
            if boundary(src, cols.end) && !src.unit(cols.end).is_some_and(is_digit) {
                if let (Some(c1), Some(c2)) = (column_index(&cols.c1), column_index(&cols.c2)) {
                    if c1 <= MAX_COL && c2 <= MAX_COL {
                        return Some((
                            RefSpec {
                                kind: RefKind::Cols,
                                start: RefCorner {
                                    row: 0,
                                    col: c1 as i32,
                                    row_abs: false,
                                    col_abs: cols.abs1,
                                },
                                end: RefCorner {
                                    row: MAX_ROW as i32,
                                    col: c2 as i32,
                                    row_abs: false,
                                    col_abs: cols.abs2,
                                },
                            },
                            cols.end - at,
                        ));
                    }
                }
            }
        }
    }
    if range_start_at(src, at, false, 7) {
        if let Some(rows) = match_rows(src, at) {
            if boundary(src, rows.end) && src.unit(rows.end) != Some(u16::from(b'.')) {
                let n1: u32 = rows.r1.parse().ok()?;
                let n2: u32 = rows.r2.parse().ok()?;
                if n1 == 0 || n2 == 0 {
                    return None;
                }
                let r1 = n1 - 1;
                let r2 = n2 - 1;
                if r1 <= MAX_ROW && r2 <= MAX_ROW {
                    return Some((
                        RefSpec {
                            kind: RefKind::Rows,
                            start: RefCorner {
                                row: r1 as i32,
                                col: 0,
                                row_abs: rows.abs1,
                                col_abs: false,
                            },
                            end: RefCorner {
                                row: r2 as i32,
                                col: MAX_COL as i32,
                                row_abs: rows.abs2,
                                col_abs: false,
                            },
                        },
                        rows.end - at,
                    ));
                }
            }
        }
    }
    None
}

struct CellMatch {
    col_abs: bool,
    col: String,
    row_abs: bool,
    row: String,
    end: usize,
}

fn match_cell(src: &Src<'_>, at: usize) -> Option<CellMatch> {
    let mut i = at;
    let col_abs = src.unit(i) == Some(u16::from(b'$'));
    if col_abs {
        i += 1;
    }
    let col_from = i;
    let mut letters = 0;
    while letters < 3 && src.unit(i).is_some_and(is_letter) {
        i += 1;
        letters += 1;
    }
    if letters == 0 {
        return None;
    }
    let col = src.slice(col_from, i);
    let row_abs = src.unit(i) == Some(u16::from(b'$'));
    if row_abs {
        i += 1;
    }
    let row_from = i;
    let mut digits = 0;
    while digits < 7 && src.unit(i).is_some_and(is_digit) {
        i += 1;
        digits += 1;
    }
    if digits == 0 {
        return None;
    }
    Some(CellMatch {
        col_abs,
        col,
        row_abs,
        row: src.slice(row_from, i),
        end: i,
    })
}

struct ColMatch {
    abs1: bool,
    c1: String,
    abs2: bool,
    c2: String,
    end: usize,
}

fn match_cols(src: &Src<'_>, at: usize) -> Option<ColMatch> {
    let (abs1, c1, mut i) = take_abs_run(src, at, true, 3)?;
    if src.unit(i) != Some(u16::from(b':')) {
        return None;
    }
    i += 1;
    let (abs2, c2, end) = take_abs_run(src, i, true, 3)?;
    Some(ColMatch {
        abs1,
        c1,
        abs2,
        c2,
        end,
    })
}

struct RowMatch {
    abs1: bool,
    r1: String,
    abs2: bool,
    r2: String,
    end: usize,
}

fn match_rows(src: &Src<'_>, at: usize) -> Option<RowMatch> {
    let (abs1, r1, mut i) = take_abs_run(src, at, false, 7)?;
    if src.unit(i) != Some(u16::from(b':')) {
        return None;
    }
    i += 1;
    let (abs2, r2, end) = take_abs_run(src, i, false, 7)?;
    Some(RowMatch {
        abs1,
        r1,
        abs2,
        r2,
        end,
    })
}

fn take_abs_run(
    src: &Src<'_>,
    at: usize,
    letters: bool,
    max: usize,
) -> Option<(bool, String, usize)> {
    let mut i = at;
    let abs = src.unit(i) == Some(u16::from(b'$'));
    if abs {
        i += 1;
    }
    let from = i;
    let mut n = 0;
    while n < max
        && src
            .unit(i)
            .is_some_and(|u| if letters { is_letter(u) } else { is_digit(u) })
    {
        i += 1;
        n += 1;
    }
    if n == 0 {
        return None;
    }
    Some((abs, src.slice(from, i), i))
}

fn range_start_at(src: &Src<'_>, at: usize, letters: bool, max: usize) -> bool {
    let mut i = if src.unit(at) == Some(u16::from(b'$')) {
        at + 1
    } else {
        at
    };
    let from = i;
    while i - from <= max
        && src
            .unit(i)
            .is_some_and(|u| if letters { is_letter(u) } else { is_digit(u) })
    {
        i += 1;
    }
    i > from && i - from <= max && src.unit(i) == Some(u16::from(b':'))
}

fn boundary(src: &Src<'_>, end: usize) -> bool {
    match src.unit(end) {
        None => true,
        Some(next) => {
            !is_name_char(next)
                && next != u16::from(b'(')
                && next != u16::from(b'!')
                && next != u16::from(b'[')
        }
    }
}

fn corner(col_abs: bool, col: &str, row_abs: bool, row: &str) -> Option<RefCorner> {
    let col = column_index(col)? as i32;
    if col > MAX_COL as i32 {
        return None;
    }
    let row_num: u32 = row.parse().ok()?;
    if row_num == 0 {
        return None;
    }
    let row = row_num - 1;
    if row > MAX_ROW {
        return None;
    }
    Some(RefCorner {
        row: row as i32,
        col,
        row_abs,
        col_abs,
    })
}

fn read_quoted(src: &Src<'_>, at: usize) -> Result<(String, usize), FormulaError> {
    let mut i = at + 1;
    let mut name = Vec::new();
    while i < src.len() {
        let ch = src.units[i];
        if ch == u16::from(b'\'') {
            if src.unit(i + 1) == Some(u16::from(b'\'')) {
                name.push(u16::from(b'\''));
                i += 2;
                continue;
            }
            let text =
                String::from_utf16(&name).unwrap_or_else(|_| String::from_utf16_lossy(&name));
            return Ok((text, i + 1 - at));
        }
        name.push(ch);
        i += 1;
    }
    Err(FormulaError::new(
        "Unterminated quoted sheet name",
        at as i64,
    ))
}

fn read_brackets(src: &Src<'_>, at: usize) -> Result<usize, FormulaError> {
    let mut depth = 0i32;
    let mut i = at;
    while i < src.len() {
        let ch = src.units[i];
        if ch == u16::from(b'\'') {
            i += 2;
            continue;
        }
        if ch == u16::from(b'[') {
            depth += 1;
        } else if ch == u16::from(b']') {
            depth -= 1;
            if depth == 0 {
                return Ok(i + 1 - at);
            }
        }
        i += 1;
    }
    Err(FormulaError::new(
        "Unterminated structured reference",
        at as i64,
    ))
}

fn split_sheets(text: &str) -> (String, Option<String>, Option<String>) {
    let mut rest = text;
    let mut book = None;
    if let Some(end) = rest.find(']') {
        if rest.starts_with('[') {
            book = Some(rest[1..end].to_owned());
            rest = &rest[end + 1..];
        }
    }
    if let Some(colon) = rest.find(':') {
        (
            rest[..colon].to_owned(),
            Some(rest[colon + 1..].to_owned()),
            book,
        )
    } else {
        (rest.to_owned(), None, book)
    }
}

fn read_prefix(src: &Src<'_>, at: usize) -> Result<Option<(SheetPrefix, usize)>, FormulaError> {
    let Some(ch) = src.unit(at) else {
        return Ok(None);
    };
    if ch == u16::from(b'\'') {
        let (name, quoted_len) = read_quoted(src, at)?;
        if src.unit(at + quoted_len) != Some(u16::from(b'!')) {
            return Ok(None);
        }
        let length = quoted_len + 1;
        let (sheet, sheet2, book) = split_sheets(&name);
        return Ok(Some((
            SheetPrefix {
                text: src.slice(at, at + length),
                sheet,
                sheet2,
                book,
            },
            length,
        )));
    }
    let mut i = at;
    if ch == u16::from(b'[') {
        i += 1;
        let digits_from = i;
        while src.unit(i).is_some_and(is_digit) {
            i += 1;
        }
        if i == digits_from || src.unit(i) != Some(u16::from(b']')) {
            return Ok(None);
        }
        i += 1;
    }
    let sheet_from = i;
    while src.unit(i).is_some_and(is_sheet_char) {
        i += 1;
    }
    if i == sheet_from {
        return Ok(None);
    }
    let mut last_from = -1isize;
    if src.unit(i) == Some(u16::from(b':')) {
        last_from = (i + 1) as isize;
        i = last_from as usize;
        while src.unit(i).is_some_and(is_sheet_char) {
            i += 1;
        }
        if i == last_from as usize {
            return Ok(None);
        }
    }
    if src.unit(i) != Some(u16::from(b'!')) {
        return Ok(None);
    }
    if ch != u16::from(b'[') && last_from < 0 && src.unit(sheet_from).is_some_and(is_digit) {
        return Ok(None);
    }
    let length = i + 1 - at;
    let (sheet, sheet2, book) = split_sheets(&src.slice(at, i));
    Ok(Some((
        SheetPrefix {
            text: src.slice(at, at + length),
            sheet,
            sheet2,
            book,
        },
        length,
    )))
}

fn match_name(src: &Src<'_>, at: usize) -> Option<usize> {
    if !src.unit(at).is_some_and(is_name_start) {
        return None;
    }
    let mut i = at + 1;
    while src.unit(i).is_some_and(is_name_char) {
        i += 1;
    }
    Some(i)
}

fn match_number(src: &Src<'_>, at: usize) -> Option<usize> {
    let mut i = at;
    if src.unit(i).is_some_and(is_digit) {
        while src.unit(i).is_some_and(is_digit) {
            i += 1;
        }
        if src.unit(i) == Some(u16::from(b'.')) {
            i += 1;
            while src.unit(i).is_some_and(is_digit) {
                i += 1;
            }
        }
    } else if src.unit(i) == Some(u16::from(b'.')) {
        i += 1;
        if !src.unit(i).is_some_and(is_digit) {
            return None;
        }
        while src.unit(i).is_some_and(is_digit) {
            i += 1;
        }
    } else {
        return None;
    }
    if matches!(src.unit(i), Some(u) if u == u16::from(b'e') || u == u16::from(b'E')) {
        let exp_at = i;
        i += 1;
        if matches!(src.unit(i), Some(u) if u == u16::from(b'+') || u == u16::from(b'-')) {
            i += 1;
        }
        if !src.unit(i).is_some_and(is_digit) {
            i = exp_at;
        } else {
            while src.unit(i).is_some_and(is_digit) {
                i += 1;
            }
        }
    }
    if i == at {
        None
    } else {
        Some(i)
    }
}

fn is_book_index(src: &Src<'_>, at: usize) -> bool {
    if src.unit(at) != Some(u16::from(b'[')) {
        return false;
    }
    let mut i = at + 1;
    if !src.unit(i).is_some_and(is_digit) {
        return false;
    }
    while src.unit(i).is_some_and(is_digit) {
        i += 1;
    }
    src.unit(i) == Some(u16::from(b']'))
}

fn single_kind(ch: u16) -> Option<TokenKind> {
    Some(match ch {
        c if c == u16::from(b'(') => TokenKind::Open,
        c if c == u16::from(b')') => TokenKind::Close,
        c if c == u16::from(b',') => TokenKind::Comma,
        c if c == u16::from(b';') => TokenKind::Semicolon,
        c if c == u16::from(b'{') => TokenKind::Lbrace,
        c if c == u16::from(b'}') => TokenKind::Rbrace,
        _ => return None,
    })
}

fn unexpected(src: &Src<'_>, at: usize) -> FormulaError {
    FormulaError::new(
        format!("Unexpected character '{}'", src.char_at(at)),
        at as i64,
    )
}

fn ws_token(src: &Src<'_>, start: usize, end: usize) -> Token {
    plain(TokenKind::Ws, src, start, end)
}

fn plain(kind: TokenKind, src: &Src<'_>, start: usize, end: usize) -> Token {
    make_token(kind, src.slice(start, end), start, None, None, None, false)
}

fn structured_token(src: &Src<'_>, start: usize, end: usize, table: String) -> Token {
    make_token(
        TokenKind::Structured,
        src.slice(start, end),
        start,
        Some(TokenValue::Text(table)),
        None,
        None,
        false,
    )
}

fn make_token(
    kind: TokenKind,
    text: String,
    start: usize,
    value: Option<TokenValue>,
    prefix: Option<SheetPrefix>,
    reference: Option<RefSpec>,
    spill: bool,
) -> Token {
    Token {
        kind,
        text,
        start,
        value,
        prefix,
        reference,
        spill,
    }
}

fn skip_while(src: &Src<'_>, at: usize, pred: fn(u16) -> bool) -> usize {
    let mut i = at;
    while src.unit(i).is_some_and(pred) {
        i += 1;
    }
    i
}

fn find_unit(src: &Src<'_>, at: usize, target: u16) -> Option<usize> {
    src.units[at..]
        .iter()
        .position(|u| *u == target)
        .map(|p| at + p)
}

struct Src<'a> {
    raw: &'a str,
    units: Vec<u16>,
    byte_at: Vec<usize>,
}

impl<'a> Src<'a> {
    fn new(raw: &'a str) -> Self {
        let units: Vec<u16> = raw.encode_utf16().collect();
        let mut byte_at = vec![0usize; units.len() + 1];
        let mut ui = 0;
        for (bi, ch) in raw.char_indices() {
            let width = ch.len_utf16();
            byte_at[ui] = bi;
            if width == 2 {
                byte_at[ui + 1] = bi;
            }
            ui += width;
        }
        byte_at[units.len()] = raw.len();
        Self {
            raw,
            units,
            byte_at,
        }
    }

    fn len(&self) -> usize {
        self.units.len()
    }

    fn unit(&self, i: usize) -> Option<u16> {
        self.units.get(i).copied()
    }

    fn slice(&self, start: usize, end: usize) -> String {
        let start = start.min(self.byte_at.len() - 1);
        let end = end.min(self.byte_at.len() - 1);
        self.raw[self.byte_at[start]..self.byte_at[end]].to_owned()
    }

    fn char_at(&self, i: usize) -> String {
        self.unit(i)
            .and_then(|u| char::from_u32(u32::from(u)))
            .map(|c| c.to_string())
            .unwrap_or_else(|| "\u{FFFD}".to_owned())
    }

    fn starts_with(&self, at: usize, text: &str) -> bool {
        let bytes = text.as_bytes();
        if at + bytes.len() > self.units.len() {
            return false;
        }
        self.units[at..at + bytes.len()]
            .iter()
            .zip(bytes)
            .all(|(unit, byte)| *unit == u16::from(*byte))
    }
}

fn is_space(code: u16) -> bool {
    matches!(code, 32 | 9 | 13 | 10)
}

fn is_digit(code: u16) -> bool {
    (48..=57).contains(&code)
}

fn is_letter(code: u16) -> bool {
    (65..=90).contains(&code) || (97..=122).contains(&code)
}

fn is_name_start(code: u16) -> bool {
    is_letter(code) || code == 95 || code == 92 || code >= 0xA1
}

fn is_name_char(code: u16) -> bool {
    is_name_start(code) || is_digit(code) || code == 46 || code == 63
}

fn is_sheet_char(code: u16) -> bool {
    is_letter(code) || is_digit(code) || code == 95 || code == 46 || code >= 0xA1
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

fn split_chars(text: &str, at: usize) -> (&str, &str) {
    for (count, (idx, _)) in text.char_indices().enumerate() {
        if count == at {
            return (&text[..idx], &text[idx..]);
        }
    }
    (text, "")
}

/// JavaScript `Number(text)` for a lexer number literal.
fn parse_js_number(text: &str) -> f64 {
    let mut s = text.to_owned();
    if let Some(rest) = s.strip_prefix('.') {
        s = format!("0.{rest}");
    } else if let Some(rest) = s.strip_prefix("-.") {
        s = format!("-0.{rest}");
    } else if let Some(rest) = s.strip_prefix("+.") {
        s = format!("+0.{rest}");
    }
    if let Some(exp) = s.find(['e', 'E']) {
        if exp > 0 && s.as_bytes()[exp - 1] == b'.' {
            s.insert(exp, '0');
        }
    } else if s.ends_with('.') {
        s.push('0');
    }
    s.parse::<f64>().unwrap_or(f64::NAN)
}
