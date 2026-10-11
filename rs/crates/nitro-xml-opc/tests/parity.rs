// SPDX-License-Identifier: MIT
//! Foundation dump shape checks for the #5 parity harness.
//! Format crates add insta view-IR snapshots when their parsers land (#15/#20/#24).

use nitro_xml_opc::xml::{parse_xml_bytes, Element, Node};
use serde_json::{json, Value};

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

#[test]
fn xml_dump_shape_keeps_xmlns_out_of_attrs_and_merges_adjacent_text() {
    // Catches: xmlns declarations leaking into attr dumps (false OPC/XML parity diffs).
    let doc = parse_xml_bytes(
        br#"<r xmlns="urn:d" xmlns:p="urn:p"><p:c>a<!--x-->b</p:c></r>"#,
    )
    .expect("parse");
    let tree = element_to_dump_value(&doc.root);
    let arr = tree.as_array().expect("tree array");
    assert_eq!(arr[0], json!("e"));
    assert_eq!(arr[1], json!("urn:d"));
    assert_eq!(arr[2], json!("r"));
    assert_eq!(arr[3], json!([]), "xmlns* must not appear as attributes");
    let child = arr[4].as_array().expect("child");
    assert_eq!(child[0], json!("e"));
    assert_eq!(child[2], json!("c"));
    // Comment dropped; adjacent text merged.
    assert_eq!(child[4], json!(["t", "ab"]));
}
