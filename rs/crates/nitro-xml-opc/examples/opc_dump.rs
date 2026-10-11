// SPDX-License-Identifier: MIT
//! Prints the OPC part graph of a package as JSON for the #13 differential gate.

use std::process;

use nitro_xml_opc::opc::{hyperlink_policy, Package};
use serde_json::json;

fn fail_json(value: serde_json::Value) -> ! {
    println!("{value}");
    process::exit(2);
}

fn main() {
    let mut args = std::env::args_os();
    let _ = args.next();
    let Some(mode) = args.next() else {
        fail_json(json!({"error": "usage"}));
    };
    let mode = mode.to_string_lossy();
    match mode.as_ref() {
        "package" => dump_package(args.next()),
        "href" => dump_href(args.next()),
        _ => fail_json(json!({"error": "usage"})),
    }
}

fn dump_package(path: Option<std::ffi::OsString>) {
    let Some(path) = path else {
        fail_json(json!({"error": "usage"}));
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => fail_json(json!({"error": "io"})),
    };
    match Package::open(&bytes) {
        Ok(mut pkg) => match pkg.inspect_json() {
            Ok(s) => {
                println!("{s}");
            }
            Err(e) => fail_json(json!({"error": e.kind()})),
        },
        Err(e) => match &e {
            nitro_xml_opc::opc::OpcError::UnsupportedFormat { detected } => {
                fail_json(json!({"error": "unsupported_format", "detected": detected}));
            }
            _ => fail_json(json!({"error": e.kind()})),
        },
    }
}

fn dump_href(href: Option<std::ffi::OsString>) {
    // `href @path` reads raw bytes (allows NULs); bare missing/`__NULL__` is null.
    let policy = match href.as_ref().map(|h| h.to_string_lossy()) {
        None => hyperlink_policy(None),
        Some(s) if s == "__NULL__" => hyperlink_policy(None),
        Some(s) if s.starts_with('@') => {
            let path = s.trim_start_matches('@');
            match std::fs::read(path) {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    hyperlink_policy(Some(text.as_ref()))
                }
                Err(_) => fail_json(json!({"error": "io"})),
            }
        }
        Some(s) => hyperlink_policy(Some(s.as_ref())),
    };
    let mut out = json!({
        "store": policy.store,
        "open": policy.open,
    });
    if let Some(reason) = policy.reason {
        out.as_object_mut()
            .unwrap()
            .insert("reason".into(), json!(reason.as_str()));
    }
    println!("{out}");
}
