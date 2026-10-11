// SPDX-License-Identifier: MIT
//! WordprocessingML XML helpers over the owned [`Element`] tree.

use nitro_xml_opc::xml::{ns, Attr, Element, Node};

pub fn named_w(el: &Element, local: &str) -> bool {
    el.local == local && el.ns.as_deref() == Some(ns::W)
}

pub fn first_w<'a>(parent: Option<&'a Element>, local: &str) -> Option<&'a Element> {
    parent?
        .element_children()
        .into_iter()
        .find(|e| named_w(e, local))
}

pub fn children_w<'a>(parent: &'a Element, local: &str) -> Vec<&'a Element> {
    parent
        .element_children()
        .into_iter()
        .filter(|e| named_w(e, local))
        .collect()
}

pub fn get_w(el: Option<&Element>, attr: &str) -> Option<String> {
    el?.attr_w(attr)
}

pub fn get_r(el: Option<&Element>, attr: &str) -> Option<String> {
    el?.attr_ns(ns::R, attr)
}

pub fn named_mc_choice(el: &Element) -> bool {
    el.local == "Choice"
        && el.ns.as_deref() == Some("http://schemas.openxmlformats.org/markup-compatibility/2006")
}

pub fn attr_ns(el: &Element, namespace: &str, local: &str) -> Option<String> {
    el.attrs
        .iter()
        .find(|a| a.local == local && a.ns.as_deref() == Some(namespace))
        .map(|a| a.value.clone())
}

pub fn first_ns<'a>(
    parent: Option<&'a Element>,
    namespace: &str,
    local: &str,
) -> Option<&'a Element> {
    parent?
        .element_children()
        .into_iter()
        .find(|e| e.local == local && e.ns.as_deref() == Some(namespace))
}

pub fn descendant_ns<'a>(
    parent: Option<&'a Element>,
    namespace: &str,
    local: &str,
) -> Option<&'a Element> {
    let parent = parent?;
    let mut stack: Vec<&Element> = vec![parent];
    while let Some(node) = stack.pop() {
        for child in node.element_children() {
            if child.local == local && child.ns.as_deref() == Some(namespace) {
                return Some(child);
            }
            stack.push(child);
        }
    }
    None
}

pub fn text_content(el: &Element) -> String {
    el.text_content()
}

/// Serialize an element subtree to a string (equation OMML and revision snapshots).
pub fn serialize_element(el: &Element) -> String {
    fn write_el(el: &Element, out: &mut String) {
        out.push('<');
        if let Some(ns_uri) = &el.ns {
            if let Some(prefix) = ns_prefix(ns_uri) {
                out.push_str(prefix);
                out.push(':');
            }
        }
        out.push_str(&el.local);
        for attr in &el.attrs {
            out.push(' ');
            write_attr(attr, out);
        }
        let child_els: Vec<_> = el.element_children();
        if child_els.is_empty() && !has_text(el) {
            out.push_str("/>");
            return;
        }
        out.push('>');
        for child in &el.children {
            match child {
                Node::Text(t) => escape_text(t, out),
                Node::Element(e) => write_el(e, out),
            }
        }
        out.push_str("</");
        if let Some(ns_uri) = &el.ns {
            if let Some(prefix) = ns_prefix(ns_uri) {
                out.push_str(prefix);
                out.push(':');
            }
        }
        out.push_str(&el.local);
        out.push('>');
    }
    fn write_attr(attr: &Attr, out: &mut String) {
        if let Some(ns_uri) = &attr.ns {
            if let Some(prefix) = ns_prefix(ns_uri) {
                out.push_str(prefix);
                out.push(':');
            }
        }
        out.push_str(&attr.local);
        out.push('=');
        out.push('"');
        for c in attr.value.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '"' => out.push_str("&quot;"),
                _ => out.push(c),
            }
        }
        out.push('"');
    }
    fn escape_text(s: &str, out: &mut String) {
        for c in s.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                _ => out.push(c),
            }
        }
    }
    fn has_text(el: &Element) -> bool {
        el.children
            .iter()
            .any(|n| matches!(n, Node::Text(s) if !s.is_empty()))
    }
    fn ns_prefix(uri: &str) -> Option<&'static str> {
        match uri {
            ns::W => Some("w"),
            ns::R => Some("r"),
            ns::M => Some("m"),
            "http://schemas.openxmlformats.org/markup-compatibility/2006" => Some("mc"),
            _ => None,
        }
    }
    let mut out = String::new();
    write_el(el, &mut out);
    out
}

pub fn by_name_w<'a>(root: &'a Element, local: &str) -> Vec<&'a Element> {
    let mut out = Vec::new();
    collect_named_w(root, local, &mut out);
    out
}

pub fn first_body(document_root: &Element) -> Option<&Element> {
    by_name_w(document_root, "body").into_iter().next()
}

fn collect_named_w<'a>(el: &'a Element, local: &str, out: &mut Vec<&'a Element>) {
    if named_w(el, local) {
        out.push(el);
    }
    for child in el.element_children() {
        collect_named_w(child, local, out);
    }
}

pub trait ElementExt {
    fn element_children(&self) -> Vec<&Element>;
    fn attr_w(&self, local: &str) -> Option<String>;
    fn attr_ns(&self, namespace: &str, local: &str) -> Option<String>;
}

impl ElementExt for Element {
    fn element_children(&self) -> Vec<&Element> {
        self.children
            .iter()
            .filter_map(|n| match n {
                Node::Element(e) => Some(e),
                Node::Text(_) => None,
            })
            .collect()
    }

    fn attr_w(&self, local: &str) -> Option<String> {
        self.attrs
            .iter()
            .find(|a| a.local == local && a.ns.as_deref() == Some(ns::W))
            .or_else(|| self.attrs.iter().find(|a| a.local == local))
            .map(|a| a.value.clone())
    }

    fn attr_ns(&self, namespace: &str, local: &str) -> Option<String> {
        attr_ns(self, namespace, local)
    }
}
