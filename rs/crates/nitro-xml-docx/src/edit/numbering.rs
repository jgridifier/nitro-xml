// SPDX-License-Identifier: MIT
//! List / numbering edits on paragraph blocks (#78).
//!
//! Ports the model-side of upstream `numbering-editing.ts` (ensure definition,
//! apply/clear `numPr`). Package rewrite of `word/numbering.xml` waits on the
//! docx writer; catalog mutations stay in-memory for undo parity.

use nitro_xml_opc::edit::EditError;
use serde_json::json;

use super::host::DocxHost;

/// Ensure a simple bullet or decimal list definition exists; returns `numId`.
pub fn ensure_list_definition(host: &mut DocxHost, kind: &str) -> Result<u32, EditError> {
    let fmt = match kind {
        "bullet" => "bullet",
        "decimal" | "number" => "decimal",
        other => {
            return Err(EditError::Other {
                message: format!("unsupported list kind {other}"),
            })
        }
    };
    let catalog = host
        .numbering_catalog
        .get_or_insert_with(|| json!({ "abstractNums": [], "nums": [] }));
    let abs_len = catalog
        .get("abstractNums")
        .and_then(|n| n.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let nums_len = catalog
        .get("nums")
        .and_then(|n| n.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let abs_id = (abs_len as u32) + 1;
    let num_id = (nums_len as u32) + 1;
    let lvl_text = if fmt == "bullet" { "•" } else { "%1." };
    catalog
        .get_mut("abstractNums")
        .and_then(|n| n.as_array_mut())
        .ok_or_else(|| EditError::Other {
            message: "numbering catalog missing abstractNums".into(),
        })?
        .push(json!({
            "abstractNumId": abs_id,
            "levels": [{ "ilvl": 0, "numFmt": fmt, "lvlText": lvl_text }],
        }));
    catalog
        .get_mut("nums")
        .and_then(|n| n.as_array_mut())
        .ok_or_else(|| EditError::Other {
            message: "numbering catalog missing nums".into(),
        })?
        .push(json!({
            "numId": num_id,
            "abstractNumId": abs_id,
        }));
    Ok(num_id)
}

/// Attach `numbering: { numId, level }` to a paragraph.
pub fn apply_numbering(
    host: &mut DocxHost,
    block_index: usize,
    num_id: u32,
    level: u32,
) -> Result<(), EditError> {
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    if let Some(obj) = block.as_object_mut() {
        obj.insert(
            "numbering".into(),
            json!({ "numId": num_id, "level": level }),
        );
    }
    host.mark_touched(block_index);
    Ok(())
}

/// Clear list numbering from a paragraph.
pub fn clear_numbering(host: &mut DocxHost, block_index: usize) -> Result<(), EditError> {
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    if let Some(obj) = block.as_object_mut() {
        obj.remove("numbering");
    }
    host.mark_touched(block_index);
    Ok(())
}

/// Toggle a bullet list on a paragraph (create definition if needed).
pub fn toggle_bullet_list(host: &mut DocxHost, block_index: usize) -> Result<(), EditError> {
    let has = host
        .paragraph(block_index)
        .ok()
        .and_then(|p| p.get("numbering"))
        .is_some();
    if has {
        clear_numbering(host, block_index)
    } else {
        let id = ensure_list_definition(host, "bullet")?;
        apply_numbering(host, block_index, id, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catches_toggle_bullet_adds_and_clears_numbering() {
        // Catches: list toggle leaving stale numbering after a second toggle (#78).
        let mut host = DocxHost {
            format: "docx".into(),
            blocks: vec![json!({
                "type": "paragraph",
                "id": "p0",
                "runs": [{ "text": "item" }],
            })],
            numbering_catalog: None,
            paragraph_styles: None,
            character_styles: None,
            sections: None,
            page: None,
            even_and_odd_headers: false,
            touched: Default::default(),
        };
        toggle_bullet_list(&mut host, 0).unwrap();
        assert!(host.blocks[0].get("numbering").is_some());
        toggle_bullet_list(&mut host, 0).unwrap();
        assert!(host.blocks[0].get("numbering").is_none());
    }
}
