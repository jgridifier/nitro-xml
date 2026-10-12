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
#[derive(Debug, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    Text(String),
}

/// An element with resolved namespace, local name, attributes and children.
#[derive(Debug, PartialEq, Eq)]
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
    ///
    /// Iterative so a [`MAX_XML_DEPTH`](super::MAX_XML_DEPTH)-deep tree cannot blow the WASM stack.
    pub fn text_content(&self) -> String {
        let mut out = String::new();
        let mut stack: Vec<&Node> = self.children.iter().rev().collect();
        while let Some(node) = stack.pop() {
            match node {
                Node::Text(t) => out.push_str(t),
                Node::Element(e) => {
                    stack.extend(e.children.iter().rev());
                }
            }
        }
        out
    }
}

/// Document with a single root element.
#[derive(Debug, PartialEq, Eq)]
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

/// Iterative deep clone: never recurses per nesting level (WASM stack-safe at 4096).
impl Clone for Element {
    fn clone(&self) -> Self {
        // Post-order stack: (source, next child index, children accumulated so far).
        let mut stack: Vec<(&Element, usize, Vec<Node>)> = vec![(self, 0, Vec::new())];
        loop {
            let Some((src, idx, acc)) = stack.pop() else {
                unreachable!("clone stack emptied without producing a root");
            };
            if idx < src.children.len() {
                stack.push((src, idx + 1, acc));
                match &src.children[idx] {
                    Node::Text(t) => {
                        // Re-open the parent frame and append the text node.
                        let Some((p_src, p_idx, mut p_acc)) = stack.pop() else {
                            unreachable!("parent frame missing for text child");
                        };
                        p_acc.push(Node::Text(t.clone()));
                        stack.push((p_src, p_idx, p_acc));
                    }
                    Node::Element(child) => {
                        stack.push((child, 0, Vec::with_capacity(child.children.len())));
                    }
                }
                continue;
            }
            let cloned = Element {
                ns: src.ns.clone(),
                local: src.local.clone(),
                attrs: src.attrs.clone(),
                children: acc,
            };
            if stack.is_empty() {
                return cloned;
            }
            let Some((p_src, p_idx, mut p_acc)) = stack.pop() else {
                unreachable!("parent frame missing for finished element");
            };
            p_acc.push(Node::Element(cloned));
            stack.push((p_src, p_idx, p_acc));
        }
    }
}

impl Clone for Node {
    fn clone(&self) -> Self {
        match self {
            Node::Text(t) => Node::Text(t.clone()),
            Node::Element(e) => Node::Element(e.clone()),
        }
    }
}

impl Clone for Document {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
        }
    }
}

/// Iteratively clear a deep tree so `Drop` cannot blow the stack on 4096-deep documents.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::parse_xml_str;

    fn deep_chain(depth: usize) -> Element {
        let xml = format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth));
        parse_xml_str(&xml).expect("deep chain parses").root
    }

    #[test]
    fn catches_recursive_clone_blowing_wasm_stack_at_max_depth() {
        // Catches: #[derive(Clone)] on Element recursing once per nesting level and overflowing
        // the WASM stack at MAX_XML_DEPTH (4096).
        let root = deep_chain(4096);
        let cloned = root.clone();
        assert_eq!(cloned.local, "a");
        // Walk cloned depth iteratively to prove structure survived.
        let mut cur = &cloned;
        let mut n = 1usize;
        while let Some(Node::Element(child)) = cur.children.first() {
            cur = child;
            n += 1;
        }
        assert_eq!(n, 4096);
    }

    #[test]
    fn catches_recursive_text_content_blowing_wasm_stack_at_max_depth() {
        // Catches: Element::text_content recursing once per nesting level on a max-depth tree.
        let xml = format!("{}x{}", "<a>".repeat(4096), "</a>".repeat(4096));
        let root = parse_xml_str(&xml).expect("parses").root;
        assert_eq!(root.text_content(), "x");
    }

    #[test]
    fn catches_drop_of_cloned_4096_deep_tree() {
        // Catches: Drop of a cloned 4096-deep tree still needing the iterative Drop path.
        let root = deep_chain(4096);
        let cloned = root.clone();
        drop(root);
        drop(cloned);
    }
}
