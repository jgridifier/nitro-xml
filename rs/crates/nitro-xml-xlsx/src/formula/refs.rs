// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/structured.ts, ref-text.ts,
// and referencedRanges/referenceSpans in transform.ts; changes listed in PROVENANCE.md.
//! Structured references, reference text, and the ranges a formula points at.

use crate::address::column_label;
use crate::formula::ast::{
    FormulaError, RefKind, RefSpec, SheetPrefix, StructuredRef, StructuredSpecial,
};
use crate::formula::lexer::{tokenize, TokenKind};
use crate::model::{CellAddress, CellRange};

/// Parses `Table1[...]` (or a bare `[...]` inside a table) into its parts.
pub fn parse_structured(text: &str, table_name: &str) -> Result<StructuredRef, FormulaError> {
    let open = text.find('[').map(|i| i + 1).unwrap_or(0);
    let end = text.len().saturating_sub(1);
    let inner = if open <= end { &text[open..end] } else { "" };
    let mut out = StructuredRef {
        table: if table_name.is_empty() {
            None
        } else {
            Some(table_name.to_owned())
        },
        specials: Vec::new(),
        column: None,
        column2: None,
    };
    if !inner.contains('[') {
        handle_item(&mut out, text, inner)?;
        return Ok(out);
    }
    for item in split_items(inner) {
        let stripped = strip(&item).to_lowercase();
        if looks_like_bracket_range(&item) && lookup_special(&stripped).is_none() {
            add_columns(&mut out, &item);
            continue;
        }
        handle_item(&mut out, text, &item)?;
    }
    Ok(out)
}

fn handle_item(out: &mut StructuredRef, text: &str, raw: &str) -> Result<(), FormulaError> {
    let mut item = raw.trim().to_owned();
    if item.is_empty() {
        return Ok(());
    }
    if let Some(rest) = item.strip_prefix('@') {
        out.specials.push(StructuredSpecial::ThisRow);
        item = rest.trim().to_owned();
        if item.is_empty() {
            return Ok(());
        }
    }
    if let Some(special) = lookup_special(&strip(&item).to_lowercase()) {
        out.specials.push(special);
        return Ok(());
    }
    if out.column.is_some() {
        return Err(FormulaError::new(
            format!("Invalid structured reference {text}"),
            -1,
        ));
    }
    add_columns(out, &item);
    Ok(())
}

fn add_columns(out: &mut StructuredRef, spec: &str) {
    if let Some((left, right)) = strict_column_range(spec) {
        out.column = Some(left);
        out.column2 = Some(right);
        return;
    }
    out.column = Some(unescape_sq(strip(spec)));
}

fn lookup_special(lower: &str) -> Option<StructuredSpecial> {
    Some(match lower {
        "#all" => StructuredSpecial::All,
        "#data" => StructuredSpecial::Data,
        "#headers" => StructuredSpecial::Headers,
        "#totals" => StructuredSpecial::Totals,
        "#this row" => StructuredSpecial::ThisRow,
        _ => return None,
    })
}

fn split_items(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut items = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\'' {
            current.push(ch);
            if i + 1 < chars.len() {
                current.push(chars[i + 1]);
            }
            i += 2;
            continue;
        }
        if ch == '[' {
            depth += 1;
        }
        if ch == ']' {
            depth -= 1;
        }
        if ch == ',' && depth == 0 {
            items.push(current.trim().to_owned());
            current.clear();
            i += 1;
            continue;
        }
        current.push(ch);
        i += 1;
    }
    if !current.trim().is_empty() {
        items.push(current.trim().to_owned());
    }
    items
}

fn strip(item: &str) -> &str {
    if item.starts_with('[') && item.ends_with(']') && item.len() >= 2 {
        &item[1..item.len() - 1]
    } else {
        item
    }
}

fn unescape_sq(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\'' && i + 1 < chars.len() {
            let next = chars[i + 1];
            if !is_line_terminator(next) {
                out.push(next);
                i += 2;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn is_line_terminator(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn looks_like_bracket_range(item: &str) -> bool {
    if item.chars().any(is_line_terminator) || !item.starts_with('[') || !item.ends_with(']') {
        return false;
    }
    let chars: Vec<char> = item.chars().collect();
    for (i, ch) in chars.iter().enumerate() {
        if *ch != ']' {
            continue;
        }
        let mut j = i + 1;
        while j < chars.len() && is_js_space(chars[j]) {
            j += 1;
        }
        if chars.get(j) != Some(&':') {
            continue;
        }
        j += 1;
        while j < chars.len() && is_js_space(chars[j]) {
            j += 1;
        }
        if chars.get(j) == Some(&'[') {
            return true;
        }
    }
    false
}

fn strict_column_range(spec: &str) -> Option<(String, String)> {
    if spec.chars().any(is_line_terminator) {
        return None;
    }
    let chars: Vec<char> = spec.chars().collect();
    let (left, mut i) = read_bracket_inner(&chars, 0)?;
    while i < chars.len() && is_js_space(chars[i]) {
        i += 1;
    }
    if chars.get(i) != Some(&':') {
        return None;
    }
    i += 1;
    while i < chars.len() && is_js_space(chars[i]) {
        i += 1;
    }
    let (right, end) = read_bracket_inner(&chars, i)?;
    if end != chars.len() {
        return None;
    }
    Some((unescape_sq(&left), unescape_sq(&right)))
}

fn read_bracket_inner(chars: &[char], open: usize) -> Option<(String, usize)> {
    if chars.get(open) != Some(&'[') {
        return None;
    }
    let mut inner = String::new();
    let mut i = open + 1;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\'' {
            let next = *chars.get(i + 1)?;
            if is_line_terminator(next) {
                return None;
            }
            inner.push(ch);
            inner.push(next);
            i += 2;
            continue;
        }
        if ch == ']' {
            return Some((inner, i + 1));
        }
        inner.push(ch);
        i += 1;
    }
    None
}

fn is_js_space(ch: char) -> bool {
    matches!(
        ch,
        ' ' | '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// Formats a reference spec (without its sheet prefix).
pub fn format_ref_spec(spec: &RefSpec) -> String {
    let start = corner_text(&spec.start, spec.kind);
    if spec.kind == RefKind::Cell {
        return start;
    }
    format!("{start}:{}", corner_text(&spec.end, spec.kind))
}

fn corner_text(corner: &crate::formula::ast::RefCorner, kind: RefKind) -> String {
    match kind {
        RefKind::Cols => format!(
            "{}{}",
            if corner.col_abs { "$" } else { "" },
            column_label(corner.col as u32)
        ),
        RefKind::Rows => format!(
            "{}{}",
            if corner.row_abs { "$" } else { "" },
            corner.row + 1
        ),
        RefKind::Cell | RefKind::Area => format!(
            "{}{}{}{}",
            if corner.col_abs { "$" } else { "" },
            column_label(corner.col as u32),
            if corner.row_abs { "$" } else { "" },
            corner.row + 1
        ),
    }
}

/// The text of a reference token rewritten to `spec` (`#REF!` when `spec` is absent).
pub fn rewrite_ref(prefix_text: &str, spill: bool, spec: Option<&RefSpec>) -> String {
    match spec {
        None => format!("{prefix_text}#REF!"),
        Some(spec) => {
            let body = format_ref_spec(spec);
            if spill {
                format!("{prefix_text}{body}#")
            } else {
                format!("{prefix_text}{body}")
            }
        }
    }
}

/// Same grid rectangle, ignoring absolute markers (upstream `sameSpec`).
pub fn same_spec(a: &RefSpec, b: &RefSpec) -> bool {
    a.kind == b.kind
        && a.start.row == b.start.row
        && a.start.col == b.start.col
        && a.end.row == b.end.row
        && a.end.col == b.end.col
}

/// Sheet prefix text (`Sheet1!`, `'My Sheet'!`), quoted when Excel would misread the name.
pub fn prefix_text(prefix: &SheetPrefix) -> String {
    let sheets = match &prefix.sheet2 {
        Some(sheet2) => format!("{}:{sheet2}", prefix.sheet),
        None => prefix.sheet.clone(),
    };
    let book = match &prefix.book {
        Some(book) => format!("[{book}]"),
        None => String::new(),
    };
    let quote = needs_quote(&prefix.sheet) || prefix.sheet2.as_deref().is_some_and(needs_quote);
    if quote {
        let body = format!("{book}{sheets}").replace('\'', "''");
        format!("'{body}'!")
    } else {
        format!("{book}{sheets}!")
    }
}

fn needs_quote(name: &str) -> bool {
    !is_plain_ident(name) || looks_like_a1(name) || looks_like_r1c1(name) || is_logical(name)
}

fn is_plain_ident(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    is_ident_start(first) && chars.all(is_ident_cont)
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || (ch as u32) >= 0xA1
}

fn is_ident_cont(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit() || ch == '.'
}

fn looks_like_a1(name: &str) -> bool {
    let bytes = name.as_bytes();
    let mut i = 0;
    while i < bytes.len() && i < 3 && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    i > 0 && i < bytes.len() && bytes[i..].iter().all(|b| b.is_ascii_digit())
}

fn looks_like_r1c1(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let first = bytes[0].to_ascii_uppercase();
    if first == b'C' {
        return bytes[1..].iter().all(|b| b.is_ascii_digit());
    }
    if first != b'R' {
        return false;
    }
    let mut i = 1;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == bytes.len() {
        return true;
    }
    if !bytes[i].eq_ignore_ascii_case(&b'C') {
        return false;
    }
    i += 1;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    i == bytes.len()
}

fn is_logical(name: &str) -> bool {
    name.eq_ignore_ascii_case("TRUE") || name.eq_ignore_ascii_case("FALSE")
}

/// One directly referenced range (external-workbook refs are omitted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferencedRange {
    pub sheet: String,
    pub range: CellRange,
}

impl ReferencedRange {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "sheet": self.sheet,
            "range": self.range.to_json(),
        })
    }
}

/// A reference token's character span in the formula text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceSpan {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub sheet: String,
    pub range: CellRange,
}

impl ReferenceSpan {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "start": self.start,
            "end": self.end,
            "text": self.text,
            "sheet": self.sheet,
            "range": self.range.to_json(),
        })
    }
}

/// Ranges a formula references directly. An unparsable formula yields nothing.
pub fn referenced_ranges(formula: &str, formula_sheet: &str) -> Vec<ReferencedRange> {
    let Ok(tokens) = tokenize(formula) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for token in tokens {
        if token.kind != TokenKind::Ref {
            continue;
        }
        let Some(spec) = token.reference else {
            continue;
        };
        if token
            .prefix
            .as_ref()
            .is_some_and(|prefix| prefix.book.is_some())
        {
            continue;
        }
        let sheet = token
            .prefix
            .as_ref()
            .map(|prefix| prefix.sheet.clone())
            .unwrap_or_else(|| formula_sheet.to_owned());
        out.push(ReferencedRange {
            sheet,
            range: normalize_spec(&spec),
        });
    }
    out
}

/// Every A1 reference with its character span. `#REF!`, 3D and external refs are skipped.
pub fn reference_spans(formula: &str, formula_sheet: &str) -> Vec<ReferenceSpan> {
    let Ok(tokens) = tokenize(formula) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for token in tokens {
        if token.kind != TokenKind::Ref {
            continue;
        }
        let Some(spec) = token.reference else {
            continue;
        };
        if let Some(prefix) = &token.prefix {
            if prefix.book.is_some() || prefix.sheet2.is_some() {
                continue;
            }
        }
        let sheet = token
            .prefix
            .as_ref()
            .map(|prefix| prefix.sheet.clone())
            .unwrap_or_else(|| formula_sheet.to_owned());
        let start = token.start;
        let end = start + token.text.encode_utf16().count();
        out.push(ReferenceSpan {
            start,
            end,
            text: token.text,
            sheet,
            range: normalize_spec(&spec),
        });
    }
    out
}

fn normalize_spec(spec: &RefSpec) -> CellRange {
    let start_row = spec.start.row.max(0) as u32;
    let start_col = spec.start.col.max(0) as u32;
    let end_row = spec.end.row.max(0) as u32;
    let end_col = spec.end.col.max(0) as u32;
    CellRange {
        start: CellAddress {
            row: start_row.min(end_row),
            col: start_col.min(end_col),
        },
        end: CellAddress {
            row: start_row.max(end_row),
            col: start_col.max(end_col),
        },
    }
}
