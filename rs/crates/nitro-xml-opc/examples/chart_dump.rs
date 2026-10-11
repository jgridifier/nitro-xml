// SPDX-License-Identifier: MIT
//! JSON CLI for chart model differential (#28).

use nitro_xml_opc::chart::parse_chart_space_xml;
use std::io::{self, Read};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let xml = if args.is_empty() {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).expect("stdin");
        buf
    } else if args[0] == "--file" {
        let path = args.get(1).expect("path");
        std::fs::read_to_string(path).expect("read file")
    } else {
        args.remove(0)
    };
    let result = parse_chart_space_xml(&xml);
    println!("{}", serde_json::to_string(&result).expect("json"));
}
