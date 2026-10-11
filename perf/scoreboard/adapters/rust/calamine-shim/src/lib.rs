// SPDX-License-Identifier: MIT
//! calamine behind wasm-bindgen for the scoreboard parse race. `parse` (timed) opens the package
//! and reads every worksheet range (calamine's documented `worksheets()`); `extract` (not timed)
//! serialises the cells for the equivalence check.
use calamine::{Data, Reader, Xlsx};
use std::fmt::Write;
use std::io::Cursor;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Model {
    sheets: Vec<(String, calamine::Range<Data>)>,
}

#[wasm_bindgen]
pub fn parse(bytes: &[u8]) -> Result<Model, JsError> {
    let mut book: Xlsx<_> = Xlsx::new(Cursor::new(bytes)).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(Model { sheets: book.worksheets() })
}

fn col_label(mut col: u32) -> String {
    let mut s = Vec::new();
    col += 1;
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
    for (name, range) in &model.sheets {
        let (r0, c0) = range.start().unwrap_or((0, 0));
        for (r, c, v) in range.used_cells() {
            let (t, val): (&str, String) = match v {
                Data::Int(i) => ("n", i.to_string()),
                Data::Float(f) => ("n", format!("{f:?}")),
                Data::DateTime(d) => ("n", format!("{:?}", d.as_f64())),
                Data::String(s) => ("s", s.clone()),
                Data::Bool(b) => ("b", b.to_string()),
                Data::DateTimeIso(s) => ("d", s.clone()),
                Data::DurationIso(s) => ("s", s.clone()),
                Data::Error(e) => ("e", e.to_string()),
                Data::Empty => continue,
            };
            if !first {
                out.push(',');
            }
            first = false;
            out.push('[');
            json_str(&mut out, name);
            out.push(',');
            json_str(&mut out, &format!("{}{}", col_label(c0 + c as u32), r0 + r as u32 + 1));
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
