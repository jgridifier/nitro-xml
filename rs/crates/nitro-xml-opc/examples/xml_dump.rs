// SPDX-License-Identifier: MIT
//! Prints one JSON line per XML file path listed in the argument file, in the differential-gate
//! shape compared against upstream `parseXml`.

use std::fs;
use std::process;

use nitro_xml_opc::xml::{parse_xml_bytes, Element, Node};
use serde_json::{json, Value};

fn main() {
    let code = run();
    process::exit(code);
}

fn run() -> i32 {
    let Some(list_path) = std::env::args_os().nth(1) else {
        eprintln!("usage: xml_dump <listfile>");
        return 2;
    };
    let list = match fs::read_to_string(&list_path) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("error: cannot read listfile");
            return 2;
        }
    };
    for line in list.lines() {
        let path = line.trim();
        if path.is_empty() {
            continue;
        }
        let line = match fs::read(path) {
            Ok(bytes) => match parse_xml_bytes(&bytes) {
                Ok(doc) => json!({ "file": path, "tree": element_to_dump_value(&doc.root) }),
                Err(err) => json!({ "file": path, "error": err.kind() }),
            },
            Err(_) => json!({ "file": path, "error": "io" }),
        };
        println!("{line}");
    }
    0
}

fn element_to_dump_value(element: &Element) -> Value {
    let attrs: Vec<Value> = element
        .attrs
        .iter()
        .map(|a| json!([a.ns.as_deref(), a.local.as_str(), a.value.as_str()]))
        .collect();
    let mut out = vec![
        json!("e"),
        json!(element.ns.as_deref()),
        json!(element.local.as_str()),
        Value::Array(attrs),
    ];
    for child in &element.children {
        match child {
            Node::Element(el) => out.push(element_to_dump_value(el)),
            Node::Text(t) => out.push(json!(["t", t])),
        }
    }
    Value::Array(out)
}
