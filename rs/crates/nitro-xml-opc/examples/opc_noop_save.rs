// SPDX-License-Identifier: MIT
//! No-op OPC package save for the #65 / #49 round-trip gate (Rust-vs-original).
//!
//! Usage:
//!   opc_noop_save <input> <output>
//!
//! Reads the package, opens a PackageWriter with no dirty parts, and writes
//! the finished bytes (identity when untouched).

use std::env;
use std::fs;
use std::process;

use nitro_xml_opc::write::noop_save;

fn main() {
    let mut args = env::args_os();
    let _ = args.next();
    let Some(input) = args.next() else {
        eprintln!("usage: opc_noop_save <input> <output>");
        process::exit(2);
    };
    let Some(output) = args.next() else {
        eprintln!("usage: opc_noop_save <input> <output>");
        process::exit(2);
    };

    let bytes = match fs::read(&input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("opc_noop_save: read {}: {e}", input.to_string_lossy());
            process::exit(1);
        }
    };

    let saved = match noop_save(&bytes) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("opc_noop_save: {e}");
            process::exit(1);
        }
    };

    if let Err(e) = fs::write(&output, &saved) {
        eprintln!("opc_noop_save: write {}: {e}", output.to_string_lossy());
        process::exit(1);
    }
}
