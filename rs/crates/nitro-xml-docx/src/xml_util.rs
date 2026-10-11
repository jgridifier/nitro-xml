// SPDX-License-Identifier: MIT
//! WordprocessingML XML helpers over the owned [`Element`] tree.

use nitro_xml_opc::xml::{ns, Element, Node};

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
}
