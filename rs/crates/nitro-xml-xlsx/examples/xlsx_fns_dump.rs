// SPDX-License-Identifier: MIT
//! Dump Math & Trig / Statistical formula results for the #73 differential.

use std::process;

use nitro_xml_xlsx::formula::fns::dump_fn_cases;

fn main() {
    let mut args = std::env::args_os();
    let _ = args.next();
    let Some(path) = args.next() else {
        eprintln!("usage: xlsx_fns_dump <cases.json>");
        process::exit(2);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("read {}: {err}", path.to_string_lossy());
            process::exit(2);
        }
    };
    let cases: serde_json::Value = match serde_json::from_str(&text) {
        Ok(cases) => cases,
        Err(err) => {
            eprintln!("json: {err}");
            process::exit(2);
        }
    };
    let dump = dump_fn_cases(&cases);
    println!(
        "{}",
        serde_json::to_string(&dump).unwrap_or_else(|_| "[]".to_owned())
    );
}
