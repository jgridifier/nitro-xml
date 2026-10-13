// SPDX-License-Identifier: MIT
//! Save an xlsx package, optionally after replaying an edit script.
//!
//! ```text
//! xlsx_save <in> <out> [--script edits.json] [--omit-full-calc]
//! ```
//!
//! Script replay matches TS edit-parity (`recalc: false` in `parity/edit/apply.mjs`):
//! cached formula values stay as loaded. `--omit-full-calc` is the negative control:
//! a dirty save without `fullCalcOnLoad="1"`.

use std::env;
use std::fs;
use std::process;

use nitro_xml_xlsx::edit::{replay_xlsx_session, XlsxHost};
use nitro_xml_xlsx::{save_xlsx_with_options, XlsxSaveOptions};

fn usage() -> ! {
    eprintln!("usage: xlsx_save <in> <out> [--script json] [--omit-full-calc]");
    process::exit(2);
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut positional = Vec::new();
    let mut script: Option<String> = None;
    let mut omit = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--script" => {
                let Some(path) = args.get(i + 1) else {
                    usage();
                };
                script = Some(path.clone());
                i += 2;
            }
            "--omit-full-calc" => {
                omit = true;
                i += 1;
            }
            other if other.starts_with('-') => usage(),
            other => {
                positional.push(other.to_owned());
                i += 1;
            }
        }
    }
    if positional.len() != 2 {
        usage();
    }
    let bytes = fs::read(&positional[0]).unwrap_or_else(|e| {
        eprintln!("read {}: {e}", positional[0]);
        process::exit(1);
    });
    let host = if let Some(path) = script {
        let json = fs::read_to_string(&path).unwrap_or_else(|e| {
            eprintln!("read {path}: {e}");
            process::exit(1);
        });
        let (report, session) = replay_xlsx_session(&bytes, &json).unwrap_or_else(|e| {
            eprintln!("replay: {e}");
            process::exit(1);
        });
        if report.status != "ok" {
            eprintln!("replay status {}", report.status);
            process::exit(1);
        }
        session.host
    } else {
        XlsxHost::open(&bytes).unwrap_or_else(|e| {
            eprintln!("open: {e}");
            process::exit(1);
        })
    };
    let saved = save_xlsx_with_options(
        &host,
        &bytes,
        &XlsxSaveOptions {
            full_calc_on_load: !omit,
        },
    )
    .unwrap_or_else(|e| {
        eprintln!("save: {e}");
        process::exit(1);
    });
    fs::write(&positional[1], saved).unwrap_or_else(|e| {
        eprintln!("write {}: {e}", positional[1]);
        process::exit(1);
    });
}
