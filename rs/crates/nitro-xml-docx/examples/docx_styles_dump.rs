// SPDX-License-Identifier: MIT
use std::env;
use std::fs;
use std::process;

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: docx_styles_dump <file>");
        process::exit(2);
    });
    let bytes = fs::read(&path).unwrap_or_else(|e| {
        eprintln!("{e}");
        process::exit(1);
    });
    match nitro_xml_docx::dump::dump_docx_styles(&bytes) {
        Ok(json) => println!("{json}"),
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}
