// SPDX-License-Identifier: MIT
//! Small `w:` element helpers for the document writer (#80).

use nitro_xml_opc::xml::{ns, parse_xml_str, Attr, Element, Node};

use crate::xml_util::ElementExt;

pub fn make_w(local: &str) -> Element {
    let xml = format!(r#"<w:{} xmlns:w="{}"/>"#, local, ns::W);
    parse_xml_str(&xml)
        .unwrap_or_else(|e| panic!("make_w({local}): {e}"))
        .root
}

pub fn is_body_block(el: &Element) -> bool {
    el.ns.as_deref() == Some(ns::W) && (el.local == "p" || el.local == "tbl")
}

pub fn set_w_attr(el: &mut Element, local: &str, value: &str) {
    if let Some(attr) = el
        .attrs
        .iter_mut()
        .find(|a| a.local == local && a.ns.as_deref() == Some(ns::W))
    {
        attr.value = value.to_owned();
        return;
    }
    el.attrs.push(Attr {
        ns: Some(ns::W.to_owned()),
        local: local.to_owned(),
        value: value.to_owned(),
    });
}

pub fn remove_w_children(parent: &mut Element, local: &str) {
    parent.children.retain(|child| {
        !matches!(
            child,
            Node::Element(e) if e.local == local && e.ns.as_deref() == Some(ns::W)
        )
    });
}

pub fn first_w_mut<'a>(parent: &'a mut Element, local: &str) -> Option<&'a mut Element> {
    for child in &mut parent.children {
        if let Node::Element(e) = child {
            if e.local == local && e.ns.as_deref() == Some(ns::W) {
                return Some(e);
            }
        }
    }
    None
}

pub fn first_w<'a>(parent: &'a Element, local: &str) -> Option<&'a Element> {
    parent
        .element_children()
        .into_iter()
        .find(|e| e.local == local && e.ns.as_deref() == Some(ns::W))
}

pub fn children_w<'a>(parent: &'a Element, local: &str) -> Vec<&'a Element> {
    parent
        .element_children()
        .into_iter()
        .filter(|e| e.local == local && e.ns.as_deref() == Some(ns::W))
        .collect()
}

/// Replace body `w:p` / `w:tbl` nodes in order; extra outputs insert before the trailing `w:sectPr`.
pub fn replace_body_blocks(body: &mut Element, outputs: &[Element]) {
    let sect_pr = body
        .children
        .iter()
        .position(|n| {
            matches!(
                n,
                Node::Element(e) if e.local == "sectPr" && e.ns.as_deref() == Some(ns::W)
            )
        })
        .map(|i| body.children.remove(i));

    let mut out_i = 0;
    for child in &mut body.children {
        if let Node::Element(e) = child {
            if is_body_block(e) && out_i < outputs.len() {
                *child = Node::Element(outputs[out_i].clone());
                out_i += 1;
            }
        }
    }

    if out_i < outputs.len() {
        let insert_at = body.children.len();
        for block in outputs.iter().skip(out_i) {
            body.children
                .insert(insert_at, Node::Element(block.clone()));
        }
    }

    if let Some(sp) = sect_pr {
        body.children.push(sp);
    }
}

/// Replace direct children with the given WordprocessingML local name.
pub fn replace_children_local(
    parent: &mut Element,
    local: &str,
    old_len: usize,
    outputs: &[Element],
) {
    let mut idx = 0;
    parent.children.retain_mut(|child| {
        if matches!(
            child,
            Node::Element(e) if e.local == local && e.ns.as_deref() == Some(ns::W)
        ) {
            if idx < old_len {
                if idx < outputs.len() {
                    *child = Node::Element(outputs[idx].clone());
                }
                idx += 1;
                return idx <= outputs.len();
            }
            return false;
        }
        true
    });
    for block in outputs.iter().skip(idx) {
        parent.children.push(Node::Element(block.clone()));
    }
}

/// Replace direct `w:p` / `w:tbl` children; drops surplus old block nodes.
pub fn replace_slots(parent: &mut Element, old_len: usize, outputs: &[Element]) {
    let mut block_idx = 0;
    parent.children.retain_mut(|child| {
        if !matches!(child, Node::Element(e) if is_body_block(e)) {
            return true;
        }
        if block_idx >= old_len {
            return false;
        }
        if block_idx < outputs.len() {
            *child = Node::Element(outputs[block_idx].clone());
            block_idx += 1;
            true
        } else {
            block_idx += 1;
            false
        }
    });
    for block in outputs.iter().skip(block_idx) {
        parent.children.push(Node::Element(block.clone()));
    }
}
