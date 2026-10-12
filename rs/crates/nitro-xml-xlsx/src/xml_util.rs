// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/xml-util.ts (and relAttr from
// src/core/xml/xml.ts); changes listed in PROVENANCE.md.
//! SpreadsheetML XML helpers over the owned OPC element tree.

use nitro_xml_opc::xml::{ns, Element, Node};

/// Direct SpreadsheetML children named `local`.
pub fn x_children<'a>(parent: &'a Element, local: &str) -> Vec<&'a Element> {
    children_ns(parent, local, ns::X)
}

/// The first SpreadsheetML child named `local`.
pub fn x_first<'a>(parent: Option<&'a Element>, local: &str) -> Option<&'a Element> {
    parent.and_then(|p| first_ns(p, local, ns::X))
}

/// Direct DrawingML children named `local`.
pub fn a_children<'a>(parent: &'a Element, local: &str) -> Vec<&'a Element> {
    children_ns(parent, local, ns::A)
}

/// The first DrawingML child named `local`.
pub fn a_first<'a>(parent: Option<&'a Element>, local: &str) -> Option<&'a Element> {
    parent.and_then(|p| first_ns(p, local, ns::A))
}

/// All direct element children (any namespace).
pub fn elements(parent: &Element) -> impl Iterator<Item = &Element> {
    parent.children.iter().filter_map(|n| match n {
        Node::Element(e) => Some(e),
        Node::Text(_) => None,
    })
}

/// An un-namespaced attribute.
pub fn att<'a>(element: Option<&'a Element>, name: &str) -> Option<&'a str> {
    element.and_then(|el| {
        el.attrs
            .iter()
            .find(|a| a.local == name && a.ns.is_none())
            .map(|a| a.value.as_str())
    })
}

/// Relationship attribute (`r:id`).
pub fn rel_attr<'a>(element: Option<&'a Element>, local: &str) -> Option<&'a str> {
    element.and_then(|el| {
        el.attrs
            .iter()
            .find(|a| a.local == local && a.ns.as_deref() == Some(ns::R))
            .map(|a| a.value.as_str())
    })
}

/// `1`/`true` -> true, `0`/`false` -> false, absent -> `None` or fallback.
pub fn bool_attr(element: Option<&Element>, name: &str) -> Option<bool> {
    let value = att(element, name)?;
    Some(value == "1" || value.eq_ignore_ascii_case("true"))
}

pub fn bool_attr_or(element: Option<&Element>, name: &str, fallback: bool) -> bool {
    bool_attr(element, name).unwrap_or(fallback)
}

/// A numeric attribute, or `None` when absent or not finite.
pub fn num_attr(element: Option<&Element>, name: &str) -> Option<f64> {
    let value = att(element, name)?;
    if value.trim().is_empty() {
        return None;
    }
    let n: f64 = value.parse().ok()?;
    if n.is_finite() {
        Some(n)
    } else {
        None
    }
}

/// The `val` attribute of a SpreadsheetML child element.
pub fn child_val<'a>(parent: &'a Element, local: &str) -> Option<&'a str> {
    att(x_first(Some(parent), local), "val")
}

/// Text of an element, decoding SpreadsheetML `_xHHHH_` escapes.
pub fn x_text(element: Option<&Element>) -> String {
    decode_escapes(&element.map(Element::text_content).unwrap_or_default())
}

/// Decodes `_xHHHH_` escapes (`_x000D_` carriage return, `_x005F_` a literal underscore).
/// All descendant elements named `local` in `ns_uri` (document order).
///
/// Iterative walk so a [`MAX_XML_DEPTH`](nitro_xml_opc::xml::MAX_XML_DEPTH)-deep tree cannot
/// blow the WASM stack.
pub fn descendants<'a>(root: &'a Element, local: &str, ns_uri: &str) -> Vec<&'a Element> {
    let mut out = Vec::new();
    let mut stack: Vec<&Element> = elements(root).collect();
    stack.reverse();
    while let Some(node) = stack.pop() {
        if node.local == local && node.ns.as_deref() == Some(ns_uri) {
            out.push(node);
        }
        let mut children: Vec<&Element> = elements(node).collect();
        children.reverse();
        stack.extend(children);
    }
    out
}

/// Direct children named `local` in `ns_uri`.
pub fn children_ns<'a>(parent: &'a Element, local: &str, ns_uri: &str) -> Vec<&'a Element> {
    parent
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(e) if e.local == local && e.ns.as_deref() == Some(ns_uri) => Some(e),
            _ => None,
        })
        .collect()
}

/// First child named `local` in `ns_uri`.
pub fn first_ns<'a>(parent: &'a Element, local: &str, ns_uri: &str) -> Option<&'a Element> {
    children_ns(parent, local, ns_uri).into_iter().next()
}

/// Trimmed text content of an element, if non-empty.
pub fn element_text(element: Option<&Element>) -> Option<String> {
    let raw = element.map(Element::text_content).unwrap_or_default();
    let t = raw.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_owned())
    }
}

pub fn decode_escapes(text: &str) -> String {
    if !text.contains("_x") {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 7 <= bytes.len()
            && bytes[i] == b'_'
            && bytes[i + 1] == b'x'
            && bytes[i + 6] == b'_'
            && bytes[i + 2].is_ascii_hexdigit()
            && bytes[i + 3].is_ascii_hexdigit()
            && bytes[i + 4].is_ascii_hexdigit()
            && bytes[i + 5].is_ascii_hexdigit()
        {
            let hex = &text[i + 2..i + 6];
            if let Ok(code) = u32::from_str_radix(hex, 16) {
                if let Some(ch) = char::from_u32(code) {
                    out.push(ch);
                    i += 7;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}
