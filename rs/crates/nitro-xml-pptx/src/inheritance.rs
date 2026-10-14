// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/core/runtime/merge-xml-objects.ts,
// PptxHandlerRuntimePlaceholderLookup.ts and placeholder-style-family.ts; changes in PROVENANCE.md.

use nitro_xml_opc::xml::{Element, Node};

/// `p:ph` metadata on a shape.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaceholderInfo {
    pub idx: Option<String>,
    pub type_: Option<String>,
    pub sz: Option<String>,
}

/// Which spTree bucket matched (shape / picture / graphic frame).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderBucket {
    Shape,
    Picture,
    GraphicFrame,
}

#[derive(Debug, Clone)]
pub struct PlaceholderNode {
    pub bucket: PlaceholderBucket,
    pub element: Element,
}

const HEADER_FOOTER_TYPES: [&str; 4] = ["hdr", "ftr", "dt", "sldnum"];
const SPECIAL_PLACEHOLDER_TYPES: [&str; 4] = ["dt", "ftr", "hdr", "sldnum"];
const ORPHANED_PLACEHOLDER_INDEX: u64 = 4_294_967_295;

fn normalize_placeholder_index(raw: Option<&str>) -> Option<String> {
    let text = raw.map(str::trim).filter(|s| !s.is_empty())?;
    if !text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let value = text.parse::<u64>().ok()?;
    if value == ORPHANED_PLACEHOLDER_INDEX {
        return None;
    }
    Some(value.to_string())
}

fn types_match(source: &str, target: &str) -> bool {
    source == target
        || (source == "ctrtitle" && target == "title")
        || (source == "subtitle" && target == "body")
}

#[allow(dead_code)]
pub fn placeholder_style_family(type_: Option<&str>) -> String {
    let declared = type_
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("body")
        .to_lowercase();
    match declared.as_str() {
        "title" | "ctrtitle" => "title".to_owned(),
        "body" | "obj" | "subtitle" => "body".to_owned(),
        other => other.to_owned(),
    }
}

pub fn extract_placeholder_info(nv_pr: Option<&Element>) -> Option<PlaceholderInfo> {
    let nv_pr = nv_pr?;
    let ph = child_local(nv_pr, "ph", nitro_xml_opc::xml::ns::P)?;
    Some(PlaceholderInfo {
        idx: normalize_placeholder_index(attr(ph, "idx")),
        type_: attr(ph, "type").map(|t| t.to_lowercase()),
        sz: attr(ph, "sz").map(|s| s.to_lowercase()),
    })
}

pub fn placeholder_matches(
    source: Option<&PlaceholderInfo>,
    target: Option<&PlaceholderInfo>,
) -> bool {
    match (source, target) {
        (None, None) => true,
        (_, None) => false,
        (None, Some(_)) => true,
        (Some(source), Some(target)) => {
            let source_idx = source.idx.as_deref().unwrap_or("0");
            let target_idx = target.idx.as_deref().unwrap_or("0");
            if source_idx != target_idx {
                return false;
            }
            if let (Some(st), Some(tt)) = (&source.type_, &target.type_) {
                if !types_match(st, tt) {
                    return false;
                }
            }
            if let Some(st) = &source.type_ {
                if SPECIAL_PLACEHOLDER_TYPES.contains(&st.as_str()) && target.type_.is_none() {
                    return false;
                }
            }
            if let Some(tt) = &target.type_ {
                if SPECIAL_PLACEHOLDER_TYPES.contains(&tt.as_str()) && source.type_.is_none() {
                    return false;
                }
            }
            let both_explicit = source.idx.is_some() && target.idx.is_some();
            if !both_explicit && source.type_.is_some() && target.type_.is_none() {
                return false;
            }
            true
        }
    }
}

pub fn is_header_footer_placeholder(type_: Option<&str>) -> bool {
    type_
        .map(|t| t.to_lowercase())
        .is_some_and(|t| HEADER_FOOTER_TYPES.contains(&t.as_str()))
}

/// Field type for inherited header/footer literal text (`ftr` → `footer`, `hdr` → `header`).
pub fn inherited_placeholder_field_type(type_: Option<&str>) -> Option<&'static str> {
    match type_.map(|t| t.trim().to_ascii_lowercase()) {
        Some(t) if t == "ftr" => Some("footer"),
        Some(t) if t == "hdr" => Some("header"),
        _ => None,
    }
}

/// Deep-merge `override` onto `base` for placeholder inheritance (layout over master).
pub fn merge_elements(
    base: Option<&Element>,
    override_el: Option<&Element>,
    depth: usize,
) -> Option<Element> {
    const MAX_DEPTH: usize = 64;
    match (base, override_el) {
        (None, None) => None,
        (None, Some(o)) => Some(o.clone()),
        (Some(b), None) => Some(b.clone()),
        (Some(b), Some(o)) => {
            if depth > MAX_DEPTH {
                return Some(o.clone());
            }
            let mut merged = b.clone();
            // Merge attributes from override
            for oa in &o.attrs {
                if let Some(existing) = merged
                    .attrs
                    .iter_mut()
                    .find(|a| a.local == oa.local && a.ns == oa.ns)
                {
                    existing.value = oa.value.clone();
                } else {
                    merged.attrs.push(oa.clone());
                }
            }
            // Merge children by local+ns key; recurse when both sides have element child
            for o_child in &o.children {
                if let Node::Element(o_el) = o_child {
                    if let Some(b_idx) = merged.children.iter().position(|c| {
                        matches!(c, Node::Element(e) if e.local == o_el.local && e.ns == o_el.ns)
                    }) {
                        if let Node::Element(b_el) = &merged.children[b_idx] {
                            let sub = merge_elements(Some(b_el), Some(o_el), depth + 1);
                            if let Some(sub) = sub {
                                merged.children[b_idx] = Node::Element(sub);
                            }
                        }
                    } else if !is_empty_placeholder_element(o_el) {
                        merged.children.push(o_child.clone());
                    }
                } else if let Node::Text(t) = o_child {
                    if !t.is_empty() {
                        merged.children.push(o_child.clone());
                    }
                }
            }
            Some(merged)
        }
    }
}

fn is_empty_placeholder_element(el: &Element) -> bool {
    el.children.is_empty() && el.attrs.is_empty()
}

pub fn find_placeholder_in_sp_tree(
    sp_tree: &Element,
    expected: &PlaceholderInfo,
) -> Option<PlaceholderNode> {
    for (bucket, local, nv_local) in [
        (PlaceholderBucket::Shape, "sp", "nvSpPr"),
        (PlaceholderBucket::Picture, "pic", "nvPicPr"),
        (
            PlaceholderBucket::GraphicFrame,
            "graphicFrame",
            "nvGraphicFramePr",
        ),
    ] {
        for sp in children_local(sp_tree, local, nitro_xml_opc::xml::ns::P) {
            let nv = child_path(
                sp,
                &[
                    (nv_local, nitro_xml_opc::xml::ns::P),
                    ("nvPr", nitro_xml_opc::xml::ns::P),
                ],
            );
            let info = extract_placeholder_info(nv);
            if placeholder_matches(Some(expected), info.as_ref()) {
                return Some(PlaceholderNode {
                    bucket,
                    element: sp.clone(),
                });
            }
        }
    }
    None
}

pub fn find_singleton_placeholder_in_sp_tree(
    sp_tree: &Element,
    expected: &PlaceholderInfo,
) -> Option<PlaceholderNode> {
    let t = expected.type_.as_deref();
    if !is_header_footer_placeholder(t) {
        return None;
    }
    for (bucket, local, nv_local) in [
        (PlaceholderBucket::Shape, "sp", "nvSpPr"),
        (PlaceholderBucket::Picture, "pic", "nvPicPr"),
        (
            PlaceholderBucket::GraphicFrame,
            "graphicFrame",
            "nvGraphicFramePr",
        ),
    ] {
        for sp in children_local(sp_tree, local, nitro_xml_opc::xml::ns::P) {
            let nv = child_path(
                sp,
                &[
                    (nv_local, nitro_xml_opc::xml::ns::P),
                    ("nvPr", nitro_xml_opc::xml::ns::P),
                ],
            );
            let info = extract_placeholder_info(nv);
            if info.as_ref().and_then(|i| i.type_.as_deref()) == t {
                return Some(PlaceholderNode {
                    bucket,
                    element: sp.clone(),
                });
            }
        }
    }
    None
}

pub fn merge_placeholder_context(
    layout: Option<PlaceholderNode>,
    master: Option<PlaceholderNode>,
) -> Option<PlaceholderNode> {
    match (layout, master) {
        (None, m) => m,
        (l, None) => l,
        (Some(layout), Some(master)) if layout.bucket == master.bucket => {
            let merged_el = merge_elements(Some(&master.element), Some(&layout.element), 0)?;
            Some(PlaceholderNode {
                bucket: layout.bucket,
                element: merged_el,
            })
        }
        (Some(layout), Some(master)) => {
            // Different buckets: layout wins as node, merge element trees anyway
            let merged_el = merge_elements(Some(&master.element), Some(&layout.element), 0)?;
            Some(PlaceholderNode {
                bucket: layout.bucket,
                element: merged_el,
            })
        }
    }
}

pub fn sp_pr_of_placeholder(node: &PlaceholderNode) -> Option<&Element> {
    child_local(&node.element, "spPr", nitro_xml_opc::xml::ns::P)
}

pub fn tx_body_of_placeholder(node: &PlaceholderNode) -> Option<&Element> {
    child_local(&node.element, "txBody", nitro_xml_opc::xml::ns::P)
}

pub fn child_local<'a>(parent: &'a Element, local: &str, ns: &str) -> Option<&'a Element> {
    parent.children.iter().find_map(|n| match n {
        Node::Element(e) if e.local == local && e.ns.as_deref() == Some(ns) => Some(e),
        _ => None,
    })
}

pub fn children_local<'a>(parent: &'a Element, local: &str, ns: &str) -> Vec<&'a Element> {
    parent
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(e) if e.local == local && e.ns.as_deref() == Some(ns) => Some(e),
            _ => None,
        })
        .collect()
}

pub fn child_path<'a>(parent: &'a Element, path: &[(&str, &str)]) -> Option<&'a Element> {
    let mut cur = parent;
    for (local, ns) in path {
        cur = child_local(cur, local, ns)?;
    }
    Some(cur)
}

pub fn attr<'a>(el: &'a Element, name: &str) -> Option<&'a str> {
    el.attrs
        .iter()
        .find(|a| a.local == name && a.ns.is_none())
        .map(|a| a.value.as_str())
}

#[allow(dead_code)] // used by slide parsers when DrawingML transform is absent
pub fn parse_bool_attr(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim).map(|s| s.to_lowercase()),
        Some(v) if v == "1" || v == "true"
    )
}

/// OOXML boolean attribute (`1`/`true`/`0`/`false`); absent when unset.
pub fn parse_optional_bool_attr(value: Option<&str>) -> Option<bool> {
    let trimmed = value.map(str::trim).filter(|s| !s.is_empty());
    trimmed.map(|s| parse_bool_attr(Some(s)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nitro_xml_opc::xml::{ns, parse_xml_str};

    #[test]
    fn empty_layout_sp_pr_does_not_drop_master_xfrm() {
        // Catches: self-closing layout <p:spPr/> clobbering master a:xfrm so the
        // slide placeholder resolves at 0×0 and disappears from the model.
        let master = parse_xml_str(
            r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <p:spPr><a:xfrm><a:off x="1" y="2"/><a:ext cx="3" cy="4"/></a:xfrm></p:spPr>
            </p:sp>"#,
        )
        .unwrap();
        let layout = parse_xml_str(
            r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <p:spPr/>
            </p:sp>"#,
        )
        .unwrap();
        let merged = merge_elements(Some(&master.root), Some(&layout.root), 0).unwrap();
        let sp_pr = child_local(&merged, "spPr", ns::P).unwrap();
        assert!(
            child_local(sp_pr, "xfrm", ns::A).is_some(),
            "master xfrm must survive an empty layout spPr"
        );
    }
}
