// SPDX-License-Identifier: MIT
//! Prints the Stage-3 xlsx workbook slice as JSON for the #15 differential gate.

use std::process;

use nitro_xml_xlsx::load_workbook;
use nitro_xml_xlsx::model::stringify_json;
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
        Ok(wb) => println!("{}", stringify_json(&wb.to_json())),
        Err(e) => fail_json(json!({"error": e})),
    }
}
