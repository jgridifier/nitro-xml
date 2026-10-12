// SPDX-License-Identifier: MIT
//! `word/document.xml` serialization with required namespace declarations (#80).

use nitro_xml_opc::xml::{ns, Attr, Element, Node};

use super::xml_escape::{escape_attr, escape_text};

fn write_escaped_attr(value: &str, out: &mut String) {
    out.push_str(&escape_attr(value));
}

fn write_escaped_text(value: &str, out: &mut String) {
    out.push_str(&escape_text(value));
}

/// Serialize the document root; the pull-parser drops `xmlns:*` so we re-declare Word namespaces.
pub fn build_document_xml(root: &Element) -> String {
    let mut out = String::new();
    write_element(root, &mut out, true);
    out
}

struct Frame<'a> {
    el: &'a Element,
    child_idx: usize,
    opened: bool,
    emit_xmlns: bool,
}

fn write_element(root: &Element, out: &mut String, emit_root_xmlns: bool) {
    let mut stack = vec![Frame {
        el: root,
        child_idx: 0,
        opened: false,
        emit_xmlns: emit_root_xmlns,
    }];
    while let Some(frame) = stack.last_mut() {
        if !frame.opened {
            let prefix = prefix_for(frame.el);
            out.push('<');
            out.push_str(prefix);
            out.push_str(&frame.el.local);
            for attr in &frame.el.attrs {
                if attr.local.starts_with("xmlns") {
                    continue;
                }
                out.push(' ');
                write_attr(attr, out);
            }
            if frame.emit_xmlns {
                write_xmlns_decls(out, frame.el);
            }
            let has_element_children = frame
                .el
                .children
                .iter()
                .any(|c| matches!(c, Node::Element(_)));
            let has_text = frame
                .el
                .children
                .iter()
                .any(|c| matches!(c, Node::Text(s) if !s.is_empty()));
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
                Node::Text(t) => write_escaped_text(t, out),
                Node::Element(e) => {
                    stack.push(Frame {
                        el: e,
                        child_idx: 0,
                        opened: false,
                        emit_xmlns: false,
                    });
                }
            }
            continue;
        }
        out.push_str("</");
        out.push_str(prefix_for(frame.el));
        out.push_str(&frame.el.local);
        out.push('>');
        stack.pop();
    }
}

fn prefix_for(el: &Element) -> &'static str {
    match el.ns.as_deref() {
        Some(ns::W) => "w:",
        Some(ns::R) => "r:",
        Some(ns::M) => "m:",
        Some(ns::WP) => "wp:",
        Some(ns::A) => "a:",
        Some(ns::PIC) => "pic:",
        Some(ns::MC) => "mc:",
        _ => "",
    }
}

fn write_xmlns_decls(out: &mut String, root: &Element) {
    let mut declared = std::collections::HashSet::new();
    declare_if_needed(out, &mut declared, "w", ns::W);
    declare_if_needed(out, &mut declared, "r", ns::R);
    for (prefix, uri) in used_extra_namespaces(root) {
        if declared.insert(uri.clone()) {
            out.push_str(" xmlns:");
            out.push_str(&prefix);
            out.push_str("=\"");
            write_escaped_attr(&uri, out);
            out.push('"');
        }
    }
}

fn declare_if_needed(
    out: &mut String,
    declared: &mut std::collections::HashSet<String>,
    prefix: &str,
    uri: &str,
) {
    if declared.insert(uri.to_owned()) {
        out.push_str(" xmlns:");
        out.push_str(prefix);
        out.push_str("=\"");
        write_escaped_attr(uri, out);
        out.push('"');
    }
}

fn used_extra_namespaces(root: &Element) -> Vec<(String, String)> {
    let mut out = Vec::new();
    walk_extra_ns(root, &mut out);
    out
}

fn walk_extra_ns(el: &Element, out: &mut Vec<(String, String)>) {
    let known = [ns::W, ns::R];
    let mut note = |uri: &str| {
        if known.contains(&uri) {
            return;
        }
        if let Some(p) = prefix_for_uri(uri) {
            if !out.iter().any(|(_, u)| u == uri) {
                out.push((p.to_string(), uri.to_owned()));
            }
        }
    };
    if let Some(uri) = &el.ns {
        note(uri);
    }
    for attr in &el.attrs {
        if let Some(uri) = &attr.ns {
            note(uri);
        }
    }
    for child in &el.children {
        if let Node::Element(e) = child {
            walk_extra_ns(e, out);
        }
    }
}

fn prefix_for_uri(uri: &str) -> Option<&'static str> {
    match uri {
        ns::M => Some("m"),
        ns::WP => Some("wp"),
        ns::A => Some("a"),
        ns::PIC => Some("pic"),
        ns::MC => Some("mc"),
        _ => None,
    }
}

fn write_attr(attr: &Attr, out: &mut String) {
    if let Some(ns_uri) = &attr.ns {
        if ns_uri == ns::W {
            out.push_str("w:");
        } else if ns_uri == ns::R {
            out.push_str("r:");
        } else if ns_uri == ns::XML {
            out.push_str("xml:");
        }
    }
    out.push_str(&attr.local);
    out.push('=');
    out.push('"');
    write_escaped_attr(&attr.value, out);
    out.push('"');
}
