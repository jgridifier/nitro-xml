// SPDX-License-Identifier: MIT
//! Paragraph text insert/replace, split and join on the docx edit host (#78).
//!
//! Script op `setParagraphText` matches `parity/edit/apply.mjs`. Split/join feed
//! the #61 rich surface host path (no ProseMirror). Table cell text is #79.

use std::any::Any;

use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry, EditError};
use serde_json::{json, Value};

use super::host::{make_run, DocxHost};

fn host_mut(host: &mut dyn Any) -> Result<&mut DocxHost, EditError> {
    host.downcast_mut::<DocxHost>()
        .ok_or_else(|| EditError::Other {
            message: "expected DocxHost".into(),
        })
}

fn block_index(args: &Value) -> Result<usize, EditError> {
    args.get("blockIndex")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .ok_or_else(|| EditError::Other {
            message: "missing blockIndex".into(),
        })
}

/// Replace a paragraph's runs with a single styled run (TS `setParagraphText`).
pub fn set_paragraph_text(
    host: &mut DocxHost,
    block_index: usize,
    text: &str,
    attrs: &serde_json::Map<String, Value>,
) -> Result<(), EditError> {
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let run = make_run(text, attrs);
    if let Some(obj) = block.as_object_mut() {
        obj.insert("runs".into(), json!([run]));
    }
    host.mark_touched(block_index);
    Ok(())
}

/// Split a top-level paragraph at a UTF-8 byte offset into two blocks.
pub fn split_paragraph(
    host: &mut DocxHost,
    block_index: usize,
    offset: usize,
) -> Result<(), EditError> {
    let block = host
        .paragraph(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let text: String = block
        .get("runs")
        .and_then(|r| r.as_array())
        .map(|rs| {
            rs.iter()
                .map(|r| r.get("text").and_then(|t| t.as_str()).unwrap_or(""))
                .collect()
        })
        .unwrap_or_default();
    let offset = offset.min(text.len());
    if !text.is_char_boundary(offset) {
        return Err(EditError::Other {
            message: "split offset not on char boundary".into(),
        });
    }
    let (left, right) = text.split_at(offset);
    let id = block
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("p")
        .to_owned();
    let align = block.get("align").cloned();
    let style = block.get("style").cloned();
    let numbering = block.get("numbering").cloned();

    let mut left_para = json!({
        "type": "paragraph",
        "id": id,
        "runs": [{ "text": left }],
    });
    let mut right_para = json!({
        "type": "paragraph",
        "id": format!("{id}-split"),
        "runs": [{ "text": right }],
    });
    for (para, extra) in [(&mut left_para, align.clone()), (&mut right_para, align)] {
        if let (Some(obj), Some(a)) = (para.as_object_mut(), extra) {
            obj.insert("align".into(), a);
        }
    }
    if let (Some(obj), Some(s)) = (left_para.as_object_mut(), style.clone()) {
        obj.insert("style".into(), s);
    }
    if let (Some(obj), Some(s)) = (right_para.as_object_mut(), style) {
        obj.insert("style".into(), s);
    }
    if let (Some(obj), Some(n)) = (left_para.as_object_mut(), numbering.clone()) {
        obj.insert("numbering".into(), n);
    }
    if let (Some(obj), Some(n)) = (right_para.as_object_mut(), numbering) {
        obj.insert("numbering".into(), n);
    }

    host.blocks[block_index] = left_para;
    host.blocks.insert(block_index + 1, right_para);
    host.mark_touched(block_index);
    host.mark_touched(block_index + 1);
    Ok(())
}

/// Join paragraph `block_index + 1` into `block_index` (top-level paragraphs only).
pub fn join_paragraph(host: &mut DocxHost, block_index: usize) -> Result<(), EditError> {
    if block_index + 1 >= host.blocks.len() {
        return Err(EditError::Other {
            message: "no next paragraph to join".into(),
        });
    }
    let left_ty = host.blocks[block_index]
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let right_ty = host.blocks[block_index + 1]
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    if left_ty != "paragraph" || right_ty != "paragraph" {
        return Err(EditError::Other {
            message: "join requires adjacent paragraphs".into(),
        });
    }
    let right = host.blocks.remove(block_index + 1);
    let right_runs = right
        .get("runs")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let left = &mut host.blocks[block_index];
    if let Some(obj) = left.as_object_mut() {
        let runs = obj
            .entry("runs")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| EditError::Other {
                message: "runs not array".into(),
            })?;
        runs.extend(right_runs);
    }
    host.mark_touched(block_index);
    Ok(())
}

pub fn register_text_commands(registry: &mut CommandRegistry) {
    registry.register(
        Command::stub("setParagraphText", "setParagraphText").with_runner(|host, args| {
            let h = host_mut(host)?;
            let bi = block_index(args)?;
            let text =
                args.get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EditError::Other {
                        message: "setParagraphText missing text".into(),
                    })?;
            let attrs = args.as_object().cloned().unwrap_or_default();
            set_paragraph_text(h, bi, text, &attrs)?;
            let mut cs = ChangeSet::new(ChangeKind::Other, format!("setParagraphText {bi}"));
            cs.dirty_parts = vec!["word/document.xml".into()];
            Ok(cs)
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catches_set_paragraph_text_on_table_block_is_rejected() {
        // Catches: setParagraphText silently mutating a table as if it were a paragraph (#78 negative).
        let mut host = DocxHost {
            format: "docx".into(),
            blocks: vec![json!({ "type": "table", "rows": [] })],
            numbering_catalog: None,
            paragraph_styles: None,
            character_styles: None,
            touched: Default::default(),
        };
        let err = set_paragraph_text(&mut host, 0, "x", &Default::default()).unwrap_err();
        assert!(err.to_string().contains("not a paragraph"));
    }
}
