// SPDX-License-Identifier: MIT
//! nitro-xml-xlsx behind wasm-bindgen for the scoreboard parse race. `parse` (timed) is
//! `load_workbook_meta` then `parse_all_worksheets` (one pass per sheet); `extract` (not timed)
//! serialises cell values.
use nitro_xml_xlsx::format_address;
use nitro_xml_xlsx::model::{ActiveWorksheet, CellAddress, DumpCellValue};
use nitro_xml_xlsx::{load_workbook_meta, parse_all_worksheets};
use std::fmt::Write;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Model {
    sheets: Vec<(String, ActiveWorksheet)>,
}

#[wasm_bindgen]
pub fn parse(bytes: &[u8]) -> Result<Model, JsError> {
    let wb = load_workbook_meta(bytes).map_err(|e| JsError::new(&e))?;
    let names: Vec<String> = wb.sheets.iter().map(|s| s.name.clone()).collect();
    let parsed = parse_all_worksheets(bytes, &wb).map_err(|e| JsError::new(&e))?;
    let sheets = names
        .into_iter()
        .zip(parsed)
        .filter_map(|(name, ws)| ws.map(|w| (name, w)))
        .collect();
    Ok(Model { sheets })
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

fn cell_type_value(value: &DumpCellValue) -> Option<(&'static str, String)> {
    match value {
        DumpCellValue::Null => None,
        DumpCellValue::Number(n) => Some(("n", format!("{n:?}"))),
        DumpCellValue::NumberRaw(s) => Some(("n", s.clone())),
        DumpCellValue::String(s) if s.is_empty() => None,
        DumpCellValue::String(s) => Some(("s", s.clone())),
        DumpCellValue::Bool(b) => Some(("b", b.to_string())),
        DumpCellValue::Error(e) => Some(("e", e.clone())),
    }
}

#[wasm_bindgen]
pub fn extract(model: &Model) -> String {
    let mut out = String::from("{\"cells\":[");
    let mut first = true;
    for (name, sheet) in &model.sheets {
        for cell in &sheet.cells {
            let (t, val) = match cell_type_value(&cell.value) {
                Some(tv) => tv,
                None => continue,
            };
            if !first {
                out.push(',');
            }
            first = false;
            let addr = format_address(&CellAddress {
                row: cell.row,
                col: cell.col,
            });
            out.push('[');
            json_str(&mut out, name);
            out.push(',');
            json_str(&mut out, &addr);
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
