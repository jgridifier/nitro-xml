// SPDX-License-Identifier: MIT
//! Dump parse results for a JSON array of formula strings (#71 differential).

use std::process;

use nitro_xml_xlsx::formula::{dump_formulas, stringify_formula_dump, DUMP_SHEET};

fn main() {
    let mut args = std::env::args_os();
    let _ = args.next();
    let Some(path) = args.next() else {
        eprintln!("usage: xlsx_formula_dump <formulas.json>");
        process::exit(2);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("read {}: {err}", path.to_string_lossy());
            process::exit(2);
        }
    };
    let formulas: Vec<String> = match serde_json::from_str(&text) {
        Ok(formulas) => formulas,
        Err(err) => {
            eprintln!("json: {err}");
            process::exit(2);
        }
    };
    let dump = dump_formulas(&formulas, DUMP_SHEET);
    println!("{}", stringify_formula_dump(&dump));
}
