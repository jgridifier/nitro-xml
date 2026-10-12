// SPDX-License-Identifier: MIT
//! Header and footer story editing (#79).
//!
//! Mutates `sections[i].headers|footers.{default|even|first}.blocks` the same
//! way upstream header/footer editors do. No corpus ops yet.

use nitro_xml_opc::edit::EditError;
use serde_json::{json, Value};

use super::host::DocxHost;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderFooterKind {
    Header,
    Footer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderFooterSlot {
    Default,
    Even,
    First,
}

impl HeaderFooterSlot {
    fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Even => "even",
            Self::First => "first",
        }
    }
}

fn slot_map_key(kind: HeaderFooterKind) -> &'static str {
    match kind {
        HeaderFooterKind::Header => "headers",
        HeaderFooterKind::Footer => "footers",
    }
}

fn ensure_slot(
    host: &mut DocxHost,
    section_index: usize,
    kind: HeaderFooterKind,
    slot: HeaderFooterSlot,
) -> Result<&mut Value, EditError> {
    let sections = host.sections.get_or_insert_with(|| json!([]));
    let arr = sections.as_array_mut().ok_or_else(|| EditError::Other {
        message: "sections not array".into(),
    })?;
    while arr.len() <= section_index {
        arr.push(json!({
            "page": {},
            "headers": {},
            "footers": {},
        }));
    }
    let section = &mut arr[section_index];
    let map_key = slot_map_key(kind);
    let map = section
        .as_object_mut()
        .ok_or_else(|| EditError::Other {
            message: "section not object".into(),
        })?
        .entry(map_key)
        .or_insert_with(|| json!({}));
    let slot_val = map
        .as_object_mut()
        .ok_or_else(|| EditError::Other {
            message: format!("{map_key} not object"),
        })?
        .entry(slot.as_str())
        .or_insert_with(|| {
            json!({
                "partName": format!("word/{}_{}.xml", map_key.trim_end_matches('s'), slot.as_str()),
                "blocks": [],
            })
        });
    Ok(slot_val)
}

/// Replace header/footer story blocks (paragraph/table JSON like the body).
pub fn set_header_footer_blocks(
    host: &mut DocxHost,
    section_index: usize,
    kind: HeaderFooterKind,
    slot: HeaderFooterSlot,
    blocks: Vec<Value>,
) -> Result<(), EditError> {
    let slot_val = ensure_slot(host, section_index, kind, slot)?;
    if let Some(obj) = slot_val.as_object_mut() {
        obj.insert("blocks".into(), Value::Array(blocks));
    }
    if matches!(slot, HeaderFooterSlot::Even) {
        host.even_and_odd_headers = true;
    }
    Ok(())
}

/// Set plain text into the first paragraph of a header/footer slot (creates one if empty).
pub fn set_header_footer_text(
    host: &mut DocxHost,
    section_index: usize,
    kind: HeaderFooterKind,
    slot: HeaderFooterSlot,
    text: &str,
) -> Result<(), EditError> {
    set_header_footer_blocks(
        host,
        section_index,
        kind,
        slot,
        vec![json!({
            "type": "paragraph",
            "id": "hf0",
            "runs": [{ "text": text }],
        })],
    )
}

/// Read header/footer blocks (empty vec if missing).
pub fn header_footer_blocks(
    host: &DocxHost,
    section_index: usize,
    kind: HeaderFooterKind,
    slot: HeaderFooterSlot,
) -> Vec<Value> {
    let Some(sections) = host.sections.as_ref().and_then(|s| s.as_array()) else {
        return Vec::new();
    };
    let Some(section) = sections.get(section_index) else {
        return Vec::new();
    };
    section
        .get(slot_map_key(kind))
        .and_then(|m| m.get(slot.as_str()))
        .and_then(|s| s.get("blocks"))
        .and_then(|b| b.as_array())
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_header_text_lands_in_default_slot_blocks() {
        // Catches: header edit writing body blocks instead of section header slots (#79).
        let mut host = DocxHost::stub(vec![serde_json::json!({
            "type": "paragraph",
            "id": "body",
            "runs": [{ "text": "body" }],
        })]);
        set_header_footer_text(
            &mut host,
            0,
            HeaderFooterKind::Header,
            HeaderFooterSlot::Default,
            "Hdr",
        )
        .unwrap();
        let blocks = header_footer_blocks(
            &host,
            0,
            HeaderFooterKind::Header,
            HeaderFooterSlot::Default,
        );
        assert_eq!(blocks[0]["runs"][0]["text"], "Hdr");
        assert_eq!(host.blocks[0]["runs"][0]["text"], "body");
    }
}
