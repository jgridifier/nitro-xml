// SPDX-License-Identifier: MIT
//! SmartArt presentation-point style metadata (`dgm:pt` / `dgm:prSet`) for label colours.
//!
//! Process layouts with three `node1` columns paint outer labels with `dk1` and the
//! centre with `lt1`, matching upstream `projectDrawingShapes` / TS oracle output.

use std::collections::BTreeMap;

use nitro_xml_opc::xml::{parse_xml_str, Element, Node};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresStyleMeta {
    pub style_label: String,
    pub style_index: u32,
    pub style_count: u32,
}

/// `drawing shape modelId` → presentation style metadata from `ppt/diagrams/data*.xml`.
pub fn pres_styles_by_model_id(data_xml: &str) -> BTreeMap<String, PresStyleMeta> {
    let mut out = BTreeMap::new();
    let Ok(doc) = parse_xml_str(data_xml) else {
        return out;
    };
    walk_pts(&doc.root, &mut out);
    out
}

fn walk_pts(el: &Element, out: &mut BTreeMap<String, PresStyleMeta>) {
    let mut stack: Vec<&Element> = vec![el];
    while let Some(node) = stack.pop() {
        if node.local == "pt" && attr_str(node, "type").as_deref() == Some("pres") {
            if let Some(model_id) = attr_str(node, "modelId") {
                if let Some(pr_set) = child_any(node, "prSet") {
                    let style_label = attr_str(pr_set, "presStyleLbl").unwrap_or_default();
                    let style_index = attr_u32(pr_set, "presStyleIdx");
                    let style_count = attr_u32(pr_set, "presStyleCnt");
                    if !style_label.is_empty() {
                        out.insert(
                            model_id,
                            PresStyleMeta {
                                style_label,
                                style_index,
                                style_count,
                            },
                        );
                    }
                }
            }
        }
        for child in node.children.iter().rev() {
            if let Node::Element(child) = child {
                stack.push(child);
            }
        }
    }
}

fn child_any<'a>(el: &'a Element, local: &str) -> Option<&'a Element> {
    el.children.iter().find_map(|c| match c {
        Node::Element(e) if e.local == local => Some(e),
        _ => None,
    })
}

fn attr_str(el: &Element, name: &str) -> Option<String> {
    el.attrs
        .iter()
        .find(|a| a.local == name)
        .map(|a| a.value.clone())
}

fn attr_u32(el: &Element, name: &str) -> u32 {
    attr_str(el, name)
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0)
}
