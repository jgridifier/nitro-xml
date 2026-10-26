// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/ui/commands/font.ts,
// alignment.ts and number.ts (style patches for Home ribbon ids); changes listed in PROVENANCE.md.
//! Map chrome command ids onto #66/#67 edit ops (`applyStyle`, `setCellInput`, sheets).

use serde_json::{json, Value};

use crate::address::{format_address, parse_range};
use crate::edit::host::XlsxHost;
use crate::model::CellAddress;
use crate::numfmt::BUILTIN_NUMBER_FORMATS;
use crate::styles::style_at;

/// Active cell plus the A1 range the ribbon should format.
#[derive(Debug, Clone)]
pub struct SelectionTarget {
    pub sheet: u32,
    pub row: u32,
    pub col: u32,
    pub range: String,
}

impl SelectionTarget {
    pub fn cell(sheet: u32, row: u32, col: u32) -> Self {
        Self {
            sheet,
            row,
            col,
            range: format_address(&CellAddress { row, col }),
        }
    }
}

/// A chrome id that this editor can run, or a pass-through the stub registry still records.
#[derive(Debug, Clone)]
pub enum MappedCommand {
    Save,
    Undo,
    Redo,
    /// `(op id, args, structure change, history label)`.
    Op {
        id: &'static str,
        args: Value,
        structure: bool,
        label: &'static str,
    },
    Unmapped,
}

const FONT_SIZES: &[f64] = &[
    8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0, 48.0, 72.0,
];

/// Ribbon / shortcut ids that correspond to cell, style, or sheet ops.
pub fn map_chrome_command(id: &str, target: &SelectionTarget, host: &XlsxHost) -> MappedCommand {
    match id {
        "file.save" => return MappedCommand::Save,
        "edit.undo" => return MappedCommand::Undo,
        "edit.redo" => return MappedCommand::Redo,
        // Cut and Paste need the browser clipboard; the loader runs them (setCellBlock) and
        // this arm only records that the ids are known to the editor.
        "home.cut" | "home.paste" => return MappedCommand::Unmapped,
        "home.clear-contents" => {
            return op(
                "setCellInput",
                json!({
                    "sheet": target.sheet,
                    "row": target.row,
                    "col": target.col,
                    "text": "",
                }),
                false,
                "Clear Contents",
            );
        }
        "sheet.insert" => {
            return op(
                "addSheet",
                json!({ "at": target.sheet.saturating_add(1) }),
                true,
                "Insert Sheet",
            );
        }
        "sheet.delete" => {
            return op(
                "deleteSheet",
                json!({ "index": target.sheet }),
                true,
                "Delete Sheet",
            );
        }
        "cells.insert-rows"
        | "cells.delete-rows"
        | "cells.insert-columns"
        | "cells.delete-columns" => {
            return shift_op(id, target);
        }
        _ => {}
    }

    let Some(patch) = style_patch(id, target, host) else {
        return MappedCommand::Unmapped;
    };
    op(
        "applyStyle",
        json!({
            "sheet": target.sheet,
            "range": style_range(target),
            "patch": patch,
        }),
        false,
        "Format",
    )
}

/// Insert or delete the rows/columns the selection spans (Excel: Insert/Delete Sheet Rows/Columns).
/// Insert puts as many blank lines before the first selected one as the selection covers.
fn shift_op(id: &str, target: &SelectionTarget) -> MappedCommand {
    let range = parse_range(&target.range).unwrap_or_else(|| {
        let a = CellAddress {
            row: target.row,
            col: target.col,
        };
        crate::model::CellRange { start: a, end: a }
    });
    let (rows, insert) = (id.ends_with("rows"), id.starts_with("cells.insert"));
    let (at, count) = if rows {
        (
            range.start.row,
            range.end.row.saturating_sub(range.start.row) + 1,
        )
    } else {
        (
            range.start.col,
            range.end.col.saturating_sub(range.start.col) + 1,
        )
    };
    let (cmd, label) = match (rows, insert) {
        (true, true) => ("insertRows", "Insert Sheet Rows"),
        (true, false) => ("deleteRows", "Delete Sheet Rows"),
        (false, true) => ("insertColumns", "Insert Sheet Columns"),
        (false, false) => ("deleteColumns", "Delete Sheet Columns"),
    };
    op(
        cmd,
        json!({ "sheet": target.sheet, "at": at, "count": count }),
        true,
        label,
    )
}

fn op(id: &'static str, args: Value, structure: bool, label: &'static str) -> MappedCommand {
    MappedCommand::Op {
        id,
        args,
        structure,
        label,
    }
}

fn style_patch(id: &str, target: &SelectionTarget, host: &XlsxHost) -> Option<Value> {
    let style_id = host.sheet(target.sheet).ok().and_then(|sheet| {
        sheet
            .cells
            .iter()
            .find(|c| c.row == target.row && c.col == target.col)
            .and_then(|c| c.style_id)
    });
    let style = style_at(&host.workbook.styles, style_id);
    let font = &style.font;
    let align = style.alignment.as_ref();
    match id {
        "home.bold" => Some(font_toggle("bold", font.bold == Some(true), json!(true))),
        "home.italic" => Some(font_toggle(
            "italic",
            font.italic == Some(true),
            json!(true),
        )),
        "home.strikethrough" => Some(font_toggle(
            "strike",
            font.strike == Some(true),
            json!(true),
        )),
        "home.underline" => Some(font_toggle(
            "underline",
            font.underline.as_deref() == Some("single"),
            json!("single"),
        )),
        "home.underline-double" => Some(font_toggle(
            "underline",
            font.underline.as_deref() == Some("double"),
            json!("double"),
        )),
        "home.grow-font" => Some(json!({
            "font": { "size": step_font_size(font.size.unwrap_or(11.0), true) }
        })),
        "home.shrink-font" => Some(json!({
            "font": { "size": step_font_size(font.size.unwrap_or(11.0), false) }
        })),
        "home.align-left" => Some(halign(align.and_then(|a| a.horizontal.as_deref()), "left")),
        "home.align-center" => Some(halign(
            align.and_then(|a| a.horizontal.as_deref()),
            "center",
        )),
        "home.align-right" => Some(halign(align.and_then(|a| a.horizontal.as_deref()), "right")),
        "home.align-top" => Some(valign(align.and_then(|a| a.vertical.as_deref()), "top")),
        "home.align-middle" => Some(valign(align.and_then(|a| a.vertical.as_deref()), "center")),
        "home.align-bottom" => Some(valign(align.and_then(|a| a.vertical.as_deref()), "bottom")),
        "home.wrap-text" => {
            let on = align.and_then(|a| a.wrap_text) == Some(true);
            Some(json!({
                "alignment": { "wrapText": if on { Value::Null } else { json!(true) } }
            }))
        }
        "home.percent" | "home.format-percent" => Some(num_fmt("0%")),
        "home.comma" => Some(num_fmt(builtin(43))),
        "home.accounting" => Some(num_fmt(builtin(44))),
        "home.format-general" => Some(num_fmt("General")),
        "home.format-number" => Some(num_fmt("#,##0.00")),
        "home.format-currency" => Some(num_fmt(r##""$"#,##0.00_);\("$"#,##0.00\)"##)),
        "home.format-scientific" => Some(num_fmt("0.00E+00")),
        "home.format-date" => Some(num_fmt("d-mmm-yy")),
        "home.format-time" => Some(num_fmt("h:mm AM/PM")),
        _ => None,
    }
}

fn font_toggle(key: &str, on: bool, on_value: Value) -> Value {
    let mut font = serde_json::Map::new();
    font.insert(key.to_owned(), if on { Value::Null } else { on_value });
    json!({ "font": font })
}

fn halign(current: Option<&str>, value: &str) -> Value {
    let next = if current == Some(value) {
        Value::Null
    } else {
        Value::String(value.to_owned())
    };
    json!({ "alignment": { "horizontal": next } })
}

fn valign(current: Option<&str>, value: &str) -> Value {
    // Bottom is the default, so choosing it clears the stored vertical align.
    let next = if value == "bottom" || current == Some(value) {
        Value::Null
    } else {
        Value::String(value.to_owned())
    };
    json!({ "alignment": { "vertical": next } })
}

fn num_fmt(code: &str) -> Value {
    json!({ "numFmt": code })
}

fn builtin(id: u16) -> &'static str {
    BUILTIN_NUMBER_FORMATS
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, code)| *code)
        .unwrap_or("General")
}

fn step_font_size(size: f64, grow: bool) -> f64 {
    if grow {
        FONT_SIZES
            .iter()
            .copied()
            .find(|s| *s > size + f64::EPSILON)
            .unwrap_or_else(|| (size + 10.0).min(409.0))
    } else {
        FONT_SIZES
            .iter()
            .rev()
            .copied()
            .find(|s| *s < size - f64::EPSILON)
            .unwrap_or_else(|| (size - 1.0).max(1.0))
    }
}

/// Cap a style range so a whole-sheet selection cannot intern a million xfs.
fn style_range(target: &SelectionTarget) -> String {
    let fallback = format_address(&CellAddress {
        row: target.row,
        col: target.col,
    });
    let Some(range) = parse_range(&target.range) else {
        return fallback;
    };
    let rows = u64::from(range.end.row.abs_diff(range.start.row)) + 1;
    let cols = u64::from(range.end.col.abs_diff(range.start.col)) + 1;
    if rows.saturating_mul(cols) > 4096 {
        fallback
    } else {
        target.range.clone()
    }
}
