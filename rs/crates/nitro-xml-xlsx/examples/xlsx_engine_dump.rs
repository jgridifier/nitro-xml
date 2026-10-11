// SPDX-License-Identifier: MIT
//! Dump xlsx viewer engine API (#19) for differential parity.

use nitro_xml_xlsx::{
    cell_text_tsv, hyperlinks_in_sheet, load_workbook, parse_all_worksheets, search_workbook,
    visible_sheets_json, SearchOptions,
};
use serde_json::json;
use std::env;
use std::fs;
use std::process;

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: xlsx_engine_dump <file.xlsx>");
        process::exit(2);
    });
    let bytes = fs::read(&path).unwrap_or_else(|e| {
        eprintln!("read {}: {}", path, e);
        process::exit(1);
    });
    let workbook = load_workbook(&bytes).unwrap_or_else(|e| {
        eprintln!("load: {}", e);
        process::exit(1);
    });
    let sheets = parse_all_worksheets(&bytes, &workbook).unwrap_or_else(|e| {
        eprintln!("sheets: {}", e);
        process::exit(1);
    });
    let active = workbook.active_sheet as usize;
    let cell_text = if let Some(Some(sheet)) = sheets.get(active) {
        cell_text_tsv(&workbook, sheet, active, &sheets, "A1:E20").unwrap_or_default()
    } else {
        String::new()
    };
    let links = if let Some(Some(sheet)) = sheets.get(active) {
        hyperlinks_in_sheet(&workbook, sheet, active)
    } else {
        json!([])
    };
    let search = search_workbook(&workbook, &sheets, "a", SearchOptions::default());
    let out = json!({
        "format": "xlsx",
        "sheets": visible_sheets_json(&workbook),
        "activeSheet": active,
        "cellTextSample": cell_text,
        "hyperlinks": links,
        "searchSample": search,
    });
    println!(
        "{}",
        serde_json::to_string(&out).unwrap_or_else(|_| "{}".into())
    );
}
