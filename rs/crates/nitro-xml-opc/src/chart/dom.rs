// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xml/xml.ts and src/core/chart/parse-util.ts;
// changes listed in PROVENANCE.md.

use crate::xml::{ns, Element, Node};

fn named(element: &Element, local: &str, namespace: &str, lenient: bool) -> bool {
    if element.local != local {
        return false;
    }
    element.ns.as_deref() == Some(namespace) || (lenient && element.ns.is_none())
}

pub fn attr<'a>(element: &'a Element, name: &str) -> Option<&'a str> {
    element
        .attrs
        .iter()
        .find(|a| a.local == name)
        .map(|a| a.value.as_str())
}

pub fn chart_elements(parent: &Element) -> Vec<&Element> {
    parent
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
        .collect()
}

pub fn first_ns<'a>(
    parent: Option<&'a Element>,
    local: &str,
    namespace: &str,
) -> Option<&'a Element> {
    parent.and_then(|p| {
        chart_elements(p)
            .into_iter()
            .find(|e| named(e, local, namespace, true))
    })
}

pub fn c_child<'a>(parent: Option<&'a Element>, local: &str) -> Option<&'a Element> {
    parent.and_then(|p| {
        chart_elements(p)
            .into_iter()
            .find(|e| named(e, local, ns::C, true))
    })
}

pub fn first<'a>(parent: &'a Element, local: &str, namespace: &str) -> Option<&'a Element> {
    chart_elements(parent)
        .into_iter()
        .find(|e| named(e, local, namespace, true))
}

pub fn c_children<'a>(parent: &'a Element, local: &str) -> Vec<&'a Element> {
    chart_elements(parent)
        .into_iter()
        .filter(|e| named(e, local, ns::C, true))
        .collect()
}

pub fn mc_children<'a>(parent: &'a Element, local: &str) -> Vec<&'a Element> {
    chart_elements(parent)
        .into_iter()
        .filter(|e| named(e, local, ns::MC, true))
        .collect()
}

pub fn rel_attr(element: Option<&Element>, local: &str) -> Option<String> {
    element.and_then(|e| {
        attr_ns(e, local, ns::R, "r")
            .or_else(|| attr(e, &format!("r:{}", local)))
            .map(|s| s.to_string())
    })
}

pub fn attr_ns<'a>(
    element: &'a Element,
    local: &str,
    namespace: &str,
    prefix: &str,
) -> Option<&'a str> {
    element
        .attrs
        .iter()
        .find(|a| a.local == local && a.ns.as_deref() == Some(namespace))
        .map(|a| a.value.as_str())
        .or_else(|| {
            let q = format!("{}:{}", prefix, local);
            attr(element, &q)
        })
}
