// SPDX-License-Identifier: MIT
//! Dump xlsx viewer engine API (#19) for differential parity.

use nitro_xml_xlsx::{
    cell_text_tsv, hyperlinks_in_sheet, load_workbook, parse_all_worksheets, search_workbook,
    visible_sheets_json, SearchOptions,
};
use serde_json::json;
use std::process;

fn fail_json(value: serde_json::Value) -> ! {
    println!("{value}");
    process::exit(2);
}

fn main() {
    let mut args = std::env::args_os();
    let _ = args.next();
    let Some(path) = args.next() else {
        fail_json(json!({"error": "usage"}));
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => fail_json(json!({"error": "io"})),
    };
    let workbook = match load_workbook(&bytes) {
        Ok(wb) => wb,
        Err(e) => fail_json(json!({"error": e})),
    };
    let sheets = match parse_all_worksheets(&bytes, &workbook) {
        Ok(s) => s,
        Err(e) => fail_json(json!({"error": e})),
    };
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
    let mut search = search_workbook(&workbook, &sheets, "a", SearchOptions::default());
    // Cap + stable order for differential dumps (corpus can have thousands of "a" hits).
    if let Some(arr) = search.as_array_mut() {
        arr.sort_by(|a, b| {
            (
                a.get("sheet").and_then(|v| v.as_u64()),
                a.get("row").and_then(|v| v.as_u64()),
                a.get("col").and_then(|v| v.as_u64()),
            )
                .cmp(&(
                    b.get("sheet").and_then(|v| v.as_u64()),
                    b.get("row").and_then(|v| v.as_u64()),
                    b.get("col").and_then(|v| v.as_u64()),
                ))
        });
        arr.truncate(50);
    }
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
