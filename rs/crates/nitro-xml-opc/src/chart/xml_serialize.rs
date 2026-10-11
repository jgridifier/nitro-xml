// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xml/xml.ts (buildXml) and
// src/core/chart/xml-fragment.ts; changes listed in PROVENANCE.md.

use crate::xml::{ns, Element, Node};
use std::collections::HashMap;

fn static_prefix(uri: &str) -> &'static str {
    match uri {
        ns::C => "c",
        ns::A => "a",
        ns::R => "r",
        ns::MC => "mc",
        ns::C14 => "c14",
        ns::C16 => "c16",
        ns::C16R2 => "c16r2",
        _ => "",
    }
}

fn element_prefix(element: &Element, prefixes: &HashMap<String, String>) -> String {
    if let Some(uri) = element.ns.as_deref() {
        if let Some(p) = prefixes.get(uri) {
            return format!("{}:", p);
        }
        let p = static_prefix(uri);
        if !p.is_empty() {
            return format!("{}:", p);
        }
    }
    String::new()
}

fn attr_prefix(ns_uri: Option<&str>) -> &'static str {
    match ns_uri {
        Some(ns::R) => "r:",
        Some(ns::XML) => "xml:",
        _ => "",
    }
}

fn xmlns_for_element(
    element: &Element,
    prefixes: &HashMap<String, String>,
) -> Option<(String, String)> {
    let uri = element.ns.as_deref()?;
    let prefix = prefixes.get(uri).cloned().or_else(|| {
        let p = static_prefix(uri);
        if p.is_empty() {
            None
        } else {
            Some(p.to_string())
        }
    })?;
    Some((format!("xmlns:{}", prefix), uri.to_string()))
}

pub fn build_xml(element: &Element) -> String {
    build_xml_with_prefixes(element, &HashMap::new())
}

pub fn build_xml_with_prefixes(element: &Element, prefixes: &HashMap<String, String>) -> String {
    let mut out = String::new();
    write_element(element, &mut out, true, prefixes);
    out
}

fn write_element(
    element: &Element,
    out: &mut String,
    emit_root_xmlns: bool,
    prefixes: &HashMap<String, String>,
) {
    let pfx = element_prefix(element, prefixes);
    out.push('<');
    out.push_str(&pfx);
    out.push_str(&element.local);
    for attr in &element.attrs {
        out.push(' ');
        out.push_str(attr_prefix(attr.ns.as_deref()));
        out.push_str(&attr.local);
        out.push('=');
        out.push('"');
        escape_attr(&attr.value, out);
        out.push('"');
    }
    if emit_root_xmlns {
        if let Some((name, uri)) = xmlns_for_element(element, prefixes) {
            out.push(' ');
            out.push_str(&name);
            out.push('=');
            out.push('"');
            escape_attr(&uri, out);
            out.push('"');
        }
    }
    let parent_uri = element.ns.as_deref();
    let mut declared_child_ns = std::collections::HashSet::new();
    for child in element.children.iter().filter_map(|c| match c {
        Node::Element(e) => Some(e),
        _ => None,
    }) {
        let uri = child.ns.as_deref();
        if uri.is_none() || uri == parent_uri {
            continue;
        }
        if !declared_child_ns.insert(uri.unwrap()) {
            continue;
        }
        if let Some((name, uri_s)) = xmlns_for_element(child, prefixes) {
            out.push(' ');
            out.push_str(&name);
            out.push('=');
            out.push('"');
            escape_attr(&uri_s, out);
            out.push('"');
        }
    }
    let has_element_children = element
        .children
        .iter()
        .any(|c| matches!(c, Node::Element(_)));
    let has_text = element.children.iter().any(|c| matches!(c, Node::Text(_)));
    if !has_element_children && !has_text {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for child in &element.children {
        match child {
            Node::Element(e) => write_element(e, out, false, prefixes),
            Node::Text(t) => escape_text(t, out),
        }
    }
    out.push_str("</");
    out.push_str(&pfx);
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

const CHART_ROOT_BINDINGS: [(&str, &str); 3] = [("c", ns::C), ("a", ns::A), ("r", ns::R)];

fn chart_root_bindings() -> HashMap<&'static str, &'static str> {
    HashMap::from(CHART_ROOT_BINDINGS)
}

fn scan_xmlns_declarations(xml: &str) -> Vec<(usize, usize, String, String)> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while let Some(idx) = xml[pos..].find("xmlns:") {
        let start = pos + idx;
        let rest = &xml[start + 6..];
        let Some(eq) = rest.find('=') else {
            pos = start + 6;
            continue;
        };
        let prefix = rest[..eq].trim().to_string();
        let after_eq = rest[eq + 1..].trim_start();
        let quote_off = rest[eq + 1..].len() - after_eq.len();
        if let Some(stripped) = after_eq.strip_prefix('"') {
            if let Some(end_q) = stripped.find('"') {
                let uri = stripped[..end_q].to_string();
                let end = start + 6 + eq + 1 + quote_off + 1 + end_q + 1;
                out.push((start, end, prefix, uri));
                pos = end;
                continue;
            }
        }
        pos = start + 6;
    }
    out
}

/// Removes xmlns declarations that repeat chart root bindings (parity with `stripDeclarations`).
pub fn strip_declarations(xml: &str, bindings: &HashMap<&str, &str>) -> String {
    let decls = scan_xmlns_declarations(xml);
    let mut conflicting = std::collections::HashSet::new();
    for (_, _, prefix, uri) in &decls {
        if bindings.contains_key(prefix.as_str())
            && bindings.get(prefix.as_str()).copied() != Some(uri.as_str())
        {
            conflicting.insert(prefix.clone());
        }
    }
    let mut out = String::new();
    let mut pos = 0usize;
    for (start, end, prefix, uri) in decls {
        out.push_str(&xml[pos..start]);
        if bindings.get(prefix.as_str()).copied() == Some(uri.as_str())
            && !conflicting.contains(&prefix)
        {
            pos = end;
        } else {
            out.push_str(&xml[start..end]);
            pos = end;
        }
    }
    out.push_str(&xml[pos..]);
    out
}

pub fn inner_xml(element: &Element) -> String {
    let bindings = chart_root_bindings();
    let joined = super::dom::chart_elements(element)
        .iter()
        .map(|e| build_xml(e))
        .collect::<String>();
    strip_declarations(&joined, &bindings)
}

pub fn extension_list(
    parent: Option<&Element>,
    prefixes: &HashMap<String, String>,
) -> Option<String> {
    let ext_lst = super::dom::c_child(parent, "extLst");
    ext_lst.map(|e| build_xml_with_prefixes(e, prefixes))
}
