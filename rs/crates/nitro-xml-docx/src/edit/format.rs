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

/// Half-inch indent step (`w:ind/@w:left`), matching Word's Increase/Decrease Indent.
pub const INDENT_STEP_TWIPS: i64 = 720;

/// Word's Add Space Before/After Paragraph (`w:spacing` before/after), 12 points.
pub const PARAGRAPH_SPACE_TWIPS: i64 = 240;

/// `w:spacing/@w:line` with `w:lineRule="auto"`: single, 1.5, double.
const LINE_SINGLE: i64 = 240;
const LINE_ONE_HALF: i64 = 360;
const LINE_DOUBLE: i64 = 480;

fn json_i64(value: &Value, key: &str) -> i64 {
    value
        .get(key)
        .and_then(|n| {
            n.as_i64()
                .or_else(|| n.as_u64().map(|u| u as i64))
                .or_else(|| n.as_f64().map(|f| f.round() as i64))
        })
        .unwrap_or(0)
}

/// Move `indentLeftTwips` by `delta`. A flush paragraph (`0`) does not change on a negative delta.
/// Returns whether the block JSON changed.
pub fn adjust_left_indent(
    host: &mut DocxHost,
    block_index: usize,
    delta: i64,
) -> Result<bool, EditError> {
    let current = {
        let block = host
            .paragraph(block_index)
            .map_err(|e| EditError::Other { message: e })?;
        json_i64(block, "indentLeftTwips")
    };
    let next = if delta < 0 {
        current.saturating_sub(delta.unsigned_abs() as i64).max(0)
    } else {
        current.saturating_add(delta)
    };
    if next == current {
        return Ok(false);
    }
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    if let Some(obj) = block.as_object_mut() {
        if next == 0 {
            obj.remove("indentLeftTwips");
        } else {
            obj.insert("indentLeftTwips".into(), json!(next));
        }
    }
    host.mark_touched(block_index);
    Ok(true)
}

/// Cycle direct line spacing 1.5 → double → single → 1.5 so one click always changes the paragraph.
pub fn cycle_line_spacing(host: &mut DocxHost, block_index: usize) -> Result<(), EditError> {
    let current = {
        let block = host
            .paragraph(block_index)
            .map_err(|e| EditError::Other { message: e })?;
        let n = json_i64(block, "lineSpacingTwips");
        if block.get("lineSpacingTwips").is_some() {
            Some(n)
        } else {
            None
        }
    };
    let next = match current {
        Some(LINE_ONE_HALF) => LINE_DOUBLE,
        Some(LINE_DOUBLE) => LINE_SINGLE,
        _ => LINE_ONE_HALF,
    };
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    if let Some(obj) = block.as_object_mut() {
        obj.insert("lineSpacingTwips".into(), json!(next));
        obj.insert("lineSpacingRule".into(), json!("auto"));
    }
    host.mark_touched(block_index);
    Ok(())
}

/// Toggle direct space before or after the paragraph. `key` is `spacingBeforeTwips` or
/// `spacingAfterTwips`. A paragraph already at 12pt drops the direct value; any other value
/// becomes 12pt so one click always changes the saved `w:spacing`.
pub fn toggle_paragraph_space(
    host: &mut DocxHost,
    block_index: usize,
    key: &str,
) -> Result<(), EditError> {
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let obj = block.as_object_mut().ok_or_else(|| EditError::Other {
        message: format!("paragraph {block_index} is not an object"),
    })?;
    let current = obj.get(key).and_then(|n| n.as_i64());
    if current == Some(PARAGRAPH_SPACE_TWIPS) {
        obj.remove(key);
    } else {
        obj.insert(key.into(), json!(PARAGRAPH_SPACE_TWIPS));
    }
    host.mark_touched(block_index);
    Ok(())
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
        for key in [
            "bold",
            "italic",
            "fontSize",
            "color",
            "underline",
            "strike",
            "highlight",
            "verticalAlign",
            "fontFamily",
        ] {
            if let Some(v) = patch.get(key) {
                let clear = matches!(key, "highlight" | "verticalAlign" | "fontFamily")
                    && v.as_str().is_some_and(|s| s.is_empty());
                if clear {
                    obj.remove(key);
                } else {
                    obj.insert(key.into(), v.clone());
                }
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
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": "Hi", "bold": false }],
        })]);
        let mut patch = serde_json::Map::new();
        patch.insert("bold".into(), json!(true));
        set_run_formatting(&mut host, 0, 0, &patch).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["text"], "Hi");
        assert_eq!(host.blocks[0]["runs"][0]["bold"], true);
    }

    #[test]
    fn catches_outdent_on_a_flush_paragraph_writing_a_zero_indent() {
        // Catches: Decrease Indent on a paragraph with no w:ind still storing indentLeftTwips 0, so save emits w:left="0".
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": "Hi" }],
        })]);
        assert!(!adjust_left_indent(&mut host, 0, -INDENT_STEP_TWIPS).unwrap());
        assert!(host.blocks[0].get("indentLeftTwips").is_none());
        assert!(adjust_left_indent(&mut host, 0, INDENT_STEP_TWIPS).unwrap());
        assert_eq!(host.blocks[0]["indentLeftTwips"], 720);
        assert!(adjust_left_indent(&mut host, 0, -INDENT_STEP_TWIPS).unwrap());
        assert!(host.blocks[0].get("indentLeftTwips").is_none());
    }
}
