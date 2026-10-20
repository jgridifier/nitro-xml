// SPDX-License-Identifier: MIT
//! Touch-rebuild OPC save for the #65 round-trip gate (proves PackageWriter rebuild).
//!
//! Usage:
//!   opc_touch_save <input> <output>
//!   opc_touch_save <input> <output> --corrupt-carry <part>
//!
//! Marks one part dirty (rewrites same bytes), finishes through the rebuild path,
//! writes the package, and prints JSON `{ "dirtyPart": "..." }` on stdout.
//! `--corrupt-carry` flips compression on an untouched part (negative control).

use std::env;
use std::fs;
use std::process;

use nitro_xml_opc::write::touch_save_with_options;
use serde_json::json;

fn main() {
    let mut args = env::args_os().peekable();
    let _ = args.next();
    let Some(input) = args.next() else {
        eprintln!("usage: opc_touch_save <input> <output> [--corrupt-carry <part>]");
        process::exit(2);
    };
    let Some(output) = args.next() else {
        eprintln!("usage: opc_touch_save <input> <output> [--corrupt-carry <part>]");
        process::exit(2);
    };

    let mut corrupt_carry: Option<String> = None;
    while let Some(flag) = args.next() {
        if flag == "--corrupt-carry" {
            let Some(part) = args.next() else {
                eprintln!("opc_touch_save: --corrupt-carry needs a part name");
                process::exit(2);
            };
            corrupt_carry = Some(part.to_string_lossy().into_owned());
        } else {
            eprintln!("opc_touch_save: unknown arg {}", flag.to_string_lossy());
            process::exit(2);
        }
    }

    let bytes = match fs::read(&input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("opc_touch_save: read {}: {e}", input.to_string_lossy());
            process::exit(1);
        }
    };

    let touched = match touch_save_with_options(&bytes, None, corrupt_carry.as_deref()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("opc_touch_save: {e}");
            process::exit(1);
        }
    };

    if let Err(e) = fs::write(&output, &touched.bytes) {
        eprintln!("opc_touch_save: write {}: {e}", output.to_string_lossy());
        process::exit(1);
    }

    println!(
        "{}",
        json!({
            "dirtyPart": touched.dirty_part,
            "corruptCarry": corrupt_carry,
            "inputBytes": bytes.len(),
            "outputBytes": touched.bytes.len(),
            "identityShortcut": false,
        })
    );
}
