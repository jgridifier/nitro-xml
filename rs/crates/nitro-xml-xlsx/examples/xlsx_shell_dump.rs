// SPDX-License-Identifier: MIT
//! Dump Stage-6 xlsx shell snapshot (#29) for differential parity.

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
    let tabs = visible_sheets_json(&workbook);
    let cell_text = if let Some(Some(sheet)) = sheets.get(active) {
        cell_text_tsv(&workbook, sheet, active, &sheets, "A1").unwrap_or_default()
    } else {
        String::new()
    };
    let links = if let Some(Some(sheet)) = sheets.get(active) {
        hyperlinks_in_sheet(&workbook, sheet, active)
    } else {
        json!([])
    };
    let mut search = search_workbook(&workbook, &sheets, "a", SearchOptions::default());
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
        arr.truncate(20);
    }
    // Shell-facing snapshot: zoom default, tabs, copy sample, safe-href links, find sample.
    let out = json!({
        "format": "xlsx",
        "zoom": 100.0,
        "tabs": tabs,
        "selection": "A1",
        "copySample": cell_text,
        "hyperlinks": links,
        "findSample": search,
        "diagnostics": {
            "fontSubstitutions": [],
            "unsupported": [],
        },
    });
    println!("{}", serde_json::to_string(&out).unwrap_or_else(|_| "{}".into()));
}
