// SPDX-License-Identifier: MIT
//! Dump issue-24 presentation JSON for differential gates.

use std::env;
use std::fs;
use std::process;

use nitro_xml_pptx::Presentation;

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: pptx_dump <file.pptx>");
        process::exit(2);
    });
    let bytes = fs::read(&path).unwrap_or_else(|e| {
        let msg = e.to_string();
        println!(
            "{}",
            serde_json::json!({
                "error": "load_failed",
                "detail": msg.chars().take(200).collect::<String>()
            })
        );
        process::exit(1);
    });
    match Presentation::open(&bytes) {
        Ok(mut pres) => {
            let json = pres.inspect_json().unwrap_or_else(|e| {
                let msg = e.to_string();
                println!(
                    "{}",
                    serde_json::json!({
                        "error": "load_failed",
                        "detail": msg.chars().take(200).collect::<String>()
                    })
                );
                process::exit(1);
            });
            println!("{}", json);
        }
        Err(e) => {
            let msg = e.to_string();
            println!(
                "{}",
                serde_json::json!({
                    "error": "load_failed",
                    "detail": msg.chars().take(200).collect::<String>()
                })
            );
            process::exit(1);
        }
    }
}
