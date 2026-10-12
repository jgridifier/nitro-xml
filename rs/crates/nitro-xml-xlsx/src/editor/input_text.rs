// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/edit/input-text.ts
// (formulaBarText / cellInputText / numberInputText); changes listed in PROVENANCE.md.
//! Formula-bar text: what the user would type to recreate the cell.

use crate::model::{ActiveWorksheet, DumpCell, DumpCellValue, WorkbookSlice};
use crate::numfmt::{format_value, is_date_format, parse_cell_input, CellValue, FormatOptions};
use crate::styles::style_at;

/// The re-typeable content of a cell (formulas keep their leading `=`).
pub fn cell_input_text(
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
    row: u32,
    col: u32,
) -> String {
    let cell = sheet.cells.iter().find(|c| c.row == row && c.col == col);
    formula_bar_text(workbook, cell)
}

fn formula_bar_text(workbook: &WorkbookSlice, cell: Option<&DumpCell>) -> String {
    let Some(cell) = cell else {
        return String::new();
    };
    if let Some(formula) = &cell.formula {
        return format!("={formula}");
    }
    match &cell.value {
        DumpCellValue::Null => String::new(),
        DumpCellValue::Bool(true) => "TRUE".into(),
        DumpCellValue::Bool(false) => "FALSE".into(),
        DumpCellValue::Error(e) => e.clone(),
        DumpCellValue::Number(n) => {
            let style = style_at(&workbook.styles, cell.style_id);
            number_input_text(*n, &style.num_fmt, workbook.date1904)
        }
        DumpCellValue::NumberRaw(s) => s.clone(),
        DumpCellValue::String(_) => {
            let text = if let Some(runs) = &cell.rich_text {
                if runs.is_empty() {
                    string_value(&cell.value)
                } else {
                    runs.iter().map(|r| r.text.as_str()).collect::<String>()
                }
            } else {
                string_value(&cell.value)
            };
            quote_literal(workbook.date1904, &text)
        }
    }
}

fn string_value(value: &DumpCellValue) -> String {
    match value {
        DumpCellValue::String(s) => s.clone(),
        _ => String::new(),
    }
}

/// Prefix text Excel would parse as a number, date, boolean, or formula.
fn quote_literal(date1904: bool, text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let parsed = parse_cell_input(
        text,
        &FormatOptions {
            date1904,
            ..FormatOptions::default()
        },
    );
    let literal =
        parsed.formula.is_none() && matches!(&parsed.value, CellValue::Text(t) if t == text);
    if literal {
        text.to_owned()
    } else {
        format!("'{text}")
    }
}

fn number_input_text(value: f64, num_fmt: &str, date1904: bool) -> String {
    if is_date_format(num_fmt) {
        let whole = value.floor();
        let has_time = value != whole;
        let has_date = whole != 0.0 || !has_time;
        let format = if has_date {
            if has_time {
                "m/d/yyyy h:mm:ss AM/PM"
            } else {
                "m/d/yyyy"
            }
        } else {
            "h:mm:ss AM/PM"
        };
        return format_value(
            &CellValue::Number(value),
            format,
            &FormatOptions {
                date1904,
                ..FormatOptions::default()
            },
        )
        .text;
    }
    if is_percent_format(num_fmt) {
        return format!("{}%", plain_number(value * 100.0));
    }
    plain_number(value)
}

fn is_percent_format(num_fmt: &str) -> bool {
    let mut out = String::new();
    let mut chars = num_fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            for n in chars.by_ref() {
                if n == '"' {
                    break;
                }
            }
            continue;
        }
        if c == '\\' {
            chars.next();
            continue;
        }
        out.push(c);
    }
    out.contains('%')
}

/// Excel shows 15 significant digits in the formula bar (`Number(n.toPrecision(15))`).
fn plain_number(n: f64) -> String {
    if !n.is_finite() {
        return "0".into();
    }
    let rounded = round_sig(n, 15);
    if !rounded.is_finite() {
        return "0".into();
    }
    let mut buf = ryu::Buffer::new();
    buf.format_finite(rounded).to_owned()
}

fn round_sig(n: f64, digits: i32) -> f64 {
    let abs = n.abs();
    if abs == 0.0 {
        return 0.0;
    }
    let exp = abs.log10().floor() as i32;
    let scale = 10f64.powi(digits - 1 - exp);
    if !scale.is_finite() || scale == 0.0 {
        return n;
    }
    (n * scale).round() / scale
}
