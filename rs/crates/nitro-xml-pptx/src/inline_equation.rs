// SPDX-License-Identifier: MIT
//! Minimal inline OMML → MathML for visual parity on simple `m:t` equations (#27).

use nitro_xml_opc::xml::{ns, Element, Node};

/// Build MathML for a simple inline `m:oMath` tree (single-variable fixtures).
pub fn simple_omml_to_mathml(omml: &Element) -> Option<String> {
    let text = collect_omml_plain_text(omml);
    if text.is_empty() {
        return None;
    }
    let escaped = escape_xml_text(&text);
    Some(format!(
        r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><mi>{escaped}</mi></math>"#
    ))
}

fn collect_omml_plain_text(el: &Element) -> String {
    let mut out = String::new();
    walk_omml(el, &mut out);
    out
}

fn walk_omml(el: &Element, out: &mut String) {
    if el.ns.as_deref() == Some(ns::M) && el.local == "t" {
        out.push_str(&el.text_content());
    }
    for child in &el.children {
        if let Node::Element(e) = child {
            walk_omml(e, out);
        }
    }
}

fn escape_xml_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}
