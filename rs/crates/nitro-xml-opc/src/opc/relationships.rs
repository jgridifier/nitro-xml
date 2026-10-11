// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/opc/relationships.ts; changes listed in PROVENANCE.md.
//! Parses `.rels` parts and resolves relationship targets.

use indexmap::IndexMap;

use crate::xml::{parse_xml_str, Element, Node, XmlError};

/// One relationship entry from a `.rels` part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub id: String,
    pub type_: String,
    pub target: String,
    /// `External` for `TargetMode="External"`, else `Internal`.
    pub mode: RelMode,
}

/// Target mode on a relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelMode {
    Internal,
    External,
}

impl RelMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "Internal",
            Self::External => "External",
        }
    }
}

fn attr<'a>(element: &'a Element, name: &str) -> Option<&'a str> {
    element
        .attrs
        .iter()
        .find(|a| a.local == name && a.ns.is_none())
        .map(|a| a.value.as_str())
}

fn child_elements(parent: &Element) -> impl Iterator<Item = &Element> {
    parent.children.iter().filter_map(|n| match n {
        Node::Element(e) => Some(e),
        Node::Text(_) => None,
    })
}

/// Parses a `.rels` part into a map keyed by relationship id (insertion order, like upstream `Map`).
pub fn parse_relationships(xml: Option<&str>) -> Result<IndexMap<String, Relationship>, XmlError> {
    let mut map = IndexMap::new();
    let Some(xml) = xml else {
        return Ok(map);
    };
    let doc = parse_xml_str(xml)?;
    for node in child_elements(&doc.root) {
        if node.local != "Relationship" {
            continue;
        }
        let Some(id) = attr(node, "Id") else {
            continue;
        };
        if id.is_empty() {
            continue;
        }
        let Some(target) = attr(node, "Target") else {
            continue;
        };
        let mode = if attr(node, "TargetMode") == Some("External") {
            RelMode::External
        } else {
            RelMode::Internal
        };
        map.insert(
            id.to_owned(),
            Relationship {
                id: id.to_owned(),
                type_: attr(node, "Type").unwrap_or("").to_owned(),
                target: target.to_owned(),
                mode,
            },
        );
    }
    Ok(map)
}

/// Resolves a relationship `Target` against the part that declared it. Absolute targets (leading
/// `/`) are package-rooted; relative ones resolve against the declaring part's folder, handling
/// `.` and `..` segments. Returns a package part name without a leading slash.
pub fn resolve_part_path(base_part: &str, target: &str) -> String {
    if let Some(rest) = target.strip_prefix('/') {
        return rest.to_owned();
    }
    let slash = base_part.rfind('/').map(|i| i + 1).unwrap_or(0);
    let base_dir = &base_part[..slash];
    let joined = format!("{base_dir}{target}");
    let mut resolved: Vec<&str> = Vec::new();
    for segment in joined.split('/') {
        if segment == "." || segment.is_empty() {
            continue;
        }
        if segment == ".." {
            resolved.pop();
        } else {
            resolved.push(segment);
        }
    }
    resolved.join("/")
}

/// The `.rels` part name that holds the relationships of `part_name`
/// (`word/document.xml` → `word/_rels/document.xml.rels`).
pub fn relationships_part_for(part_name: &str) -> String {
    let slash = part_name.rfind('/').map(|i| i + 1).unwrap_or(0);
    format!("{}_rels/{}.rels", &part_name[..slash], &part_name[slash..])
}
