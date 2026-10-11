// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/address.ts (parseRange / parseAddress);
// changes listed in PROVENANCE.md.
//! Cell address and range parsing for Print_Area lift.

use crate::model::{CellAddress, CellRange};

/// Largest zero-based row index (1,048,576 rows).
pub const MAX_ROW: u32 = 1_048_575;
/// Largest zero-based column index (16,384 columns, `XFD`).
pub const MAX_COL: u32 = 16_383;

/// `0` -> `A`, `25` -> `Z`, `26` -> `AA`.
pub fn column_label(col: u32) -> String {
    let mut label = String::new();
    let mut n = col + 1;
    while n > 0 {
        let rem = (n - 1) % 26;
        label.insert(0, (65 + rem) as u8 as char);
        n = (n - 1) / 26;
    }
    label
}

/// `A` -> `0`, `AA` -> `26`; case-insensitive. Returns `None` for non-letters.
pub fn column_index(label: &str) -> Option<u32> {
    if label.is_empty() || label.len() > 3 || !label.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let mut n: u32 = 0;
    for ch in label.chars() {
        n = n * 26 + (ch.to_ascii_uppercase() as u32 - 64);
    }
    Some(n - 1)
}

/// Parses `A1` or `$A$1`; returns `None` for malformed or out-of-grid references.
pub fn parse_address(ref_: &str) -> Option<CellAddress> {
    let trimmed = ref_.trim();
    let bytes = trimmed.as_bytes();
    let mut i = 0;
    if i < bytes.len() && bytes[i] == b'$' {
        i += 1;
    }
    let col_start = i;
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i == col_start || i - col_start > 3 {
        return None;
    }
    let col_label = &trimmed[col_start..i];
    if i < bytes.len() && bytes[i] == b'$' {
        i += 1;
    }
    let row_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i != bytes.len() || row_start == i || i - row_start > 7 {
        return None;
    }
    let col = column_index(col_label)?;
    let row: u32 = trimmed[row_start..i].parse().ok()?;
    if row == 0 {
        return None;
    }
    let row = row - 1;
    if col > MAX_COL || row > MAX_ROW {
        return None;
    }
    Some(CellAddress { row, col })
}

fn normalize_range(range: CellRange) -> CellRange {
    CellRange {
        start: CellAddress {
            row: range.start.row.min(range.end.row),
            col: range.start.col.min(range.end.col),
        },
        end: CellAddress {
            row: range.start.row.max(range.end.row),
            col: range.start.col.max(range.end.col),
        },
    }
}

/// Formats a zero-based address as `A1`.
pub fn format_address(addr: &CellAddress) -> String {
    format!("{}{}", column_label(addr.col), addr.row + 1)
}

/// Formats a range as `A1` or `A1:B2` (whole-row / whole-column refs stay expanded).
pub fn format_range(range: &CellRange) -> String {
    if range.start == range.end {
        format_address(&range.start)
    } else {
        format!(
            "{}:{}",
            format_address(&range.start),
            format_address(&range.end)
        )
    }
}

/// Parses `A1:B2`, a single cell `A1`, whole columns `A:C` or whole rows `1:3`.
pub fn parse_range(ref_: &str) -> Option<CellRange> {
    let trimmed = ref_.trim();
    let parts: Vec<&str> = trimmed.split(':').collect();
    if parts.len() == 1 {
        let cell = parse_address(parts[0])?;
        return Some(CellRange {
            start: cell,
            end: cell,
        });
    }
    if parts.len() != 2 {
        return None;
    }
    let a = parts[0].replace('$', "");
    let b = parts[1].replace('$', "");
    if a.chars().all(|c| c.is_ascii_alphabetic()) && b.chars().all(|c| c.is_ascii_alphabetic()) {
        let c1 = column_index(&a)?;
        let c2 = column_index(&b)?;
        return Some(normalize_range(CellRange {
            start: CellAddress { row: 0, col: c1 },
            end: CellAddress {
                row: MAX_ROW,
                col: c2,
            },
        }));
    }
    if a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit()) {
        let r1: u32 = a.parse().ok()?;
        let r2: u32 = b.parse().ok()?;
        if r1 == 0 || r2 == 0 {
            return None;
        }
        return Some(normalize_range(CellRange {
            start: CellAddress {
                row: r1 - 1,
                col: 0,
            },
            end: CellAddress {
                row: r2 - 1,
                col: MAX_COL,
            },
        }));
    }
    let start = parse_address(&a)?;
    let end = parse_address(&b)?;
    Some(normalize_range(CellRange { start, end }))
}
