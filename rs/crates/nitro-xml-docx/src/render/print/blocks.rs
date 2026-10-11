// SPDX-License-Identifier: MIT
//! Column block JSON → [`LayoutBlockBox`].
//!
//! Nested table-cell paragraphs keep a serialized `kind` field for the layout-IR
//! dump (#22). Internally tagged [`LayoutBlockBox`] cannot deserialize that map
//! directly (tag and field share the name `kind`), so we branch on `kind` and
//! decode the concrete structs instead.

use crate::layout::{LayoutBlockBox, LayoutParagraphBox, LayoutTableBox};

pub fn parse_block(value: &serde_json::Value) -> Option<LayoutBlockBox> {
    match value.get("kind").and_then(|k| k.as_str())? {
        "paragraph" => serde_json::from_value::<LayoutParagraphBox>(value.clone())
            .ok()
            .map(LayoutBlockBox::Paragraph),
        "table" => serde_json::from_value::<LayoutTableBox>(value.clone())
            .ok()
            .map(LayoutBlockBox::Table),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_block_accepts_kind_on_paragraph_map() {
        // Catches: tagged-enum from_value rejecting column.blocks that also carry kind
        // (nested cell paragraphs need kind in the layout-IR dump).
        let v = json!({
            "kind": "paragraph",
            "blockId": "p0",
            "yPx": 0.0,
            "heightPx": 12.0,
            "lines": [{
                "yPx": 0.0,
                "heightPx": 12.0,
                "fragments": [{
                    "text": "Hi",
                    "xPx": 0.0,
                    "widthPx": 10.0,
                    "runIndex": 0
                }],
                "sourceStart": 0,
                "sourceEnd": 2
            }]
        });
        let block = parse_block(&v).expect("paragraph");
        match block {
            LayoutBlockBox::Paragraph(p) => {
                assert_eq!(p.block_id, "p0");
                assert_eq!(p.kind, "paragraph");
                assert_eq!(p.lines[0].fragments[0].text, "Hi");
            }
            LayoutBlockBox::Table(_) => panic!("expected paragraph"),
        }
    }
}
