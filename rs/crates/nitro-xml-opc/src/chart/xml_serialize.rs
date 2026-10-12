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

struct WriteFrame<'a> {
    el: &'a Element,
    emit_root_xmlns: bool,
    child_idx: usize,
    opened: bool,
    close_prefix: String,
}

fn write_element(
    element: &Element,
    out: &mut String,
    emit_root_xmlns: bool,
    prefixes: &HashMap<String, String>,
) {
    let mut stack = vec![WriteFrame {
        el: element,
        emit_root_xmlns,
        child_idx: 0,
        opened: false,
        close_prefix: String::new(),
    }];
    while let Some(frame) = stack.last_mut() {
        if !frame.opened {
            let pfx = element_prefix(frame.el, prefixes);
            frame.close_prefix = pfx.clone();
            out.push('<');
            out.push_str(&pfx);
            out.push_str(&frame.el.local);
            for attr in &frame.el.attrs {
                out.push(' ');
                out.push_str(attr_prefix(attr.ns.as_deref()));
                out.push_str(&attr.local);
                out.push('=');
                out.push('"');
                escape_attr(&attr.value, out);
                out.push('"');
            }
            if frame.emit_root_xmlns {
                if let Some((name, uri)) = xmlns_for_element(frame.el, prefixes) {
                    out.push(' ');
                    out.push_str(&name);
                    out.push('=');
                    out.push('"');
                    escape_attr(&uri, out);
                    out.push('"');
                }
            }
            let parent_uri = frame.el.ns.as_deref();
            let mut declared_child_ns = std::collections::HashSet::new();
            for child in frame.el.children.iter().filter_map(|c| match c {
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
            let has_element_children = frame
                .el
                .children
                .iter()
                .any(|c| matches!(c, Node::Element(_)));
            let has_text = frame.el.children.iter().any(|c| matches!(c, Node::Text(_)));
            if !has_element_children && !has_text {
                out.push_str("/>");
                stack.pop();
                continue;
            }
            out.push('>');
            frame.opened = true;
            continue;
        }
        if frame.child_idx < frame.el.children.len() {
            let child = &frame.el.children[frame.child_idx];
            frame.child_idx += 1;
            match child {
                Node::Element(e) => {
                    stack.push(WriteFrame {
                        el: e,
                        emit_root_xmlns: false,
                        child_idx: 0,
                        opened: false,
                        close_prefix: String::new(),
                    });
                }
                Node::Text(t) => escape_text(t, out),
            }
            continue;
        }
        out.push_str("</");
        out.push_str(&frame.close_prefix);
        out.push_str(&frame.el.local);
        out.push('>');
        stack.pop();
    }
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

/// Every `xmlns:prefix="uri"` in a chart part (for extension element serialization).
pub fn all_uri_prefixes_from_xml(source: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (_, _, prefix, uri) in scan_xmlns_declarations(source) {
        map.insert(uri, prefix);
    }
    map
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::parse_xml_str;

    // Catches: chart `build_xml` recursing once per nesting level and overflowing the WASM stack
    // at MAX_XML_DEPTH (4096).
    #[test]
    fn build_xml_deep_tree_does_not_overflow_stack() {
        let xml = format!("{}x{}", "<a>".repeat(4096), "</a>".repeat(4096));
        let root = parse_xml_str(&xml).expect("parses").root;
        let serialized = build_xml(&root);
        assert!(serialized.contains('x'));
    }
}
