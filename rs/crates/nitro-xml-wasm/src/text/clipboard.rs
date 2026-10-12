// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/edit/clipboard-text.ts
// (toTsv/parseTsv/quoteField) and escapeHtml from clipboard-html.ts; changes listed in PROVENANCE.md.
//! Clipboard helpers for the text surface and grid paste (#55).
//!
//! While editing, upstream lets the textarea own the system clipboard (plain
//! text). Nitro mirrors that with [`copy_plain`] / [`cut_plain`] / [`paste_plain`].
//! TSV encode/decode is ported for paste of multi-cell system clipboard text
//! into the field and for grid copy (#43 / #69).

use super::selection::TextField;

/// System-clipboard payload used by grid copy when not editing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardPayload {
    pub tsv: String,
    pub html: String,
    pub cut: bool,
}

pub fn copy_plain(field: &TextField) -> String {
    field.selected_text().to_string()
}

pub fn cut_plain(field: &mut TextField) -> String {
    let t = field.selected_text().to_string();
    if !t.is_empty() {
        field.insert("");
    }
    t
}

pub fn paste_plain(field: &mut TextField, text: &str) -> bool {
    field.insert(text);
    true
}

/// Quotes a TSV field the way Excel does when it holds a tab, line break or quote.
fn quote_field(text: &str) -> String {
    if text
        .bytes()
        .any(|b| matches!(b, b'\t' | b'\r' | b'\n' | b'"'))
    {
        let mut out = String::with_capacity(text.len() + 2);
        out.push('"');
        for ch in text.chars() {
            if ch == '"' {
                out.push_str("\"\"");
            } else {
                out.push(ch);
            }
        }
        out.push('"');
        out
    } else {
        text.to_string()
    }
}

/// Tab-separated text for the system clipboard (Excel style: CRLF rows, trailing CRLF).
pub fn to_tsv(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (ri, row) in rows.iter().enumerate() {
        if ri > 0 {
            out.push_str("\r\n");
        }
        for (ci, cell) in row.iter().enumerate() {
            if ci > 0 {
                out.push('\t');
            }
            out.push_str(&quote_field(cell));
        }
    }
    out.push_str("\r\n");
    out
}

/// Parses tab-separated clipboard text. Quoted fields may hold tabs, line breaks and doubled
/// quotes; one trailing line break is ignored. Rows are padded to the same width.
pub fn parse_tsv(text: &str) -> Vec<Vec<String>> {
    let body = {
        let mut s = text.replace("\r\n", "\n").replace('\r', "\n");
        if s.ends_with('\n') {
            s.pop();
        }
        s
    };
    if body.is_empty() {
        return if text.is_empty() {
            Vec::new()
        } else {
            vec![vec![String::new()]]
        };
    }
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0usize;
    let mut at_field_start = true;
    while i < chars.len() {
        let ch = chars[i];
        if at_field_start && ch == '"' {
            if let Some(close) = find_closing_quote(&chars, i + 1) {
                let next = chars.get(close + 1).copied();
                if next.is_none() || next == Some('\t') || next == Some('\n') {
                    field = chars[i + 1..close]
                        .iter()
                        .collect::<String>()
                        .replace("\"\"", "\"");
                    i = close + 1;
                    at_field_start = false;
                    continue;
                }
            }
        }
        at_field_start = false;
        if ch == '\t' {
            row.push(std::mem::take(&mut field));
            at_field_start = true;
        } else if ch == '\n' {
            row.push(std::mem::take(&mut field));
            rows.push(std::mem::take(&mut row));
            at_field_start = true;
        } else {
            field.push(ch);
        }
        i += 1;
    }
    row.push(field);
    rows.push(row);
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    for r in &mut rows {
        while r.len() < width {
            r.push(String::new());
        }
    }
    rows
}

fn find_closing_quote(chars: &[char], from: usize) -> Option<usize> {
    let mut i = from;
    while i < chars.len() {
        if chars[i] != '"' {
            i += 1;
            continue;
        }
        if chars.get(i + 1) == Some(&'"') {
            i += 2;
            continue;
        }
        return Some(i);
    }
    None
}

pub fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Minimal HTML table for a plain rectangular paste (no cell styles — full styled
/// `toHtml` stays with grid clipboard `#69`).
pub fn plain_html_table(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let mut out = String::from("<table>");
    for row in rows {
        out.push_str("<tr>");
        for cell in row {
            out.push_str("<td>");
            out.push_str(&escape_html(cell));
            out.push_str("</td>");
        }
        out.push_str("</tr>");
    }
    out.push_str("</table>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_tsv_roundtrip_quoted_tab_and_newline() {
        // Catches: Excel-style TSV must quote fields that hold tab/newline/quote.
        let rows = vec![
            vec!["a\tb".into(), "c\"d".into()],
            vec!["e\nf".into(), "g".into()],
        ];
        let tsv = to_tsv(&rows);
        let parsed = parse_tsv(&tsv);
        assert_eq!(parsed, rows);
    }

    #[test]
    fn catches_cut_clears_selection_and_returns_text() {
        // Catches: cut must return selected plain text and collapse to empty selection.
        let mut f = TextField::new("hello");
        f.set_caret(1);
        f.extend_to(4);
        let t = cut_plain(&mut f);
        assert_eq!(t, "ell");
        assert_eq!(f.text(), "ho");
        assert!(f.selection().is_collapsed());
    }
}
