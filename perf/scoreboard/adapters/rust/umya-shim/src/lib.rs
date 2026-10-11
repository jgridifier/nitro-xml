// SPDX-License-Identifier: MIT
//! umya-spreadsheet behind wasm-bindgen for the scoreboard parse race. `parse` (timed) is the
//! documented full read (`reader::xlsx::read_reader(.., true)`, every sheet deserialised; the lazy
//! reader would leave sheets unparsed); `extract` (not timed) serialises the cells.
use std::fmt::Write;
use std::io::Cursor;
use umya_spreadsheet::{CellRawValue, Workbook};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Model {
    book: Workbook,
}

#[wasm_bindgen]
pub fn parse(bytes: &[u8]) -> Result<Model, JsError> {
    let book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(Model { book })
}

fn col_label(mut col: u32) -> String {
    let mut s = Vec::new();
    while col > 0 {
        s.push(b'A' + ((col - 1) % 26) as u8);
        col = (col - 1) / 26;
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

fn json_str(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => write!(out, "\\u{:04x}", c as u32).unwrap(),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[wasm_bindgen]
pub fn extract(model: &Model) -> String {
    let mut out = String::from("{\"cells\":[");
    let mut first = true;
    for sheet in model.book.sheet_collection_no_check() {
        for cell in sheet.cells() {
            let (t, val): (&str, String) = match cell.raw_value() {
                CellRawValue::String(s) => ("s", s.to_string()),
                CellRawValue::RichText(r) => ("s", r.text().to_string()),
                CellRawValue::Lazy(s) => ("s", s.to_string()),
                CellRawValue::Numeric(f) => ("n", format!("{f:?}")),
                CellRawValue::Bool(b) => ("b", b.to_string()),
                CellRawValue::Error(e) => ("e", e.to_string()),
                CellRawValue::Empty => continue,
            };
            let coord = cell.coordinate();
            if !first {
                out.push(',');
            }
            first = false;
            out.push('[');
            json_str(&mut out, sheet.name());
            out.push(',');
            json_str(&mut out, &format!("{}{}", col_label(coord.col_num()), coord.row_num()));
            out.push(',');
            json_str(&mut out, t);
            out.push(',');
            json_str(&mut out, &val);
            out.push(']');
        }
    }
    out.push_str("]}");
    out
}
