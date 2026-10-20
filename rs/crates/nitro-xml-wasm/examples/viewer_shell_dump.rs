// SPDX-License-Identifier: MIT
//! Stage-6 viewer shell snapshot (#29) for docx and pptx differential parity.
//!
//! `viewer_shell_dump <file> [--negative]`
//! `--negative` forces `zoom: 0` so the comparator must fail.

use std::env;
use std::process;

use nitro_xml_wasm::Handle;
use serde_json::json;

fn main() {
    let mut negative = false;
    let mut path = None;
    for arg in env::args().skip(1) {
        if arg == "--negative" {
            negative = true;
        } else if path.is_none() {
            path = Some(arg);
        }
    }
    let Some(path) = path else {
        println!("{}", json!({ "error": "usage" }));
        process::exit(2);
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => {
            println!("{}", json!({ "error": "io" }));
            process::exit(2);
        }
    };
    let mut handle = Handle::new(bytes);
    if handle.open_document(&path).is_err() {
        println!("{}", json!({ "error": "open_failed" }));
        process::exit(1);
    }
    println!("{}", handle.shell_parity_json(negative));
}
