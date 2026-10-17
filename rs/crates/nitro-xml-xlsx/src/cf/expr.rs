// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/cf-evaluator.ts
// (literal / relative-ref expression path used by CF samples); changes listed in PROVENANCE.md.
//! Minimal CF expression evaluator for soft-corpus formulas (no full calc engine).

use crate::address::column_index;
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

fn cell_number_value(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    row: u32,
    col: u32,
) -> Option<f64> {
    match get_cell(sheet, index, row, col).map(|c| &c.value) {
        Some(DumpCellValue::Number(n)) if n.is_finite() => Some(*n),
        Some(DumpCellValue::NumberRaw(s)) => s.trim().parse::<f64>().ok().filter(|n| n.is_finite()),
        Some(DumpCellValue::Bool(b)) => Some(if *b { 1.0 } else { 0.0 }),
        Some(DumpCellValue::String(s)) => s.trim().parse::<f64>().ok().filter(|n| n.is_finite()),
        // Excel formula blanks → 0 (Compare-to-totals empty cells still enter B4<=$F4*0.2).
        Some(DumpCellValue::Null) | None => Some(0.0),
        _ => None,
    }
}

/// Resolve a CF threshold / operand formula to a number (literal or A1 cell ref).
///
/// Formula-type `cfvo` values like `$T$5` are evaluated at the format anchor with
/// no relative shift (upstream `thresholdValue` → `run` at `ranges[0].start`).
pub fn eval_cf_formula_number(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    formula: &str,
) -> Option<f64> {
    let text = formula.trim().trim_start_matches('=');
    if text.is_empty() {
        return None;
    }
    if let Ok(n) = text.parse::<f64>() {
        if n.is_finite() {
            return Some(n);
        }
    }
    let r = parse_ref(text)?;
    let (row, col) = relocate(r, 0, 0)?;
    cell_number_value(sheet, index, row, col)
}

/// Resolve a CF `cellIs` operand to display text (quoted literal or A1 cell).
pub fn eval_cf_formula_text(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    formula: &str,
) -> Option<String> {
    let text = formula.trim().trim_start_matches('=');
    if text.is_empty() {
        return None;
    }
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        return Some(text[1..text.len() - 1].replace("\"\"", "\""));
    }
    let r = parse_ref(text)?;
    let (row, col) = relocate(r, 0, 0)?;
    Some(cell_text(sheet, index, row, col))
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

    // MONTH(A3)=MONTH(TODAY())
    if let Some((left, right)) = split_cmp(text, "=") {
        if let (Some(a), Some(b)) = (
            month_of(sheet, index, left, d_row, d_col),
            month_of(sheet, index, right, d_row, d_col),
        ) {
            return a == b;
        }
    }

    // Numeric comparisons: B4<=$F4*0.2, D3<500, B4>=$F4*0.3
    for op in ["<=", ">=", "<>", "<", ">", "="] {
        if let Some((left, right)) = split_cmp(text, op) {
            if op == "<>" && right.trim() == "\"\"" {
                if let Some(r) = parse_ref(left) {
                    if let Some((rr, cc)) = relocate(r, d_row, d_col) {
                        return !cell_text(sheet, index, rr, cc).is_empty();
                    }
                }
                return false;
            }
            if op == "=" {
                if let Some(lit) = unquote(right) {
                    return eq_cell_string(sheet, index, left, &lit, d_row, d_col).unwrap_or(false);
                }
                // C3=C3 (cell vs cell text/number)
                if let (Some(l), Some(r)) = (
                    eval_side_text(sheet, index, left, d_row, d_col),
                    eval_side_text(sheet, index, right, d_row, d_col),
                ) {
                    return l.eq_ignore_ascii_case(&r);
                }
            }
            if let (Some(l), Some(r)) = (
                eval_num_side(sheet, index, left, d_row, d_col),
                eval_num_side(sheet, index, right, d_row, d_col),
            ) {
                return match op {
                    "<=" => l <= r,
                    ">=" => l >= r,
                    "<" => l < r,
                    ">" => l > r,
                    "=" => l == r,
                    "<>" => l != r,
                    _ => false,
                };
            }
            if op == "=" || op == "<>" {
                // fall through only for string-ish equals already handled
            } else {
                return false;
            }
        }
    }

    // Bare cell truthiness: `$H3` (Customers2).
    if let Some(r) = parse_ref(text) {
        if let Some((rr, cc)) = relocate(r, d_row, d_col) {
            return cell_truthy(sheet, index, rr, cc);
        }
    }

    false
}

fn eval_num_side(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    expr: &str,
    d_row: i32,
    d_col: i32,
) -> Option<f64> {
    let expr = expr.trim();
    if let Some((a, b)) = split_cmp(expr, "*") {
        let left = eval_num_atom(sheet, index, a, d_row, d_col)?;
        let right = eval_num_atom(sheet, index, b, d_row, d_col)?;
        return Some(left * right);
    }
    eval_num_atom(sheet, index, expr, d_row, d_col)
}

fn eval_num_atom(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    expr: &str,
    d_row: i32,
    d_col: i32,
) -> Option<f64> {
    let expr = expr.trim();
    if let Ok(n) = expr.parse::<f64>() {
        if n.is_finite() {
            return Some(n);
        }
    }
    let r = parse_ref(expr)?;
    let (row, col) = relocate(r, d_row, d_col)?;
    cell_number_value(sheet, index, row, col)
}

fn eval_side_text(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    expr: &str,
    d_row: i32,
    d_col: i32,
) -> Option<String> {
    let expr = expr.trim();
    if let Some(lit) = unquote(expr) {
        return Some(lit);
    }
    let r = parse_ref(expr)?;
    let (row, col) = relocate(r, d_row, d_col)?;
    Some(cell_text(sheet, index, row, col))
}

fn cell_truthy(sheet: &ActiveWorksheet, index: &CellIndex, row: u32, col: u32) -> bool {
    match get_cell(sheet, index, row, col).map(|c| &c.value) {
        Some(DumpCellValue::Bool(b)) => *b,
        Some(DumpCellValue::Number(n)) => *n != 0.0,
        Some(DumpCellValue::NumberRaw(s)) => s.trim().parse::<f64>().ok().is_some_and(|n| n != 0.0),
        _ => false,
    }
}

/// Excel month 1–12 from `MONTH(ref)` / `MONTH(TODAY())`, or None when not a month form.
fn month_of(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    expr: &str,
    d_row: i32,
    d_col: i32,
) -> Option<u32> {
    let upper = expr.trim().to_ascii_uppercase();
    if upper == "MONTH(TODAY())" {
        // Match the host clock; Products3 soft/view-IR only lights October rows when run in October.
        let now = chrono_lite_month();
        return Some(now);
    }
    let inner = upper.strip_prefix("MONTH(")?.strip_suffix(')')?.trim();
    // Preserve original casing for the ref inside MONTH(...)
    let raw = expr.trim();
    let start = raw.find('(')? + 1;
    let end = raw.rfind(')')?;
    let pref = raw[start..end].trim();
    let _ = inner;
    let r = parse_ref(pref)?;
    let (row, col) = relocate(r, d_row, d_col)?;
    let serial = cell_number_value(sheet, index, row, col)?;
    excel_month(serial)
}

fn excel_month(serial: f64) -> Option<u32> {
    // Excel 1900 date system (serial 1 = 1900-01-01), including the fictitious 1900-02-29.
    if !serial.is_finite() || serial < 1.0 {
        return None;
    }
    let mut rem = serial.floor() as i64;
    let mut y = 1900i32;
    while y <= 2200 {
        let leap = y == 1900 || (y % 4 == 0 && (y % 100 != 0 || y % 400 == 0));
        let dim = [
            31,
            if leap { 29 } else { 28 },
            31,
            30,
            31,
            30,
            31,
            31,
            30,
            31,
            30,
            31,
        ];
        for (m, &d) in dim.iter().enumerate() {
            if rem <= d as i64 {
                return Some((m + 1) as u32);
            }
            rem -= d as i64;
        }
        y += 1;
    }
    None
}

fn chrono_lite_month() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86400) as i64;
    // Unix epoch → Excel serial (date1904=false): 25569 = 1970-01-01.
    excel_month((days + 25569) as f64).unwrap_or(1)
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
    fn formula_number_resolves_absolute_cell_ref() {
        // Catches: formula cfvo `$T$5` stays 0 so iconSet cuts collapse and NewStyle icons drift.
        let sheet = ActiveWorksheet {
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
                row: 4,
                col: 19,
                value: DumpCellValue::Number(20.0),
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
        };
        let index = CellIndex::build(&sheet);
        assert_eq!(eval_cf_formula_number(&sheet, &index, "$T$5"), Some(20.0));
        assert_eq!(eval_cf_formula_number(&sheet, &index, "33"), Some(33.0));
        // Missing cells are 0 in Excel formulas (not None).
        assert_eq!(eval_cf_formula_number(&sheet, &index, "U9"), Some(0.0));
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
