// SPDX-License-Identifier: MIT
//! Formula-coverage names and editor-recalc checkpoints (#77).
//!
//!   xlsx_editor_recalc --names
//!   xlsx_editor_recalc [--no-recalc] <fixture.xlsx> <script.json> <out.json>

use std::env;
use std::fs;
use std::process;

use nitro_xml_xlsx::edit::replay_formula_checkpoints;
use nitro_xml_xlsx::formula::fns::registered_functions;

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--names") {
        for (name, category) in registered_functions() {
            println!("{name}\t{category}");
        }
        return;
    }
    let mut recalc = true;
    if args.first().map(String::as_str) == Some("--no-recalc") {
        recalc = false;
        args.remove(0);
    }
    if args.len() != 3 {
        eprintln!(
            "usage: xlsx_editor_recalc --names | [--no-recalc] <fixture.xlsx> <script.json> <out.json>"
        );
        process::exit(2);
    }
    let bytes = fs::read(&args[0]).unwrap_or_else(|err| {
        eprintln!("read {}: {err}", args[0]);
        process::exit(1);
    });
    let script = fs::read_to_string(&args[1]).unwrap_or_else(|err| {
        eprintln!("read {}: {err}", args[1]);
        process::exit(1);
    });
    match replay_formula_checkpoints(&bytes, &script, recalc) {
        Ok(value) => {
            let text = serde_json::to_string(&value).unwrap_or_else(|_| "null".to_owned());
            fs::write(&args[2], text + "\n").unwrap_or_else(|err| {
                eprintln!("write {}: {err}", args[2]);
                process::exit(1);
            });
        }
        Err(err) => {
            eprintln!("{err}");
            process::exit(1);
        }
    }
}
