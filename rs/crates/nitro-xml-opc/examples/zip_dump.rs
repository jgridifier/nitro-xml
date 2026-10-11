// SPDX-License-Identifier: MIT
//! Prints every part of a zip package as one line of JSON (`name`, `size`, `sha256`), inflating
//! each part through the bounded reader. The differential gate compares this with the TS reader.

use nitro_xml_opc::zip::ZipPackage;
use serde_json::json;
use sha2::{Digest, Sha256};

fn fail(kind: &str) -> ! {
    println!("{}", json!({ "error": kind }));
    std::process::exit(2);
}

fn main() {
    let Some(path) = std::env::args_os().nth(1) else {
        fail("usage")
    };
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => fail("io"),
    };
    let mut pkg = match ZipPackage::open(&bytes) {
        Ok(pkg) => pkg,
        Err(error) => fail(error.kind()),
    };
    let names: Vec<String> = pkg.part_names().map(str::to_owned).collect();
    let mut parts: Vec<(String, Vec<u8>)> = Vec::with_capacity(names.len());
    for name in names {
        match pkg.read(&name) {
            Ok(Some(data)) => parts.push((name, data)),
            Ok(None) => fail("invalid_archive"),
            Err(error) => fail(error.kind()),
        }
    }
    parts.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let rows: Vec<serde_json::Value> = parts
        .iter()
        .map(|(name, data)| {
            let digest: String = Sha256::digest(data)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            json!({ "name": name, "size": data.len(), "sha256": digest })
        })
        .collect();
    println!("{}", json!({ "parts": rows }));
}
