// SPDX-License-Identifier: MIT
//! Minimal deterministic `a:`-prefixed XML serialization for gradient elements.

use crate::xml::{ns, Element, Node};

pub fn serialize_element(element: &Element) -> String {
    let mut out = String::new();
    write_element(element, &mut out);
    out
}

/// Iterative so a [`crate::xml::MAX_XML_DEPTH`]-deep tree cannot blow the WASM stack.
fn write_element(root: &Element, out: &mut String) {
    struct Frame<'a> {
        el: &'a Element,
        child_idx: usize,
        opened: bool,
        prefix: &'static str,
    }
    let mut stack = vec![Frame {
        el: root,
        child_idx: 0,
        opened: false,
        prefix: "",
    }];
    while let Some(top) = stack.last_mut() {
        if !top.opened {
            let prefix = if top.el.ns.as_deref() == Some(ns::A) || top.el.ns.is_none() {
                "a:"
            } else {
                ""
            };
            top.prefix = prefix;
            out.push('<');
            out.push_str(prefix);
            out.push_str(&top.el.local);
            for attr in &top.el.attrs {
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
            let has_children = top
                .el
                .children
                .iter()
                .any(|c| matches!(c, Node::Element(_)));
            if !has_children {
                out.push_str("/>");
                stack.pop();
                continue;
            }
            out.push('>');
            top.opened = true;
            continue;
        }
        if top.child_idx < top.el.children.len() {
            let idx = top.child_idx;
            top.child_idx += 1;
            match &top.el.children[idx] {
                Node::Text(t) => escape_text(t, out),
                Node::Element(e) => {
                    stack.push(Frame {
                        el: e,
                        child_idx: 0,
                        opened: false,
                        prefix: "",
                    });
                }
            }
            continue;
        }
        let prefix = top.prefix;
        let local = top.el.local.as_str();
        out.push_str("</");
        out.push_str(prefix);
        out.push_str(local);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::parse_xml_str;

    #[test]
    fn catches_recursive_serialize_blowing_wasm_stack_at_max_depth() {
        // Catches: drawingml write_element recursing once per nesting level on a 4096-deep tree.
        let xml = format!("{}{}", "<a>".repeat(4096), "</a>".repeat(4096));
        let root = parse_xml_str(&xml).expect("parses").root;
        let s = serialize_element(&root);
        assert!(s.starts_with("<a:a>"));
        assert!(s.ends_with("</a:a>"));
        assert!(s.len() > 4096);
    }
}
