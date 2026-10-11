// SPDX-License-Identifier: MIT
//! Minimal deterministic `a:`-prefixed XML serialization for gradient elements.

use crate::xml::{ns, Element, Node};

pub fn serialize_element(element: &Element) -> String {
    let mut out = String::new();
    write_element(element, &mut out);
    out
}

fn write_element(element: &Element, out: &mut String) {
    let prefix = if element.ns.as_deref() == Some(ns::A) || element.ns.is_none() {
        "a:"
    } else {
        ""
    };
    out.push('<');
    out.push_str(prefix);
    out.push_str(&element.local);
    for attr in &element.attrs {
        out.push(' ');
        if let Some(ns_uri) = &attr.ns {
            if ns_uri == ns::R {
                out.push_str("r:");
            }
        }
        out.push_str(&attr.local);
        out.push('=');
        out.push('"');
        escape_attr(&attr.value, out);
        out.push('"');
    }
    let has_children = element
        .children
        .iter()
        .any(|c| matches!(c, Node::Element(_)));
    if !has_children {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for child in &element.children {
        match child {
            Node::Element(e) => write_element(e, out),
            Node::Text(t) => escape_text(t, out),
        }
    }
    out.push_str("</");
    out.push_str(prefix);
    out.push_str(&element.local);
    out.push('>');
}

fn escape_attr(value: &str, out: &mut String) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            _ => out.push(ch),
        }
    }
}

fn escape_text(value: &str, out: &mut String) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            _ => out.push(ch),
        }
    }
}
