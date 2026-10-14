// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/cf-evaluator.ts
// (literal / relative-ref expression path used by CF samples); changes listed in PROVENANCE.md.
//! Minimal CF expression evaluator for soft-corpus formulas (no full calc engine).

use crate::address::{column_index, parse_address};
use crate::model::{ActiveWorksheet, DumpCellValue};
use crate::sheet_access::{get_cell, CellIndex};

#[derive(Clone, Copy)]
struct A1Ref {
    row: u32,
    col: u32,
    abs_row: bool,
    abs_col: bool,
}

fn parse_ref(text: &str) -> Option<A1Ref> {
    let trimmed = text.trim();
    let bytes = trimmed.as_bytes();
    let mut i = 0;
    let mut abs_col = false;
    if i < bytes.len() && bytes[i] == b'$' {
        abs_col = true;
        i += 1;
    }
    let col_start = i;
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i == col_start {
        return None;
    }
    let col = column_index(&trimmed[col_start..i])?;
    let mut abs_row = false;
    if i < bytes.len() && bytes[i] == b'$' {
        abs_row = true;
        i += 1;
    }
    let row_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i != bytes.len() || row_start == i {
        return None;
    }
    let excel_row: u32 = trimmed[row_start..i].parse().ok()?;
    if excel_row == 0 {
        return None;
    }
    Some(A1Ref {
        row: excel_row - 1,
        col,
        abs_row,
        abs_col,
    })
}

fn relocate(r: A1Ref, d_row: i32, d_col: i32) -> Option<(u32, u32)> {
    let row = if r.abs_row {
        r.row
    } else {
        let v = r.row as i32 + d_row;
        if v < 0 {
            return None;
        }
        v as u32
    };
    let col = if r.abs_col {
        r.col
    } else {
        let v = r.col as i32 + d_col;
        if v < 0 {
            return None;
        }
        v as u32
    };
    Some((row, col))
}

fn cell_text(sheet: &ActiveWorksheet, index: &CellIndex, row: u32, col: u32) -> String {
    match get_cell(sheet, index, row, col).map(|c| &c.value) {
        Some(DumpCellValue::String(s)) => s.clone(),
        Some(DumpCellValue::Number(n)) => {
            if n.fract() == 0.0 {
                format!("{n:.0}")
            } else {
                n.to_string()
            }
        }
        Some(DumpCellValue::NumberRaw(s)) => s.clone(),
        Some(DumpCellValue::Bool(b)) => {
            if *b {
                "TRUE".into()
            } else {
                "FALSE".into()
            }
        }
        Some(DumpCellValue::Error(e)) => e.clone(),
        _ => String::new(),
    }
}

fn unquote(s: &str) -> Option<String> {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        Some(t[1..t.len() - 1].replace("\"\"", "\""))
    } else {
        None
    }
}

fn eq_cell_string(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    formula_ref: &str,
    expected: &str,
    d_row: i32,
    d_col: i32,
) -> Option<bool> {
    let r = parse_ref(formula_ref)?;
    let (row, col) = relocate(r, d_row, d_col)?;
    Some(cell_text(sheet, index, row, col).eq_ignore_ascii_case(expected))
}

/// Evaluate a small subset of CF `expression` formulas used by the soft corpus.
pub fn eval_cf_expression(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    formula: &str,
    anchor_row: u32,
    anchor_col: u32,
    row: u32,
    col: u32,
) -> bool {
    let text = formula.trim().trim_start_matches('=');
    let d_row = row as i32 - anchor_row as i32;
    let d_col = col as i32 - anchor_col as i32;

    // MOD(ROW($T2),2)=1  (banded rows)
    if let Some(caps) = regex_mod_row(text) {
        let r = parse_ref(caps.0).unwrap_or(A1Ref {
            row: anchor_row,
            col: anchor_col,
            abs_row: false,
            abs_col: true,
        });
        let (erow, _) = relocate(r, d_row, d_col).unwrap_or((row, col));
        let excel_row = erow + 1;
        let rem = excel_row % caps.1;
        return if caps.2 { rem == caps.3 } else { rem != caps.3 };
    }

    // OR(a,b,...) of cell="LIT"
    if let Some(inner) = text.strip_prefix("OR(").and_then(|s| s.strip_suffix(')')) {
        return split_top_commas(inner)
            .into_iter()
            .any(|part| eval_cf_expression(sheet, index, part, anchor_row, anchor_col, row, col));
    }
    if let Some(inner) = text.strip_prefix("AND(").and_then(|s| s.strip_suffix(')')) {
        return split_top_commas(inner)
            .into_iter()
            .all(|part| eval_cf_expression(sheet, index, part, anchor_row, anchor_col, row, col));
    }

    // $H$4="MONEY" / $E9<>""
    if let Some((left, right)) = split_cmp(text, "<>") {
        if right.trim() == "\"\"" {
            if let Some(r) = parse_ref(left) {
                if let Some((rr, cc)) = relocate(r, d_row, d_col) {
                    return !cell_text(sheet, index, rr, cc).is_empty();
                }
            }
        }
        return false;
    }
    if let Some((left, right)) = split_cmp(text, "=") {
        if let Some(lit) = unquote(right) {
            return eq_cell_string(sheet, index, left, &lit, d_row, d_col).unwrap_or(false);
        }
        // bare TRUE/FALSE or number equality against cell
        if let Some(addr) = parse_address(left.trim()) {
            let (rr, cc) = if left.contains('$') {
                // parse_ref for abs flags
                if let Some(r) = parse_ref(left) {
                    relocate(r, d_row, d_col).unwrap_or((addr.row, addr.col))
                } else {
                    (addr.row, addr.col)
                }
            } else if let Some(r) = parse_ref(left) {
                relocate(r, d_row, d_col).unwrap_or((addr.row, addr.col))
            } else {
                (addr.row, addr.col)
            };
            let cell = cell_text(sheet, index, rr, cc);
            return cell.eq_ignore_ascii_case(right.trim());
        }
    }

    false
}

fn regex_mod_row(text: &str) -> Option<(&str, u32, bool, u32)> {
    // MOD(ROW(ref),n)=k  or <>
    let upper = text.to_ascii_uppercase();
    let start = upper.find("MOD(ROW(")?;
    let after = &text[start + "MOD(ROW(".len()..];
    let close_row = after.find(')')?;
    let pref = &after[..close_row];
    let rest = &after[close_row + 1..];
    // ,n)=k
    let rest = rest.strip_prefix(',')?;
    let close_mod = rest.find(')')?;
    let n: u32 = rest[..close_mod].trim().parse().ok()?;
    let cmp = rest[close_mod + 1..].trim();
    if let Some(k) = cmp.strip_prefix('=') {
        let k: u32 = k.trim().parse().ok()?;
        return Some((pref, n, true, k));
    }
    if let Some(k) = cmp.strip_prefix("<>") {
        let k: u32 = k.trim().parse().ok()?;
        return Some((pref, n, false, k));
    }
    None
}

fn split_top_commas(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[start..].trim());
    out
}

fn split_cmp<'a>(s: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    // Prefer longest op; scan outside quotes.
    let bytes = s.as_bytes();
    let mut in_str = false;
    let mut i = 0;
    while i + op.len() <= bytes.len() {
        let ch = bytes[i];
        if ch == b'"' {
            in_str = !in_str;
            i += 1;
            continue;
        }
        if !in_str && s[i..].starts_with(op) {
            return Some((s[..i].trim(), s[i + op.len()..].trim()));
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CellAddress, DumpCell, SheetState};
    use crate::sheet_props::default_sheet_view;
    use std::collections::BTreeMap;

    fn sheet_h4(money: &str) -> ActiveWorksheet {
        ActiveWorksheet {
            name: "t".into(),
            sheet_id: 1,
            state: SheetState::Visible,
            part_name: None,
            default_row_height: 15.0,
            default_col_width: None,
            format: None,
            view: default_sheet_view(),
            columns: vec![],
            merges: vec![],
            row_info: BTreeMap::new(),
            cells: vec![DumpCell {
                row: 3,
                col: 7,
                value: DumpCellValue::String(money.into()),
                type_tag: None,
                formula: None,
                style_id: None,
                rich_text: None,
                array_range: None,
                legacy_formula: None,
                dynamic_array: None,
                spill_anchor: None,
                cell_metadata: None,
                value_metadata: None,
            }],
            data_validations: vec![],
            comment_addresses: vec![],
            hyperlinks: vec![],
            ext_lst: vec![],
            sparkline_groups: vec![],
            drawings: vec![],
            conditional_formats: vec![],
            dimension: None,
        }
    }

    #[test]
    fn expression_h4_money_and_mod_row() {
        // Catches: CF expression `$H$4="MONEY"` / `MOD(ROW($T2),2)=1` never match so soft fills vanish.
        let sheet = sheet_h4("MONEY");
        let index = CellIndex::build(&sheet);
        assert!(eval_cf_expression(
            &sheet,
            &index,
            "$H$4=\"MONEY\"",
            0,
            0,
            8,
            0
        ));
        assert!(!eval_cf_expression(
            &sheet,
            &index,
            "OR($H$4=\"DEC\",$H$4=\"PERC\")",
            0,
            0,
            8,
            0
        ));
        assert!(eval_cf_expression(
            &sheet,
            &index,
            "MOD(ROW($T2),2)=1",
            1,
            19,
            2,
            19
        )); // Excel row 3
        assert!(!eval_cf_expression(
            &sheet,
            &index,
            "MOD(ROW($T2),2)=1",
            1,
            19,
            1,
            19
        )); // Excel row 2
        let _ = CellAddress { row: 0, col: 0 };
    }
}
