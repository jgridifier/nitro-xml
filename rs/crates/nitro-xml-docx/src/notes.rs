// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/notes.ts; changes listed in PROVENANCE.md.

use serde_json::{json, Value};

use nitro_xml_opc::xml::{parse_xml_str, Element};

use crate::xml_util::{children_w, get_w};

pub fn parse_notes_part(
    xml: &str,
    kind: NoteKind,
    parse_blocks: &mut dyn FnMut(&Element, &str) -> Vec<Value>,
) -> Vec<Value> {
    let document = parse_xml_str(xml).expect("notes xml");
    let tag = if kind == NoteKind::Footnote {
        "footnote"
    } else {
        "endnote"
    };
    let id_prefix = if kind == NoteKind::Footnote {
        "fn"
    } else {
        "en"
    };
    let skipped = ["separator", "continuationSeparator"];
    let mut notes = Vec::new();
    for element in children_w(&document.root, tag) {
        let note_type = get_w(Some(element), "type");
        if note_type.as_deref().is_some_and(|t| skipped.contains(&t)) {
            continue;
        }
        let Some(id) = get_w(Some(element), "id") else {
            continue;
        };
        notes.push(json!({
            "id": id,
            "blocks": parse_blocks(element, &format!("{}{}-", id_prefix, id)),
        }));
    }
    notes
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    Footnote,
    Endnote,
}
