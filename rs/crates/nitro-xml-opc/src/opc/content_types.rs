// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/opc/content-types.ts; changes listed in PROVENANCE.md.
//! Parses `[Content_Types].xml`.

use std::collections::BTreeMap;

use crate::xml::{parse_xml_str, Element, Node};

/// Parsed `[Content_Types].xml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentTypes {
    /// Extension (lower case, no dot) → content type.
    pub defaults: BTreeMap<String, String>,
    /// Part name (leading slash) → content type.
    pub overrides: BTreeMap<String, String>,
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

/// Parses `[Content_Types].xml`; an absent part gives empty maps.
pub fn parse_content_types(xml: Option<&str>) -> Result<ContentTypes, crate::xml::XmlError> {
    let mut types = ContentTypes::default();
    let Some(xml) = xml else {
        return Ok(types);
    };
    let doc = parse_xml_str(xml)?;
    for node in child_elements(&doc.root) {
        let Some(content_type) = attr(node, "ContentType") else {
            continue;
        };
        if node.local == "Default" {
            if let Some(extension) = attr(node, "Extension") {
                types
                    .defaults
                    .insert(extension.to_ascii_lowercase(), content_type.to_owned());
            }
        } else if node.local == "Override" {
            if let Some(part_name) = attr(node, "PartName") {
                types
                    .overrides
                    .insert(part_name.to_owned(), content_type.to_owned());
            }
        }
    }
    Ok(types)
}

/// The content type of a part: its override if declared, else the default for its extension.
pub fn content_type_for_part<'a>(types: &'a ContentTypes, part_name: &str) -> Option<&'a str> {
    let override_path = if part_name.starts_with('/') {
        part_name.to_owned()
    } else {
        format!("/{part_name}")
    };
    if let Some(ct) = types.overrides.get(&override_path) {
        return Some(ct.as_str());
    }
    let extension = part_name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())?;
    types.defaults.get(&extension).map(String::as_str)
}
