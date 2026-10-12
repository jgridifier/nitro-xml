// SPDX-License-Identifier: MIT
//! Character and paragraph formatting + style id on the docx edit host (#78).
//!
//! Script ops `setRunFormatting` / `setParagraphAlign` match `parity/edit/apply.mjs`.

use std::any::Any;

use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry, EditError};
use serde_json::{json, Value};

use super::host::DocxHost;

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

/// Patch run properties on a paragraph run (TS `setRunFormatting`).
pub fn set_run_formatting(
    host: &mut DocxHost,
    block_index: usize,
    run_index: usize,
    patch: &serde_json::Map<String, Value>,
) -> Result<(), EditError> {
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let runs = block
        .get_mut("runs")
        .and_then(|r| r.as_array_mut())
        .ok_or_else(|| EditError::Other {
            message: format!("runs missing on block {block_index}"),
        })?;
    let run = runs.get_mut(run_index).ok_or_else(|| EditError::Other {
        message: format!("run {run_index} missing on block {block_index}"),
    })?;
    if let Some(obj) = run.as_object_mut() {
        for key in ["bold", "italic", "fontSize", "color", "underline"] {
            if let Some(v) = patch.get(key) {
                obj.insert(key.into(), v.clone());
            }
        }
    }
    host.mark_touched(block_index);
    Ok(())
}

/// Set paragraph alignment (TS `setParagraphAlign`).
pub fn set_paragraph_align(
    host: &mut DocxHost,
    block_index: usize,
    align: &str,
) -> Result<(), EditError> {
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    if let Some(obj) = block.as_object_mut() {
        obj.insert("align".into(), json!(align));
    }
    host.mark_touched(block_index);
    Ok(())
}

/// Apply a paragraph style id (`w:pStyle`).
pub fn set_paragraph_style(
    host: &mut DocxHost,
    block_index: usize,
    style_id: &str,
) -> Result<(), EditError> {
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    if let Some(obj) = block.as_object_mut() {
        if style_id.is_empty() {
            obj.remove("style");
        } else {
            obj.insert("style".into(), json!(style_id));
        }
    }
    host.mark_touched(block_index);
    Ok(())
}

pub fn register_format_commands(registry: &mut CommandRegistry) {
    registry.register(
        Command::stub("setRunFormatting", "setRunFormatting").with_runner(|host, args| {
            let h = host_mut(host)?;
            let bi = block_index(args)?;
            let ri = args.get("runIndex").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let patch = args.as_object().cloned().unwrap_or_default();
            set_run_formatting(h, bi, ri, &patch)?;
            let mut cs = ChangeSet::new(ChangeKind::Format, format!("setRunFormatting {bi}"));
            cs.dirty_parts = vec!["word/document.xml".into()];
            Ok(cs)
        }),
    );
    registry.register(
        Command::stub("setParagraphAlign", "setParagraphAlign").with_runner(|host, args| {
            let h = host_mut(host)?;
            let bi = block_index(args)?;
            let align =
                args.get("align")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EditError::Other {
                        message: "setParagraphAlign missing align".into(),
                    })?;
            set_paragraph_align(h, bi, align)?;
            let mut cs = ChangeSet::new(ChangeKind::Format, format!("setParagraphAlign {bi}"));
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
    fn catches_run_formatting_preserves_text_while_toggling_bold() {
        // Catches: setRunFormatting replacing the run text instead of patching props (#78).
        let mut host = DocxHost {
            format: "docx".into(),
            blocks: vec![json!({
                "type": "paragraph",
                "id": "p0",
                "runs": [{ "text": "Hi", "bold": false }],
            })],
            numbering_catalog: None,
            paragraph_styles: None,
            character_styles: None,
            touched: Default::default(),
        };
        let mut patch = serde_json::Map::new();
        patch.insert("bold".into(), json!(true));
        set_run_formatting(&mut host, 0, 0, &patch).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["text"], "Hi");
        assert_eq!(host.blocks[0]["runs"][0]["bold"], true);
    }
}
