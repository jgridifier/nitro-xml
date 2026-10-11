// SPDX-License-Identifier: MIT
//! Prints xlsx CellItem view-IR JSON for parity (`dumpXlsxViewIr` oracle shape).

use std::process;

use nitro_xml_xlsx::{dump_xlsx_view_ir_string, load_workbook, parse_all_worksheets};
use serde_json::json;

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
    match load_workbook(&bytes) {
        Ok(wb) => match parse_all_worksheets(&bytes, &wb) {
            Ok(worksheets) => println!("{}", dump_xlsx_view_ir_string(&wb, &worksheets)),
            Err(e) => fail_json(json!({"error": e})),
        },
        Err(e) => fail_json(json!({"error": e})),
    }
}
