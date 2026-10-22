// SPDX-License-Identifier: MIT
//! Character and paragraph formatting + style id on the docx edit host (#78).
//!
//! Script ops `setRunFormatting` / `setParagraphAlign` match `parity/edit/apply.mjs`.

use std::any::Any;

use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry, EditError};
use serde_json::{json, Map, Value};

use crate::run_formatting::{resolve_run_formatting, RunFormattingContext};

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
/// Word paragraph shading (`w:pPr/w:shd/@w:fill`). One click paints yellow.
pub const PARAGRAPH_SHADE: &str = "#FFFF00";
/// Word Bottom Border (`w:pBdr/w:bottom`), 1.5 pt single. Eighths of a point.
pub const PARAGRAPH_BORDER_SZ: i64 = 12;
/// Gap between the text and that rule, in points.
pub const PARAGRAPH_BORDER_SPACE: i64 = 1;
/// Word hanging indent (`w:ind/@w:hanging`). Half an inch, the same step as Increase Indent.
/// `w:firstLine` and `w:hanging` are mutually exclusive, so setting this drops a direct first line.
pub const HANGING_INDENT_TWIPS: i64 = INDENT_STEP_TWIPS;
/// Word character spacing (`w:spacing` on `w:rPr`), one point. The Font dialog steps by 1 pt.
pub const CHAR_SPACING_STEP_TWIPS: i64 = 20;
/// Word's Font dialog limit for expanded or condensed spacing (±1584 pt).
pub const CHAR_SPACING_LIMIT_TWIPS: i64 = 1584 * 20;

/// Word first-line indent (`w:ind/@w:firstLine`). Half an inch, the same step as Increase Indent.
/// `w:firstLine` and `w:hanging` are mutually exclusive, so setting this drops a direct hanging indent.
pub const FIRST_LINE_INDENT_TWIPS: i64 = INDENT_STEP_TWIPS;
/// Word character scale (`w:w`), ten percent. The Font dialog stores a percent from 1 to 600.
pub const TEXT_SCALE_STEP_PERCENT: i64 = 10;
pub const TEXT_SCALE_MIN_PERCENT: i64 = 1;
pub const TEXT_SCALE_MAX_PERCENT: i64 = 600;
/// Absent `w:w` is 100 percent.
pub const TEXT_SCALE_NORMAL_PERCENT: i64 = 100;

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
    adjust_named_indent(host, block_index, "indentLeftTwips", delta)
}

/// Move `indentRightTwips` (`w:ind/@w:right`) by `delta`. A flush paragraph does not change on a
/// negative delta, and `0` drops the field so save does not emit `w:right="0"`.
pub fn adjust_right_indent(
    host: &mut DocxHost,
    block_index: usize,
    delta: i64,
) -> Result<bool, EditError> {
    adjust_named_indent(host, block_index, "indentRightTwips", delta)
}

fn adjust_named_indent(
    host: &mut DocxHost,
    block_index: usize,
    key: &str,
    delta: i64,
) -> Result<bool, EditError> {
    let current = {
        let block = host
            .paragraph(block_index)
            .map_err(|e| EditError::Other { message: e })?;
        json_i64(block, key)
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
            obj.remove(key);
        } else {
            obj.insert(key.into(), json!(next));
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

fn shade_hex(value: &Value) -> Option<String> {
    let raw = value.as_str()?;
    let hex = raw.trim_start_matches('#').to_ascii_uppercase();
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(hex)
    } else {
        None
    }
}

/// Direct `w:shd`, or the paragraph style's fill. An empty direct value clears a style fill.
pub fn effective_paragraph_shading(host: &DocxHost, block_index: usize) -> Option<String> {
    let block = host.paragraph(block_index).ok()?;
    if block.get("shadingFill").is_some() {
        return block.get("shadingFill").and_then(shade_hex);
    }
    let style_id = block.get("style").and_then(|v| v.as_str())?;
    host.paragraph_styles
        .as_ref()
        .and_then(|c| c.get("styles"))
        .and_then(|s| s.get(style_id))
        .and_then(|s| s.get("formatting"))
        .and_then(|f| f.get("shadingFill"))
        .and_then(shade_hex)
}

/// Toggle yellow paragraph shading. A paragraph already yellow drops the direct fill, and writes
/// an empty fill when the style would otherwise keep yellow.
pub fn toggle_paragraph_shading(host: &mut DocxHost, block_index: usize) -> Result<(), EditError> {
    let yellow = effective_paragraph_shading(host, block_index).as_deref() == Some("FFFF00");
    let style_yellow = {
        let block = host
            .paragraph(block_index)
            .map_err(|e| EditError::Other { message: e })?;
        let style_id = block.get("style").and_then(|v| v.as_str());
        style_id
            .and_then(|id| {
                host.paragraph_styles
                    .as_ref()
                    .and_then(|c| c.get("styles"))
                    .and_then(|s| s.get(id))
                    .and_then(|s| s.get("formatting"))
                    .and_then(|f| f.get("shadingFill"))
                    .and_then(shade_hex)
            })
            .as_deref()
            == Some("FFFF00")
    };
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let obj = block.as_object_mut().ok_or_else(|| EditError::Other {
        message: format!("paragraph {block_index} is not an object"),
    })?;
    if yellow {
        if style_yellow {
            obj.insert("shadingFill".into(), json!(""));
        } else {
            obj.remove("shadingFill");
        }
    } else {
        obj.insert("shadingFill".into(), json!(PARAGRAPH_SHADE));
    }
    host.mark_touched(block_index);
    Ok(())
}

fn bottom_border_visible(borders: &Map<String, Value>) -> bool {
    let style = borders
        .get("bottom")
        .and_then(|side| side.get("style"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    !style.is_empty() && style != "nil" && style != "none"
}

fn style_paragraph_borders(host: &DocxHost, style_id: Option<&str>) -> Map<String, Value> {
    let Some(id) = style_id else {
        return Map::new();
    };
    host.paragraph_styles
        .as_ref()
        .and_then(|c| c.get("styles"))
        .and_then(|s| s.get(id))
        .and_then(|s| s.get("formatting"))
        .and_then(|f| f.get("borders"))
        .and_then(|b| b.as_object())
        .cloned()
        .unwrap_or_default()
}

/// Toggle the paragraph bottom border. A visible rule is removed; anything else becomes a
/// 1.5 pt single black rule. A styled rule is cleared with `w:val="nil"` so it does not stick.
/// Other sides on the effective border are kept.
pub fn toggle_paragraph_bottom_border(
    host: &mut DocxHost,
    block_index: usize,
) -> Result<(), EditError> {
    let (styled, direct, visible) = {
        let style_id = host
            .paragraph(block_index)
            .map_err(|e| EditError::Other { message: e })?
            .get("style")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let styled = style_paragraph_borders(host, style_id.as_deref());
        let direct = host
            .paragraph(block_index)
            .map_err(|e| EditError::Other { message: e })?
            .get("borders")
            .and_then(|b| b.as_object())
            .cloned();
        let effective = direct.clone().unwrap_or_else(|| styled.clone());
        let visible = bottom_border_visible(&effective);
        (styled, direct, visible)
    };
    let mut next = direct.unwrap_or_else(|| styled.clone());
    if visible {
        if bottom_border_visible(&styled) {
            next.insert("bottom".into(), json!({ "style": "nil" }));
        } else {
            next.remove("bottom");
        }
    } else {
        next.insert(
            "bottom".into(),
            json!({
                "style": "single",
                "sizeEighthPoints": PARAGRAPH_BORDER_SZ,
                "spacePoints": PARAGRAPH_BORDER_SPACE,
                "color": "000000",
            }),
        );
    }
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let obj = block.as_object_mut().ok_or_else(|| EditError::Other {
        message: format!("paragraph {block_index} is not an object"),
    })?;
    if next.is_empty() || next == styled {
        obj.remove("borders");
    } else {
        obj.insert("borders".into(), Value::Object(next));
    }
    host.mark_touched(block_index);
    Ok(())
}

/// Toggle a direct hanging indent. A paragraph already at half an inch drops `hangingTwips`.
/// Any other value, including none, becomes half an inch and drops `firstLineTwips`.
pub fn toggle_hanging_indent(host: &mut DocxHost, block_index: usize) -> Result<(), EditError> {
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let obj = block.as_object_mut().ok_or_else(|| EditError::Other {
        message: format!("paragraph {block_index} is not an object"),
    })?;
    let current = obj.get("hangingTwips").and_then(|n| n.as_i64());
    if current == Some(HANGING_INDENT_TWIPS) {
        obj.remove("hangingTwips");
    } else {
        obj.remove("firstLineTwips");
        obj.insert("hangingTwips".into(), json!(HANGING_INDENT_TWIPS));
    }
    host.mark_touched(block_index);
    Ok(())
}

/// Office Grow Font / Shrink Font sizes, in points. Same list Excel uses.
pub const OFFICE_FONT_SIZES_PT: &[f64] = &[
    8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0, 48.0, 72.0,
];

/// Next or previous size on [`OFFICE_FONT_SIZES_PT`]. Below 8 pt, shrink steps by 1 down to 1.
/// Above 72 pt, grow adds 10 up to 409.
pub fn step_office_font_pt(size: f64, grow: bool) -> f64 {
    if grow {
        OFFICE_FONT_SIZES_PT
            .iter()
            .copied()
            .find(|s| *s > size + f64::EPSILON)
            .unwrap_or_else(|| (size + 10.0).min(409.0))
    } else {
        OFFICE_FONT_SIZES_PT
            .iter()
            .rev()
            .copied()
            .find(|s| *s < size - f64::EPSILON)
            .unwrap_or_else(|| (size - 1.0).max(1.0))
    }
}

/// Direct run size, or the style's size, or 11 pt when nothing sets `w:sz`.
pub fn effective_run_font_pt(host: &DocxHost, block_index: usize, run_index: usize) -> f64 {
    let block = host.paragraph(block_index).ok();
    let style_id = block
        .and_then(|b| b.get("style"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let run = block
        .and_then(|b| b.get("runs"))
        .and_then(|r| r.as_array())
        .and_then(|runs| runs.get(run_index))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let resolved = resolve_run_formatting(
        &run,
        RunFormattingContext {
            paragraph_style_id: style_id,
            paragraph_catalog: host.paragraph_styles.clone(),
            run_catalog: host.character_styles.clone(),
        },
    );
    run_font_pt(&resolved)
}

/// Direct `w:spacing`, or the style's spacing, or 0 when nothing sets it.
pub fn effective_character_spacing_twips(
    host: &DocxHost,
    block_index: usize,
    run_index: usize,
) -> i64 {
    let block = host.paragraph(block_index).ok();
    let style_id = block
        .and_then(|b| b.get("style"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let run = block
        .and_then(|b| b.get("runs"))
        .and_then(|r| r.as_array())
        .and_then(|runs| runs.get(run_index))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let resolved = resolve_run_formatting(
        &run,
        RunFormattingContext {
            paragraph_style_id: style_id,
            paragraph_catalog: host.paragraph_styles.clone(),
            run_catalog: host.character_styles.clone(),
        },
    );
    resolved
        .get("characterSpacingTwips")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|n| n.round() as i64)))
        .unwrap_or(0)
}

/// Set direct character spacing. `0` drops the field when the style is also 0, and writes
/// an explicit `0` when the style would otherwise keep a non-zero spacing.
pub fn set_character_spacing(
    host: &mut DocxHost,
    block_index: usize,
    run_index: usize,
    twips: i64,
) -> Result<(), EditError> {
    {
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
        let obj = run.as_object_mut().ok_or_else(|| EditError::Other {
            message: format!("run {run_index} is not an object"),
        })?;
        if twips == 0 {
            obj.remove("characterSpacingTwips");
        } else {
            obj.insert("characterSpacingTwips".into(), json!(twips));
        }
    }
    if twips == 0 && effective_character_spacing_twips(host, block_index, run_index) != 0 {
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
        let obj = run.as_object_mut().ok_or_else(|| EditError::Other {
            message: format!("run {run_index} is not an object"),
        })?;
        obj.insert("characterSpacingTwips".into(), json!(0));
    }
    host.mark_touched(block_index);
    Ok(())
}

/// Direct `w:w`, or the style's scale, or 100 when nothing sets it.
pub fn effective_text_scale_percent(host: &DocxHost, block_index: usize, run_index: usize) -> i64 {
    let block = host.paragraph(block_index).ok();
    let style_id = block
        .and_then(|b| b.get("style"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let run = block
        .and_then(|b| b.get("runs"))
        .and_then(|r| r.as_array())
        .and_then(|runs| runs.get(run_index))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let resolved = resolve_run_formatting(
        &run,
        RunFormattingContext {
            paragraph_style_id: style_id,
            paragraph_catalog: host.paragraph_styles.clone(),
            run_catalog: host.character_styles.clone(),
        },
    );
    resolved
        .get("textScalePercent")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|n| n.round() as i64)))
        .filter(|n| *n > 0)
        .unwrap_or(TEXT_SCALE_NORMAL_PERCENT)
}

/// Set direct character scale. `100` drops the field when the style is also 100, and writes
/// an explicit `100` when the style would otherwise keep another percent.
pub fn set_text_scale(
    host: &mut DocxHost,
    block_index: usize,
    run_index: usize,
    percent: i64,
) -> Result<(), EditError> {
    {
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
        let obj = run.as_object_mut().ok_or_else(|| EditError::Other {
            message: format!("run {run_index} is not an object"),
        })?;
        if percent == TEXT_SCALE_NORMAL_PERCENT {
            obj.remove("textScalePercent");
        } else {
            obj.insert("textScalePercent".into(), json!(percent));
        }
    }
    if percent == TEXT_SCALE_NORMAL_PERCENT
        && effective_text_scale_percent(host, block_index, run_index) != TEXT_SCALE_NORMAL_PERCENT
    {
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
        let obj = run.as_object_mut().ok_or_else(|| EditError::Other {
            message: format!("run {run_index} is not an object"),
        })?;
        obj.insert("textScalePercent".into(), json!(TEXT_SCALE_NORMAL_PERCENT));
    }
    host.mark_touched(block_index);
    Ok(())
}

/// Direct run size, or 11 pt when the run has no `w:sz` (Calibri body).
pub fn run_font_pt(run: &Value) -> f64 {
    run.get("fontSize")
        .and_then(|v| v.as_f64())
        .filter(|n| n.is_finite() && *n > 0.0)
        .unwrap_or(11.0)
}

/// Toggle a direct first-line indent. A paragraph already at half an inch drops `firstLineTwips`.
/// Any other value, including none, becomes half an inch and drops `hangingTwips`.
pub fn toggle_first_line_indent(host: &mut DocxHost, block_index: usize) -> Result<(), EditError> {
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let obj = block.as_object_mut().ok_or_else(|| EditError::Other {
        message: format!("paragraph {block_index} is not an object"),
    })?;
    let current = obj.get("firstLineTwips").and_then(|n| n.as_i64());
    if current == Some(FIRST_LINE_INDENT_TWIPS) {
        obj.remove("firstLineTwips");
    } else {
        obj.remove("hangingTwips");
        obj.insert("firstLineTwips".into(), json!(FIRST_LINE_INDENT_TWIPS));
    }
    host.mark_touched(block_index);
    Ok(())
}

/// Direct `w:caps`, or the style chain when the run does not set it.
/// A title style can be all caps with no direct flag on the run.
pub fn effective_caps(host: &DocxHost, block_index: usize, run_index: usize) -> bool {
    let block = host.paragraph(block_index).ok();
    let style_id = block
        .and_then(|b| b.get("style"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let run = block
        .and_then(|b| b.get("runs"))
        .and_then(|r| r.as_array())
        .and_then(|runs| runs.get(run_index))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let resolved = resolve_run_formatting(
        &run,
        RunFormattingContext {
            paragraph_style_id: style_id,
            paragraph_catalog: host.paragraph_styles.clone(),
            run_catalog: host.character_styles.clone(),
        },
    );
    resolved
        .get("caps")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Toggle all caps from the effective value.
/// A styled all-caps run with no direct `w:caps` writes `false` (`w:caps w:val="0"`).
pub fn toggle_caps(
    host: &mut DocxHost,
    block_index: usize,
    run_index: usize,
) -> Result<(), EditError> {
    let on = effective_caps(host, block_index, run_index);
    let mut patch = serde_json::Map::new();
    patch.insert("caps".into(), json!(!on));
    set_run_formatting(host, block_index, run_index, &patch)
}

/// Direct `w:vanish`, or the style chain when the run does not set it.
pub fn effective_vanish(host: &DocxHost, block_index: usize, run_index: usize) -> bool {
    let block = host.paragraph(block_index).ok();
    let style_id = block
        .and_then(|b| b.get("style"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let run = block
        .and_then(|b| b.get("runs"))
        .and_then(|r| r.as_array())
        .and_then(|runs| runs.get(run_index))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let resolved = resolve_run_formatting(
        &run,
        RunFormattingContext {
            paragraph_style_id: style_id,
            paragraph_catalog: host.paragraph_styles.clone(),
            run_catalog: host.character_styles.clone(),
        },
    );
    resolved
        .get("vanish")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Toggle hidden text from the effective value.
/// A styled hidden run with no direct `w:vanish` writes `false` (`w:vanish w:val="0"`).
pub fn toggle_vanish(
    host: &mut DocxHost,
    block_index: usize,
    run_index: usize,
) -> Result<(), EditError> {
    let on = effective_vanish(host, block_index, run_index);
    let mut patch = serde_json::Map::new();
    patch.insert("vanish".into(), json!(!on));
    set_run_formatting(host, block_index, run_index, &patch)
}

/// Toggle a direct double underline (`w:u/@w:val="double"`).
/// A run already double-underlined drops the style and the underline.
/// Any other run becomes underlined with style `double`.
pub fn toggle_double_underline(
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
        message: format!("run {run_index} missing on block {block_index}"),
    })?;
    let obj = run.as_object_mut().ok_or_else(|| EditError::Other {
        message: format!("run {run_index} is not an object"),
    })?;
    let underlined = obj.get("underline").and_then(|v| v.as_bool()) == Some(true);
    let style = obj.get("underlineStyle").and_then(|v| v.as_str());
    if underlined && style == Some("double") {
        obj.insert("underline".into(), json!(false));
        obj.remove("underlineStyle");
    } else {
        obj.insert("underline".into(), json!(true));
        obj.insert("underlineStyle".into(), json!("double"));
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
            "underlineStyle",
            "strike",
            "highlight",
            "verticalAlign",
            "fontFamily",
            "caps",
            "vanish",
            "characterSpacingTwips",
            "textScalePercent",
        ] {
            if let Some(v) = patch.get(key) {
                let clear = matches!(
                    key,
                    "highlight" | "verticalAlign" | "fontFamily" | "underlineStyle"
                ) && v.as_str().is_some_and(|s| s.is_empty());
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

    #[test]
    fn catches_right_outdent_on_a_flush_paragraph_writing_a_zero_indent() {
        // Catches: Decrease Right Indent on a paragraph with no w:right still storing indentRightTwips 0, so save emits w:right="0".
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "indentLeftTwips": 720,
            "runs": [{ "text": "Hi" }],
        })]);
        assert!(!adjust_right_indent(&mut host, 0, -INDENT_STEP_TWIPS).unwrap());
        assert!(host.blocks[0].get("indentRightTwips").is_none());
        assert_eq!(host.blocks[0]["indentLeftTwips"], 720);
        assert!(adjust_right_indent(&mut host, 0, INDENT_STEP_TWIPS).unwrap());
        assert_eq!(host.blocks[0]["indentRightTwips"], 720);
        assert_eq!(host.blocks[0]["indentLeftTwips"], 720);
        assert!(adjust_right_indent(&mut host, 0, -INDENT_STEP_TWIPS).unwrap());
        assert!(host.blocks[0].get("indentRightTwips").is_none());
        assert_eq!(host.blocks[0]["indentLeftTwips"], 720);
    }

    #[test]
    fn catches_hanging_indent_keeping_a_first_line_indent() {
        // Catches: Hanging indent saving both w:firstLine and w:hanging, which Word treats as mutually exclusive.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "firstLineTwips": 360,
            "hangingTwips": 360,
            "runs": [{ "text": "Hi" }],
        })]);
        toggle_hanging_indent(&mut host, 0).unwrap();
        assert_eq!(host.blocks[0]["hangingTwips"], HANGING_INDENT_TWIPS);
        assert!(host.blocks[0].get("firstLineTwips").is_none());
        toggle_hanging_indent(&mut host, 0).unwrap();
        assert!(host.blocks[0].get("hangingTwips").is_none());
        assert!(host.blocks[0].get("firstLineTwips").is_none());
    }

    #[test]
    fn catches_first_line_indent_keeping_a_hanging_indent() {
        // Catches: First line indent saving both w:firstLine and w:hanging, which Word treats as mutually exclusive.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "firstLineTwips": 360,
            "hangingTwips": 360,
            "runs": [{ "text": "Hi" }],
        })]);
        toggle_first_line_indent(&mut host, 0).unwrap();
        assert_eq!(host.blocks[0]["firstLineTwips"], FIRST_LINE_INDENT_TWIPS);
        assert!(host.blocks[0].get("hangingTwips").is_none());
        toggle_first_line_indent(&mut host, 0).unwrap();
        assert!(host.blocks[0].get("firstLineTwips").is_none());
        assert!(host.blocks[0].get("hangingTwips").is_none());
    }

    #[test]
    fn catches_double_underline_on_a_single_underlined_run_dropping_the_style() {
        // Catches: double underline on a run that is already underlined leaving underlineStyle unset, so save keeps w:u val="single".
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": "Hi", "underline": true }],
        })]);
        toggle_double_underline(&mut host, 0, 0).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["underline"], true);
        assert_eq!(host.blocks[0]["runs"][0]["underlineStyle"], "double");
        assert_eq!(host.blocks[0]["runs"][0]["text"], "Hi");
        toggle_double_underline(&mut host, 0, 0).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["underline"], false);
        assert!(host.blocks[0]["runs"][0].get("underlineStyle").is_none());
    }

    #[test]
    fn catches_clearing_underline_leaving_a_double_style() {
        // Catches: turning underline off while underlineStyle stays "double", so the next underline writes w:u val="double".
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": "Hi", "underline": true, "underlineStyle": "double" }],
        })]);
        let mut off = serde_json::Map::new();
        off.insert("underline".into(), json!(false));
        off.insert("underlineStyle".into(), json!(""));
        set_run_formatting(&mut host, 0, 0, &off).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["underline"], false);
        assert!(host.blocks[0]["runs"][0].get("underlineStyle").is_none());
        let mut on = serde_json::Map::new();
        on.insert("underline".into(), json!(true));
        on.insert("underlineStyle".into(), json!(""));
        set_run_formatting(&mut host, 0, 0, &on).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["underline"], true);
        assert!(host.blocks[0]["runs"][0].get("underlineStyle").is_none());
    }

    #[test]
    fn catches_grow_font_jumping_from_11_to_a_fixed_size() {
        // Catches: Grow Font jumping from 11 pt to a fixed 18 instead of the next Office size, 12.
        assert_eq!(step_office_font_pt(11.0, true), 12.0);
        assert_eq!(step_office_font_pt(12.0, true), 14.0);
        assert_eq!(step_office_font_pt(12.0, false), 11.0);
        assert_eq!(step_office_font_pt(8.0, false), 7.0);
        assert_eq!(run_font_pt(&json!({ "text": "Hi" })), 11.0);
    }

    #[test]
    fn catches_grow_font_on_a_styled_title_writing_12pt() {
        // Catches: Grow Font on a 26 pt title with no direct w:sz writing 12 pt.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "style": "Title",
            "runs": [{ "text": "Hi" }],
        })]);
        host.paragraph_styles = Some(json!({ "styles": { "Title": { "id": "Title" } } }));
        host.character_styles = Some(json!({
            "styles": { "Title": { "formatting": { "fontSize": 26 } } }
        }));
        assert_eq!(effective_run_font_pt(&host, 0, 0), 26.0);
        assert_eq!(step_office_font_pt(26.0, true), 28.0);
    }

    #[test]
    fn catches_all_caps_leaving_caps_unset_on_a_sentence_case_run() {
        // Catches: All Caps leaving caps unset on a sentence-case run, so save omits w:caps.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": "Hi" }],
        })]);
        toggle_caps(&mut host, 0, 0).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["caps"], true);
        assert_eq!(host.blocks[0]["runs"][0]["text"], "Hi");
        toggle_caps(&mut host, 0, 0).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["caps"], false);
    }

    #[test]
    fn catches_all_caps_on_a_styled_title_writing_caps_true() {
        // Catches: All Caps on a title that is already all caps via its style writing caps true again, so the run stays uppercase.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "style": "Title",
            "runs": [{ "text": "Hi" }],
        })]);
        host.paragraph_styles = Some(json!({ "styles": { "Title": { "id": "Title" } } }));
        host.character_styles = Some(json!({
            "styles": { "Title": { "formatting": { "caps": true } } }
        }));
        assert!(effective_caps(&host, 0, 0));
        assert!(host.blocks[0]["runs"][0].get("caps").is_none());
        toggle_caps(&mut host, 0, 0).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["caps"], false);
        assert!(!effective_caps(&host, 0, 0));
    }

    #[test]
    fn catches_hidden_leaving_vanish_unset_on_a_visible_run() {
        // Catches: Hidden leaving vanish unset on a visible run, so save omits w:vanish.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": "Hi" }],
        })]);
        toggle_vanish(&mut host, 0, 0).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["vanish"], true);
        assert_eq!(host.blocks[0]["runs"][0]["text"], "Hi");
        toggle_vanish(&mut host, 0, 0).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["vanish"], false);
    }

    #[test]
    fn catches_hidden_on_a_styled_title_writing_vanish_false() {
        // Catches: Hidden on a title that is already hidden via its style writing vanish true again, so the run stays invisible.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "style": "Title",
            "runs": [{ "text": "Hi" }],
        })]);
        host.paragraph_styles = Some(json!({ "styles": { "Title": { "id": "Title" } } }));
        host.character_styles = Some(json!({
            "styles": { "Title": { "formatting": { "vanish": true } } }
        }));
        assert!(effective_vanish(&host, 0, 0));
        assert!(host.blocks[0]["runs"][0].get("vanish").is_none());
        toggle_vanish(&mut host, 0, 0).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["vanish"], false);
        assert!(!effective_vanish(&host, 0, 0));
    }

    #[test]
    fn catches_expand_spacing_leaving_w_spacing_off_the_run() {
        // Catches: Expand leaving characterSpacingTwips unset, so save omits w:spacing.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": "Hi" }],
        })]);
        assert_eq!(effective_character_spacing_twips(&host, 0, 0), 0);
        set_character_spacing(&mut host, 0, 0, CHAR_SPACING_STEP_TWIPS).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["characterSpacingTwips"], 20);
        assert_eq!(host.blocks[0]["runs"][0]["text"], "Hi");
        set_character_spacing(&mut host, 0, 0, 0).unwrap();
        assert!(host.blocks[0]["runs"][0]
            .get("characterSpacingTwips")
            .is_none());
    }

    #[test]
    fn catches_expand_spacing_on_a_styled_title_writing_20_twips() {
        // Catches: Expand on a title whose style spacing is -10 twips writing 20 twips.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "style": "Title",
            "runs": [{ "text": "Hi" }],
        })]);
        host.paragraph_styles = Some(json!({ "styles": { "Title": { "id": "Title" } } }));
        host.character_styles = Some(json!({
            "styles": { "Title": { "formatting": { "characterSpacingTwips": -10 } } }
        }));
        assert_eq!(effective_character_spacing_twips(&host, 0, 0), -10);
        set_character_spacing(&mut host, 0, 0, 10).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["characterSpacingTwips"], 10);
        set_character_spacing(&mut host, 0, 0, 0).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["characterSpacingTwips"], 0);
        assert_eq!(effective_character_spacing_twips(&host, 0, 0), 0);
    }

    #[test]
    fn catches_scale_up_leaving_w_w_off_a_normal_run() {
        // Catches: Scale up leaving textScalePercent unset, so save omits w:w.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": "Hi" }],
        })]);
        assert_eq!(effective_text_scale_percent(&host, 0, 0), 100);
        set_text_scale(&mut host, 0, 0, 110).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["textScalePercent"], 110);
        assert_eq!(host.blocks[0]["runs"][0]["text"], "Hi");
        set_text_scale(&mut host, 0, 0, TEXT_SCALE_NORMAL_PERCENT).unwrap();
        assert!(host.blocks[0]["runs"][0].get("textScalePercent").is_none());
    }

    #[test]
    fn catches_scale_up_on_a_90_percent_title_writing_110() {
        // Catches: Scale up on a title whose style scale is 90 percent writing 110 percent.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "style": "Title",
            "runs": [{ "text": "Hi" }],
        })]);
        host.paragraph_styles = Some(json!({ "styles": { "Title": { "id": "Title" } } }));
        host.character_styles = Some(json!({
            "styles": { "Title": { "formatting": { "textScalePercent": 90 } } }
        }));
        assert_eq!(effective_text_scale_percent(&host, 0, 0), 90);
        set_text_scale(&mut host, 0, 0, 100).unwrap();
        assert_eq!(host.blocks[0]["runs"][0]["textScalePercent"], 100);
        assert_eq!(effective_text_scale_percent(&host, 0, 0), 100);
    }

    #[test]
    fn catches_shade_leaving_w_shd_off_an_unshaded_paragraph() {
        // Catches: Shade leaving shadingFill unset, so save omits w:shd.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": "Hi" }],
        })]);
        assert!(effective_paragraph_shading(&host, 0).is_none());
        toggle_paragraph_shading(&mut host, 0).unwrap();
        assert_eq!(host.blocks[0]["shadingFill"], "#FFFF00");
        assert_eq!(host.blocks[0]["runs"][0]["text"], "Hi");
        toggle_paragraph_shading(&mut host, 0).unwrap();
        assert!(host.blocks[0].get("shadingFill").is_none());
    }

    #[test]
    fn catches_shade_on_a_green_title_writing_yellow() {
        // Catches: Shade on a title whose style fill is green writing yellow, then clearing back onto that green.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "style": "Title",
            "runs": [{ "text": "Hi" }],
        })]);
        host.paragraph_styles = Some(json!({
            "styles": { "Title": { "id": "Title", "formatting": { "shadingFill": "#00FF00" } } }
        }));
        assert_eq!(
            effective_paragraph_shading(&host, 0).as_deref(),
            Some("00FF00")
        );
        toggle_paragraph_shading(&mut host, 0).unwrap();
        assert_eq!(host.blocks[0]["shadingFill"], "#FFFF00");
        assert_eq!(
            effective_paragraph_shading(&host, 0).as_deref(),
            Some("FFFF00")
        );
        toggle_paragraph_shading(&mut host, 0).unwrap();
        assert!(host.blocks[0].get("shadingFill").is_none());
        assert_eq!(
            effective_paragraph_shading(&host, 0).as_deref(),
            Some("00FF00")
        );
    }

    #[test]
    fn catches_bottom_border_leaving_w_pbdr_off_an_unbordered_paragraph() {
        // Catches: Bottom border leaving borders unset, so save omits w:pBdr.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": "Hi" }],
        })]);
        toggle_paragraph_bottom_border(&mut host, 0).unwrap();
        assert_eq!(host.blocks[0]["borders"]["bottom"]["style"], "single");
        assert_eq!(host.blocks[0]["borders"]["bottom"]["sizeEighthPoints"], 12);
        assert_eq!(host.blocks[0]["runs"][0]["text"], "Hi");
        toggle_paragraph_bottom_border(&mut host, 0).unwrap();
        assert!(host.blocks[0].get("borders").is_none());
    }

    #[test]
    fn catches_bottom_border_on_a_blue_title_writing_nil() {
        // Catches: Bottom border on a title whose style rule is blue leaving that rule in place.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "style": "Title",
            "runs": [{ "text": "Hi" }],
        })]);
        host.paragraph_styles = Some(json!({
            "styles": { "Title": { "id": "Title", "formatting": { "borders": {
                "bottom": { "style": "single", "sizeEighthPoints": 8, "spacePoints": 4, "color": "4F81BD" }
            } } } }
        }));
        toggle_paragraph_bottom_border(&mut host, 0).unwrap();
        assert_eq!(host.blocks[0]["borders"]["bottom"]["style"], "nil");
        toggle_paragraph_bottom_border(&mut host, 0).unwrap();
        assert_eq!(host.blocks[0]["borders"]["bottom"]["style"], "single");
        assert_eq!(host.blocks[0]["borders"]["bottom"]["sizeEighthPoints"], 12);
    }

    #[test]
    fn catches_bottom_border_dropping_a_styled_left_rule() {
        // Catches: Bottom border on a paragraph whose style has a left rule dropping that left rule.
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "style": "Title",
            "runs": [{ "text": "Hi" }],
        })]);
        host.paragraph_styles = Some(json!({
            "styles": { "Title": { "formatting": { "borders": {
                "left": { "style": "single", "sizeEighthPoints": 4, "color": "FF0000" }
            } } } }
        }));
        toggle_paragraph_bottom_border(&mut host, 0).unwrap();
        assert_eq!(host.blocks[0]["borders"]["left"]["style"], "single");
        assert_eq!(host.blocks[0]["borders"]["left"]["color"], "FF0000");
        assert_eq!(host.blocks[0]["borders"]["bottom"]["style"], "single");
    }
}
