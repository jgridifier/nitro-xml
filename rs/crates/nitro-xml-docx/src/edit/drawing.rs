// SPDX-License-Identifier: MIT
//! Inline picture / drawing edits on paragraph runs (#79).
//!
//! No `edits/*.json` ops yet — host API for the rich surface. Package media
//! write-back waits on the docx writer (`TODO(#65)` / save path).

use nitro_xml_opc::edit::EditError;
use serde_json::{json, Value};

use super::host::DocxHost;

/// Attach or replace an inline image descriptor on a run.
///
/// `image` is the JSON object shape produced by the parser (`run.image`).
pub fn set_run_image(
    host: &mut DocxHost,
    block_index: usize,
    run_index: usize,
    image: Value,
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
        obj.insert("image".into(), image);
    }
    host.mark_touched(block_index);
    Ok(())
}

/// Remove an inline image from a run (keeps text).
pub fn clear_run_image(
    host: &mut DocxHost,
    block_index: usize,
    run_index: usize,
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
        message: format!("run {run_index} missing"),
    })?;
    if let Some(obj) = run.as_object_mut() {
        obj.remove("image");
    }
    host.mark_touched(block_index);
    Ok(())
}

/// Placeholder image JSON used by unit tests (not a real media part).
pub fn stub_image(part_name: &str, width_emu: u64, height_emu: u64) -> Value {
    json!({
        "partName": part_name,
        "widthEmu": width_emu,
        "heightEmu": height_emu,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catches_set_run_image_attaches_descriptor_without_dropping_text() {
        // Catches: picture edit replacing the run instead of attaching `image` (#79).
        let mut host = DocxHost {
            format: "docx".into(),
            blocks: vec![json!({
                "type": "paragraph",
                "id": "p0",
                "runs": [{ "text": "cap" }],
            })],
            numbering_catalog: None,
            paragraph_styles: None,
            character_styles: None,
            sections: None,
            page: None,
            even_and_odd_headers: false,
            touched: Default::default(),
        };
        set_run_image(
            &mut host,
            0,
            0,
            stub_image("word/media/image1.png", 100, 200),
        )
        .unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["text"], "cap");
        assert_eq!(
            host.blocks[0]["runs"][0]["image"]["partName"],
            "word/media/image1.png"
        );
    }
}
