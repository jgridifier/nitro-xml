// SPDX-License-Identifier: MIT
//! Inline picture / drawing edits on paragraph runs (#79).
//!
//! `append_inline_picture` stores PNG bytes on a new run (`embedToken` + `png`).
//! Save writes `w:drawing`, `word/media`, and the document image relationship.

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

/// 1×1 RGB PNG inserted by the Picture command.
pub const INSERTED_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
    0x00, 0x03, 0x01, 0x01, 0x00, 0xC9, 0xFE, 0x92, 0xEF, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

/// Append an inline picture run on `prefer` when that block is a paragraph,
/// otherwise on the first paragraph.
pub fn append_inline_picture(host: &mut DocxHost, prefer: usize) -> Result<(), EditError> {
    let index = if host.paragraph(prefer).is_ok() {
        prefer
    } else {
        host.blocks
            .iter()
            .position(|b| b.get("type").and_then(|t| t.as_str()) == Some("paragraph"))
            .ok_or_else(|| EditError::Other {
                message: "no paragraph for insert.picture".into(),
            })?
    };
    let n = next_picture_index(host);
    let png: Vec<Value> = INSERTED_PNG.iter().copied().map(|b| json!(b)).collect();
    let block = host
        .paragraph_mut(index)
        .map_err(|e| EditError::Other { message: e })?;
    let runs = block
        .as_object_mut()
        .ok_or_else(|| EditError::Other {
            message: format!("paragraph {index} is not an object"),
        })?
        .entry("runs")
        .or_insert_with(|| json!([]));
    let runs = runs.as_array_mut().ok_or_else(|| EditError::Other {
        message: format!("runs on paragraph {index} are not an array"),
    })?;
    runs.push(json!({
        "text": "",
        "image": {
            "partName": format!("word/media/nitro-edit-{n}.png"),
            "contentType": "image/png",
            "widthPx": 64,
            "heightPx": 64,
            "embedToken": format!("nitroPic{n}"),
            "png": png,
        }
    }));
    host.mark_touched(index);
    Ok(())
}

fn next_picture_index(host: &DocxHost) -> u32 {
    let mut max = 0u32;
    for block in &host.blocks {
        let Some(runs) = block.get("runs").and_then(|r| r.as_array()) else {
            continue;
        };
        for run in runs {
            let Some(token) = run
                .get("image")
                .and_then(|image| image.get("embedToken"))
                .and_then(|v| v.as_str())
            else {
                continue;
            };
            if let Some(n) = token
                .strip_prefix("nitroPic")
                .and_then(|n| n.parse::<u32>().ok())
            {
                max = max.max(n);
            }
        }
    }
    max + 1
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
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": "cap" }],
        })]);
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
