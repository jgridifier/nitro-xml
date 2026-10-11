// SPDX-License-Identifier: MIT
//! Dump pptx view-IR JSON for parity (`parity/oracle/pptx.ts`).

use std::env;
use std::fs;
use std::process;

use nitro_xml_pptx::dump_pptx_view_ir_string;
use nitro_xml_pptx::Presentation;

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: pptx_view_ir_dump <file.pptx>");
        process::exit(2);
    });
    let bytes = fs::read(&path).unwrap_or_else(|e| {
        eprintln!(
            "{}",
            serde_json::json!({
                "error": "load_failed",
                "detail": e.to_string()
            })
        );
        process::exit(1);
    });
    match Presentation::open(&bytes) {
        Ok(mut pres) => match dump_pptx_view_ir_string(&mut pres) {
            Ok(json) => println!("{}", json),
            Err(e) => {
                eprintln!(
                    "{}",
                    serde_json::json!({
                        "error": "dump_failed",
                        "detail": e.to_string()
                    })
                );
                process::exit(1);
            }
        },
        Err(e) => {
            eprintln!(
                "{}",
                serde_json::json!({
                    "error": "load_failed",
                    "detail": e.to_string()
                })
            );
            process::exit(1);
        }
    }
}
