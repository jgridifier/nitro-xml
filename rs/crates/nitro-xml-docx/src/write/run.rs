// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/write-run.ts; changes listed in PROVENANCE.md.

use serde_json::Value;

use nitro_xml_opc::xml::{ns, Attr, Element, Node};

use super::element::{first_w_mut, make_w, remove_w_children, set_w_attr};
use super::xml_escape::encode_escapes;

pub fn run_property_changed(run: &Value, base: Option<&Value>, key: &str) -> bool {
    match base {
        None => run.get(key).is_some(),
        Some(b) => run.get(key) != b.get(key),
    }
}

pub fn create_run(run: &Value, base: Option<&Value>, old: Option<&Element>) -> Element {
    let mut node = old.cloned().unwrap_or_else(|| make_w("r"));
    set_run_properties(&mut node, run, base);
    node.children
        .retain(|n| matches!(n, Node::Element(e) if e.local == "rPr"));

    let text = run.get("text").and_then(|t| t.as_str()).unwrap_or("");
    if run.get("break").is_some() {
        let mut br = make_w("br");
        if let Some(ty) = run.get("break").and_then(|t| t.as_str()) {
            set_w_attr(&mut br, "type", ty);
        }
        node.children.push(Node::Element(br));
        return node;
    }

    for piece in split_text_pieces(text) {
        match piece {
            TextPiece::Newline => node.children.push(Node::Element(make_w("br"))),
            TextPiece::Tab => node.children.push(Node::Element(make_w("tab"))),
            TextPiece::Text(s) if !s.is_empty() => {
                let mut t = make_w("t");
                if s.starts_with(' ') || s.ends_with(' ') {
                    t.attrs.push(Attr {
                        ns: Some(ns::XML.to_owned()),
                        local: "space".to_owned(),
                        value: "preserve".to_owned(),
                    });
                }
                t.children.push(Node::Text(encode_escapes(&s)));
                node.children.push(Node::Element(t));
            }
            TextPiece::Text(_) => {}
        }
    }
    if text.is_empty()
        && !node
            .children
            .iter()
            .any(|n| matches!(n, Node::Element(e) if e.local == "t"))
    {
        node.children.push(Node::Element(make_w("t")));
    }
    node
}

enum TextPiece {
    Text(String),
    Newline,
    Tab,
}

fn split_text_pieces(text: &str) -> Vec<TextPiece> {
    let mut out = Vec::new();
    let mut buf = String::new();
    for ch in text.chars() {
        if ch == '\n' {
            if !buf.is_empty() {
                out.push(TextPiece::Text(buf.clone()));
                buf.clear();
            }
            out.push(TextPiece::Newline);
        } else if ch == '\t' {
            if !buf.is_empty() {
                out.push(TextPiece::Text(buf.clone()));
                buf.clear();
            }
            out.push(TextPiece::Tab);
        } else {
            buf.push(ch);
        }
    }
    if !buf.is_empty() {
        out.push(TextPiece::Text(buf));
    }
    out
}

fn set_run_properties(run_node: &mut Element, run: &Value, base: Option<&Value>) {
    let needs_props = [
        "bold",
        "italic",
        "underline",
        "fontSize",
        "color",
        "strike",
        "highlight",
    ]
    .iter()
    .any(|k| run_property_changed(run, base, k));
    if !needs_props {
        return;
    }
    if first_w_mut(run_node, "rPr").is_none() {
        run_node.children.insert(0, Node::Element(make_w("rPr")));
    }
    let props = first_w_mut(run_node, "rPr").expect("rPr");

    if run_property_changed(run, base, "bold") {
        set_toggle(props, "b", run.get("bold").and_then(|v| v.as_bool()));
    }
    if run_property_changed(run, base, "italic") {
        set_toggle(props, "i", run.get("italic").and_then(|v| v.as_bool()));
    }
    if run_property_changed(run, base, "underline") {
        remove_w_children(props, "u");
        if run.get("underline").and_then(|v| v.as_bool()) == Some(false) {
            let mut u = make_w("u");
            set_w_attr(&mut u, "val", "none");
            props.children.push(Node::Element(u));
        } else if run.get("underline").and_then(|v| v.as_bool()) == Some(true) {
            let mut u = make_w("u");
            let style = run
                .get("underlineStyle")
                .and_then(|v| v.as_str())
                .unwrap_or("single");
            set_w_attr(&mut u, "val", style);
            props.children.push(Node::Element(u));
        }
    }
    if run_property_changed(run, base, "fontSize") {
        remove_w_children(props, "sz");
        if let Some(size) = run.get("fontSize").and_then(|v| v.as_f64()) {
            let mut sz = make_w("sz");
            set_w_attr(&mut sz, "val", &format!("{}", (size * 2.0).round() as i64));
            props.children.push(Node::Element(sz));
        }
    }
    if run_property_changed(run, base, "color") {
        remove_w_children(props, "color");
        if let Some(color) = run.get("color").and_then(|v| v.as_str()) {
            let hex = color.trim_start_matches('#');
            if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                let mut c = make_w("color");
                set_w_attr(&mut c, "val", hex);
                props.children.push(Node::Element(c));
            }
        }
    }
    order_rpr_children(props);
    if props.children.is_empty() {
        run_node
            .children
            .retain(|n| !matches!(n, Node::Element(e) if e.local == "rPr"));
    }
}

fn order_rpr_children(props: &mut Element) {
    const ORDER: &[&str] = &[
        "rStyle",
        "rFonts",
        "b",
        "bCs",
        "i",
        "iCs",
        "caps",
        "smallCaps",
        "strike",
        "dstrike",
        "color",
        "sz",
        "szCs",
        "highlight",
        "u",
        "vertAlign",
        "rtl",
        "lang",
    ];
    props.children.sort_by_key(|child| match child {
        Node::Element(e) => ORDER
            .iter()
            .position(|name| *name == e.local)
            .unwrap_or(ORDER.len()),
        Node::Text(_) => ORDER.len() + 1,
    });
}

fn set_toggle(props: &mut Element, local: &str, value: Option<bool>) {
    remove_w_children(props, local);
    if value.is_none() {
        return;
    }
    let mut el = make_w(local);
    if value == Some(false) {
        set_w_attr(&mut el, "val", "0");
    }
    props.children.push(Node::Element(el));
}
