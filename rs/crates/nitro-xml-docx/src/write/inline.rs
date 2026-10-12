// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/write-inline.ts (collectInlineSlots subset); changes listed in PROVENANCE.md.

use nitro_xml_opc::xml::{ns, Element, Node};

use crate::xml_util::ElementExt;

pub struct InlineSlot {
    pub element: Option<Element>,
}

fn run_is_safe(run: &Element) -> bool {
    let content: Vec<&Element> = run
        .element_children()
        .into_iter()
        .filter(|e| e.local != "rPr")
        .collect();
    if content.len() == 1 {
        let only = content[0];
        if only.local == "t" || only.local == "tab" || only.local == "br" {
            return true;
        }
    }
    content.is_empty()
}

/// Collect modeled `w:r` slots in document order; returns `None` when inline content is unsafe.
pub fn collect_inline_slots(paragraph: &Element) -> Option<Vec<InlineSlot>> {
    let mut slots = Vec::new();
    for child in &paragraph.children {
        match child {
            Node::Element(e) if e.ns.as_deref() == Some(ns::W) => {
                if e.local == "pPr" || e.local == "bookmarkStart" || e.local == "bookmarkEnd" {
                    continue;
                }
                if e.local == "r" {
                    if !run_is_safe(e) {
                        return None;
                    }
                    slots.push(InlineSlot {
                        element: Some(e.clone()),
                    });
                } else {
                    return None;
                }
            }
            Node::Text(t) if !t.trim().is_empty() => return None,
            _ => {}
        }
    }
    Some(slots)
}

pub fn replaceable_inline_indices(paragraph: &Element) -> Vec<usize> {
    paragraph
        .children
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            if let Node::Element(e) = n {
                if e.ns.as_deref() == Some(ns::W) && e.local == "r" {
                    Some(i)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}
