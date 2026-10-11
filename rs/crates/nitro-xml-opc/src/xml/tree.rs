// SPDX-License-Identifier: MIT
//! Minimal owned element tree for small OPC parts (rels, styles, theme).

use std::ops::{Deref, DerefMut};

/// An attribute with a resolved namespace URI (or none) and local name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub ns: Option<String>,
    pub local: String,
    pub value: String,
}

/// A child of an element: nested element or character data (text / CDATA merged).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    Text(String),
}

/// An element with resolved namespace, local name, attributes and children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub ns: Option<String>,
    pub local: String,
    pub attrs: Vec<Attr>,
    pub children: Vec<Node>,
}

impl Element {
    pub(crate) fn new(ns: Option<String>, local: String, attrs: Vec<Attr>) -> Self {
        Self {
            ns,
            local,
            attrs,
            children: Vec::new(),
        }
    }

    /// Depth-first text concatenation (DOM `textContent` subset).
    pub fn text_content(&self) -> String {
        let mut out = String::new();
        for child in &self.children {
            match child {
                Node::Text(t) => out.push_str(t),
                Node::Element(e) => out.push_str(&e.text_content()),
            }
        }
        out
    }
}

/// Document with a single root element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub root: Element,
}

impl Deref for Document {
    type Target = Element;
    fn deref(&self) -> &Self::Target {
        &self.root
    }
}

impl DerefMut for Document {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.root
    }
}

/// Iteratively clear a deep tree so `Drop` cannot blow the stack on 2,000-deep documents.
impl Drop for Element {
    fn drop(&mut self) {
        let mut stack = Vec::new();
        stack.append(&mut self.children);
        while let Some(node) = stack.pop() {
            if let Node::Element(mut el) = node {
                stack.append(&mut el.children);
            }
        }
    }
}
