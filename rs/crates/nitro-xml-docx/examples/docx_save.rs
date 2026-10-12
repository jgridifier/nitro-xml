// SPDX-License-Identifier: MIT
//! Save a docx package, optionally after replaying an edit script.
//!
//! ```text
//! docx_save <in> <out> [--script edits.json] [--skip-document-rewrite]
//! ```
//!
//! `--skip-document-rewrite` is the negative control: dirty host without rewriting document.xml.

use std::env;
use std::fs;
use std::process;

use nitro_xml_docx::edit::{replay_docx_session, DocxHost};
use nitro_xml_docx::{save_docx_with_options, DocxSaveOptions};

fn usage() -> ! {
    eprintln!("usage: docx_save <in> <out> [--script json] [--skip-document-rewrite]");
    process::exit(2);
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut positional = Vec::new();
    let mut script: Option<String> = None;
    let mut skip = false;
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
            "--skip-document-rewrite" => {
                skip = true;
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
        let (report, session) = replay_docx_session(&bytes, &json).unwrap_or_else(|e| {
            eprintln!("replay: {e}");
            process::exit(1);
        });
        if report.status != "ok" {
            eprintln!("replay status {}", report.status);
            process::exit(1);
        }
        session.host
    } else {
        DocxHost::open(&bytes).unwrap_or_else(|e| {
            eprintln!("open: {e}");
            process::exit(1);
        })
    };
    let saved = save_docx_with_options(
        &host,
        &bytes,
        &DocxSaveOptions {
            skip_document_rewrite: skip,
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
