// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/hyperlink.ts; changes listed in PROVENANCE.md.

use indexmap::IndexMap;
use serde_json::{json, Value};

use nitro_xml_opc::opc::{RelMode, Relationship};
use nitro_xml_opc::xml::Element;

use crate::xml_util::{get_r, get_w};

pub fn resolve_hyperlink(element: &Element, rels: &IndexMap<String, Relationship>) -> Value {
    let mut obj = serde_json::Map::new();
    if let Some(rel_id) = get_r(Some(element), "id") {
        if let Some(rel) = rels.get(&rel_id) {
            if rel.mode == RelMode::External {
                obj.insert("href".into(), json!(rel.target));
            }
        }
    }
    if let Some(anchor) = get_w(Some(element), "anchor") {
        obj.insert("anchor".into(), json!(anchor));
    }
    if let Some(tooltip) = get_w(Some(element), "tooltip").filter(|s| !s.is_empty()) {
        obj.insert("tooltip".into(), json!(tooltip));
    }
    Value::Object(obj)
}

/// Parses `HYPERLINK "target"` (optionally `\l anchor`) from `w:fldSimple/@w:instr`.
pub fn parse_simple_hyperlink_field(instr: &str) -> Option<Value> {
    let s = instr.trim_start();
    if !s.to_uppercase().starts_with("HYPERLINK") {
        return None;
    }
    let chars: Vec<char> = s.chars().collect();
    let mut i = "HYPERLINK".len();
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    if i >= chars.len() || chars[i] != '"' {
        return None;
    }
    i += 1;
    let start = i;
    while i < chars.len() && chars[i] != '"' {
        i += 1;
    }
    if i >= chars.len() {
        return None;
    }
    let target: String = chars[start..i].iter().collect();
    i += 1;
    let rest: String = chars[i..].iter().collect();
    let rest = rest.trim_start();
    if rest.len() >= 2 && rest[..2].eq_ignore_ascii_case("\\l") {
        let rest_chars: Vec<char> = rest.chars().collect();
        let mut j = 2;
        while j < rest_chars.len() && rest_chars[j].is_whitespace() {
            j += 1;
        }
        if j < rest_chars.len() && rest_chars[j] == '"' {
            j += 1;
            let a_start = j;
            while j < rest_chars.len() && rest_chars[j] != '"' {
                j += 1;
            }
            if j < rest_chars.len() {
                let anchor: String = rest_chars[a_start..j].iter().collect();
                return Some(json!({ "anchor": anchor }));
            }
        }
    }
    if !target.is_empty() {
        return Some(json!({ "href": target }));
    }
    None
}
