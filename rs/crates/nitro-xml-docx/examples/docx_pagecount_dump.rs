// SPDX-License-Identifier: MIT
//! Page-count dump for the real-font differential (#22).
//!
//! `docx_pagecount_dump <fixture.docx> [cache.json]`
//! Prints `{done:false, pending:[...]}` until the cache covers every probe,
//! then `{done:true, format:"docx", pageCount:N}`.

use std::env;
use std::fs;
use std::path::Path;
use std::process;

fn main() {
    let mut args = env::args().skip(1);
    let path = args.next().unwrap_or_else(|| {
        eprintln!("usage: docx_pagecount_dump <fixture.docx> [cache.json]");
        process::exit(1);
    });
    let cache_path = args.next();
    let bytes = fs::read(&path).unwrap_or_else(|err| {
        eprintln!("read {path}: {err}");
        process::exit(1);
    });
    let cache_json = match cache_path.as_deref() {
        Some(cache) if Path::new(cache).is_file() => {
            Some(fs::read_to_string(cache).unwrap_or_else(|err| {
                eprintln!("read {cache}: {err}");
                process::exit(1);
            }))
        }
        _ => None,
    };
    match nitro_xml_docx::dump::dump_docx_pagecount(&bytes, cache_json.as_deref()) {
        Ok(json) => println!("{json}"),
        Err(err) => {
            eprintln!("{err}");
            process::exit(1);
        }
    }
}
