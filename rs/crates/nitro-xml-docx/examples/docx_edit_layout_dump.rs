// SPDX-License-Identifier: MIT
//! Dump LayoutResult JSON after applying an edit script (#81).
//!
//! ```text
//! docx_edit_layout_dump <path.docx> <edits.json>
//! ```

use std::env;
use std::fs;
use std::process;

fn main() {
    let mut args = env::args().skip(1);
    let docx = args.next().unwrap_or_else(|| {
        eprintln!("usage: docx_edit_layout_dump <path.docx> <edits.json>");
        process::exit(2);
    });
    let script_path = args.next().unwrap_or_else(|| {
        eprintln!("usage: docx_edit_layout_dump <path.docx> <edits.json>");
        process::exit(2);
    });
    let bytes = fs::read(&docx).unwrap_or_else(|e| {
        eprintln!("read {docx}: {e}");
        process::exit(1);
    });
    let script = fs::read_to_string(&script_path).unwrap_or_else(|e| {
        eprintln!("read {script_path}: {e}");
        process::exit(1);
    });
    match nitro_xml_docx::dump::dump_docx_layout_after_edit(&bytes, &script) {
        Ok(json) => println!("{json}"),
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}
