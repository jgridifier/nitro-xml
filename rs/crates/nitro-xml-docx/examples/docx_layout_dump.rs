// SPDX-License-Identifier: MIT
//! Dump LayoutResult JSON for differential parity (#22).

use std::env;
use std::process;

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: docx_layout_dump <path.docx>");
        process::exit(1);
    });
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        eprintln!("read {}: {}", path, e);
        process::exit(1);
    });
    match nitro_xml_docx::dump::dump_docx_layout(&bytes) {
        Ok(json) => println!("{}", json),
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}
