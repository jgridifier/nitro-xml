// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/drawingml/dom.ts and src/core/xml/xml.ts;
// changes listed in PROVENANCE.md.

use crate::xml::{ns, Element, Node};

pub fn attr<'a>(element: &'a Element, name: &str) -> Option<&'a str> {
    element
        .attrs
        .iter()
        .find(|a| a.local == name)
        .map(|a| a.value.as_str())
}

pub fn attr_ns<'a>(element: &'a Element, namespace: &str, local: &str) -> Option<&'a str> {
    element
        .attrs
        .iter()
        .find(|a| a.local == local && a.ns.as_deref() == Some(namespace))
        .map(|a| a.value.as_str())
}

#[allow(dead_code)]
pub fn string_attribute(element: &Element, name: &str) -> Option<String> {
    attr(element, name)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

pub fn integer_attribute(element: &Element, name: &str) -> Option<i64> {
    attr(element, name).and_then(|v| v.parse::<i64>().ok())
}

pub fn boolean_attribute(element: &Element, name: &str) -> Option<bool> {
    match attr(element, name).map(|s| s.trim().to_ascii_lowercase()) {
        Some(v) if v == "1" || v == "true" => Some(true),
        Some(v) if v == "0" || v == "false" => Some(false),
        _ => None,
    }
}

pub fn is_drawing_element(element: &Element) -> bool {
    element.ns.as_deref() == Some(ns::A) || element.ns.is_none()
}

pub fn elements(parent: &Element) -> Vec<&Element> {
    parent
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(e) if is_drawing_element(e) => Some(e),
            _ => None,
        })
        .collect()
}

#[allow(dead_code)]
pub fn all_elements(parent: &Element) -> Vec<&Element> {
    parent
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
        .collect()
}

pub fn named(element: &Element, local: &str, namespace: &str, lenient: bool) -> bool {
    if element.local != local {
        return false;
    }
    element.ns.as_deref() == Some(namespace) || (lenient && element.ns.is_none())
}

pub fn children<'a>(parent: &'a Element, local: &str, namespace: &str) -> Vec<&'a Element> {
    elements(parent)
        .into_iter()
        .filter(|e| named(e, local, namespace, true))
        .collect()
}

pub fn first<'a>(parent: &'a Element, local: &str, namespace: &str) -> Option<&'a Element> {
    children(parent, local, namespace).into_iter().next()
}

#[allow(dead_code)]
pub fn descendants<'a>(root: &'a Element, local: &str) -> Vec<&'a Element> {
    let mut result = Vec::new();
    fn visit<'a>(node: &'a Element, local: &str, out: &mut Vec<&'a Element>) {
        for child in all_elements(node) {
            if child.local == local {
                out.push(child);
            }
            visit(child, local, out);
        }
    }
    visit(root, local, &mut result);
    result
}
