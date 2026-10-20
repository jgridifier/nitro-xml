// SPDX-License-Identifier: MIT
//! List / numbering edits on paragraph blocks (#78).
//!
//! Ports the model-side of upstream `numbering-editing.ts` (ensure definition,
//! apply/clear `numPr`). The catalog matches `parse_numbering_catalog` so the
//! writer can emit `word/numbering.xml` and a reopen reads the same num id.

use nitro_xml_opc::edit::EditError;
use serde_json::json;

use super::host::DocxHost;

/// Ensure a simple bullet or decimal list definition exists; returns `numId`.
///
/// Reuses an existing level-0 definition of the same `numFmt`. New entries use
/// the parser's object catalog (`abstractNums` / `nums` keyed by id string).
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
    let lvl_text = if fmt == "bullet" { "•" } else { "%1." };
    if host.numbering_catalog.is_none() {
        host.numbering_catalog = Some(json!({
            "abstractNums": {},
            "nums": {},
            "warnings": [],
        }));
    }
    let catalog = host
        .numbering_catalog
        .as_mut()
        .ok_or_else(|| EditError::Other {
            message: "numbering catalog missing".into(),
        })?;
    if let Some(id) = find_num_id(catalog, fmt) {
        return Ok(id);
    }
    let abs = catalog
        .get_mut("abstractNums")
        .and_then(|n| n.as_object_mut())
        .ok_or_else(|| EditError::Other {
            message: "numbering catalog abstractNums is not a map".into(),
        })?;
    let next = next_id(abs.keys().map(String::as_str));
    let id = next.to_string();
    abs.insert(
        id.clone(),
        json!({
            "id": id,
            "levels": {
                "0": { "level": 0, "start": 1, "numFmt": fmt, "lvlText": lvl_text }
            }
        }),
    );
    let nums = catalog
        .get_mut("nums")
        .and_then(|n| n.as_object_mut())
        .ok_or_else(|| EditError::Other {
            message: "numbering catalog nums is not a map".into(),
        })?;
    let num_id = next_id(nums.keys().map(String::as_str)).max(next);
    let num_key = num_id.to_string();
    nums.insert(
        num_key.clone(),
        json!({ "id": num_key, "abstractNumId": id }),
    );
    Ok(num_id)
}

fn find_num_id(catalog: &serde_json::Value, fmt: &str) -> Option<u32> {
    let abs = catalog.get("abstractNums")?.as_object()?;
    let nums = catalog.get("nums")?.as_object()?;
    for (id, num) in nums {
        let abs_id = num.get("abstractNumId").and_then(|v| v.as_str())?;
        let levels = abs.get(abs_id)?.get("levels")?.as_object()?;
        let lvl = levels.get("0")?;
        if lvl.get("numFmt").and_then(|v| v.as_str()) == Some(fmt) {
            return id.parse().ok();
        }
    }
    None
}

fn next_id<'a>(keys: impl Iterator<Item = &'a str>) -> u32 {
    keys.filter_map(|k| k.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1
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
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": "item" }],
        })]);
        toggle_bullet_list(&mut host, 0).unwrap();
        assert!(host.blocks[0].get("numbering").is_some());
        toggle_bullet_list(&mut host, 0).unwrap();
        assert!(host.blocks[0].get("numbering").is_none());
    }
}
