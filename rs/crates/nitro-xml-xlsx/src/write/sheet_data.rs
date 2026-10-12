// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/write/sheet-data.ts; changes listed in PROVENANCE.md.
//! `<sheetData>` in row and column order. Dynamic-array formulas are written as
//! `t="array"` anchors with an XLDAPR `cm` index.

use std::collections::BTreeMap;

use crate::address::{format_address, format_range};
use crate::model::{ActiveWorksheet, CellAddress, DumpCell, DumpCellValue, RowInfo};
use crate::number_format::js_number_string;

use super::dynamic_array::needs_array_evaluation;
use super::shared_strings::SharedStringTable;
use super::xml_out::{attr_bool, attr_f64, attr_str, attr_u32, encode_escapes, escape_text};

const MAX_CELL_TEXT: usize = 32_767;

fn style_attr(style_id: Option<u32>, style_count: usize) -> Option<u32> {
    style_id.filter(|id| *id > 0 && (*id as usize) < style_count)
}

fn stored_error(code: &str) -> &str {
    if code == "#SPILL!" || code == "#CALC!" {
        "#VALUE!"
    } else {
        code
    }
}

fn is_dynamic_formula(cell: &DumpCell) -> bool {
    if cell.array_range.is_some() || cell.legacy_formula == Some(true) {
        return false;
    }
    if cell.dynamic_array == Some(true) {
        return true;
    }
    if let DumpCellValue::Error(code) = &cell.value {
        if code == "#SPILL!" {
            return true;
        }
    }
    cell.formula.as_deref().is_some_and(needs_array_evaluation)
}

fn cell_xml(
    strings: &mut SharedStringTable,
    style_count: usize,
    dynamic_cm: u32,
    dynamic_used: &mut bool,
    cell: &DumpCell,
) -> String {
    let addr = format_address(&CellAddress {
        row: cell.row,
        col: cell.col,
    });
    let s = style_attr(cell.style_id, style_count);
    let mut f = String::new();
    let mut cm: Option<u32> = None;
    if let Some(formula) = &cell.formula {
        let formula = escape_text(formula);
        if is_dynamic_formula(cell) {
            *dynamic_used = true;
            cm = Some(dynamic_cm);
            f = format!("<f t=\"array\" ref=\"{addr}\" aca=\"false\">{formula}</f>");
        } else if let Some(range) = &cell.array_range {
            f = format!(
                "<f t=\"array\" ref=\"{}\">{formula}</f>",
                format_range(range)
            );
        } else {
            f = format!("<f>{formula}</f>");
        }
    }
    let mut t: Option<&str> = None;
    let mut v = String::new();
    match &cell.value {
        DumpCellValue::Number(n) => {
            if n.is_finite() {
                v = js_number_string(*n);
            } else {
                t = Some("e");
                v = "#NUM!".into();
            }
        }
        DumpCellValue::NumberRaw(raw) => v = raw.clone(),
        DumpCellValue::Bool(b) => {
            t = Some("b");
            v = if *b { "1" } else { "0" }.into();
        }
        DumpCellValue::String(text) => {
            let text = if text.chars().count() > MAX_CELL_TEXT {
                text.chars().take(MAX_CELL_TEXT).collect()
            } else {
                text.clone()
            };
            if cell.formula.is_some() {
                t = Some("str");
                v = escape_text(&encode_escapes(&text));
            } else {
                t = Some("s");
                let id = strings.id(&text, cell.rich_text.as_deref());
                v = id.to_string();
            }
        }
        DumpCellValue::Error(code) => {
            t = Some("e");
            v = stored_error(code).to_owned();
        }
        DumpCellValue::Null => {}
    }
    let attrs = format!(
        "{}{}{}{}",
        attr_str("r", Some(&addr)),
        attr_u32("s", s),
        attr_str("t", t),
        attr_u32("cm", cm),
    );
    if f.is_empty() && v.is_empty() && t != Some("str") {
        return format!("<c{attrs}/>");
    }
    let value = if t == Some("str") || !v.is_empty() {
        format!("<v>{v}</v>")
    } else {
        String::new()
    };
    format!("<c{attrs}>{f}{value}</c>")
}

fn row_open(row: u32, info: Option<&RowInfo>, style_count: usize) -> String {
    let s = info.and_then(|info| style_attr(info.style_id, style_count));
    format!(
        "{}{}{}{}{}{}{}{}",
        attr_u32("r", Some(row + 1)),
        attr_u32("s", s),
        attr_bool("customFormat", s.map(|_| true)),
        attr_f64("ht", info.and_then(|info| info.height)),
        attr_bool("hidden", info.and_then(|info| info.hidden).filter(|v| *v)),
        attr_bool(
            "customHeight",
            info.and_then(|info| info.custom_height).filter(|v| *v)
        ),
        attr_f64(
            "outlineLevel",
            info.and_then(|info| info.outline_level)
                .filter(|v| *v != 0.0)
        ),
        attr_bool(
            "collapsed",
            info.and_then(|info| info.collapsed).filter(|v| *v)
        ),
    )
}

/// `<sheetData>` for one worksheet.
pub fn sheet_data_xml(
    strings: &mut SharedStringTable,
    style_count: usize,
    dynamic_cm: u32,
    dynamic_used: &mut bool,
    sheet: &ActiveWorksheet,
) -> String {
    let mut by_row: BTreeMap<u32, BTreeMap<u32, &DumpCell>> = BTreeMap::new();
    for cell in &sheet.cells {
        by_row.entry(cell.row).or_default().insert(cell.col, cell);
    }
    let mut rows: Vec<u32> = by_row.keys().copied().collect();
    for row in sheet.row_info.keys() {
        if !by_row.contains_key(row) {
            rows.push(*row);
        }
    }
    rows.sort_unstable();
    rows.dedup();
    let mut out = String::new();
    for row in rows {
        let info = sheet.row_info.get(&row);
        let body = by_row.get(&row).map(|cols| {
            cols.values()
                .map(|cell| cell_xml(strings, style_count, dynamic_cm, dynamic_used, cell))
                .collect::<String>()
        });
        let body = body.unwrap_or_default();
        if body.is_empty() && info.is_none() {
            continue;
        }
        let attrs = row_open(row, info, style_count);
        if body.is_empty() {
            out.push_str(&format!("<row{attrs}/>"));
        } else {
            out.push_str(&format!("<row{attrs}>{body}</row>"));
        }
    }
    if out.is_empty() {
        "<sheetData/>".into()
    } else {
        format!("<sheetData>{out}</sheetData>")
    }
}
