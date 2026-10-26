// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/edit/cell-values.ts
// (writeInput, writeValue, clearContents) and src/core/xlsx/edit/format.ts
// (applyStyle / patchRangeWith) plus ensureCell/pruneCell/baseStyleId from
// src/core/xlsx/edit/context.ts; changes listed in PROVENANCE.md.
//! Cell value and style command runners for the shared edit registry (#66).

use std::any::Any;
use std::collections::HashMap;

use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry, EditError};
use serde_json::Value;

use crate::address::{parse_range, MAX_COL, MAX_ROW};
use crate::model::{ActiveWorksheet, CellRange, DumpCell, DumpCellValue};
use crate::numfmt::{parse_cell_input, CellValue, FormatOptions};
use crate::styles::style_at;

use super::host::XlsxHost;
use super::style_patch::{apply_style_patch, style_patch_from_value, StylePatch};

fn host_mut(host: &mut dyn Any) -> Result<&mut XlsxHost, EditError> {
    host.downcast_mut::<XlsxHost>()
        .ok_or_else(|| EditError::Other {
            message: "expected XlsxHost".into(),
        })
}

/// Style a new cell inherits: row format, else column format.
pub fn base_style_id(sheet: &ActiveWorksheet, row: u32, col: u32) -> Option<u32> {
    if let Some(id) = sheet.row_info.get(&row).and_then(|info| info.style_id) {
        if id != 0 {
            return Some(id);
        }
    }
    sheet
        .columns
        .iter()
        .find(|c| col >= c.min && col <= c.max)
        .and_then(|c| c.style_id.filter(|id| *id != 0))
}

fn find_cell_index(sheet: &ActiveWorksheet, row: u32, col: u32) -> Option<usize> {
    sheet.find_cell_pos(row, col)
}

fn get_cell(sheet: &ActiveWorksheet, row: u32, col: u32) -> Option<&DumpCell> {
    find_cell_index(sheet, row, col).map(|i| &sheet.cells[i])
}

/// Stored cell at a position, created (with inherited format) when missing.
pub fn ensure_cell(sheet: &mut ActiveWorksheet, row: u32, col: u32) -> &mut DumpCell {
    sheet.ensure_cell_pos();
    if let Some(i) = find_cell_index(sheet, row, col) {
        return &mut sheet.cells[i];
    }
    let style_id = base_style_id(sheet, row, col);
    sheet.push_cell(DumpCell {
        row,
        col,
        value: DumpCellValue::Null,
        type_tag: None,
        formula: None,
        style_id,
        rich_text: None,
        array_range: None,
        legacy_formula: None,
        dynamic_array: None,
        spill_anchor: None,
        cell_metadata: None,
        value_metadata: None,
    });
    let i = sheet.cells.len() - 1;
    &mut sheet.cells[i]
}

pub fn clear_contents(cell: &mut DumpCell) {
    cell.value = DumpCellValue::Null;
    cell.formula = None;
    cell.rich_text = None;
    cell.array_range = None;
    cell.dynamic_array = None;
    cell.legacy_formula = None;
    cell.spill_anchor = None;
    cell.type_tag = None;
}

/// Drops a cell when it carries nothing worth keeping.
pub fn prune_cell(sheet: &mut ActiveWorksheet, row: u32, col: u32) {
    sheet.ensure_cell_pos();
    let Some(i) = find_cell_index(sheet, row, col) else {
        return;
    };
    let cell = &sheet.cells[i];
    let empty = matches!(cell.value, DumpCellValue::Null)
        && cell.formula.is_none()
        && cell.style_id.is_none()
        && cell.rich_text.is_none();
    if empty {
        sheet.remove_cell_pos(i);
    }
}

fn cell_value_to_dump(value: &CellValue) -> DumpCellValue {
    match value {
        CellValue::Blank => DumpCellValue::Null,
        CellValue::Number(n) => DumpCellValue::Number(*n),
        CellValue::Text(s) => DumpCellValue::String(s.clone()),
        CellValue::Bool(b) => DumpCellValue::Bool(*b),
        CellValue::Error(e) => DumpCellValue::Error(e.clone()),
    }
}

fn json_to_cell_value(v: &Value) -> Result<CellValue, EditError> {
    match v {
        Value::Null => Ok(CellValue::Blank),
        Value::Bool(b) => Ok(CellValue::Bool(*b)),
        Value::Number(n) => Ok(CellValue::Number(n.as_f64().unwrap_or(0.0))),
        Value::String(s) => {
            if s.is_empty() {
                Ok(CellValue::Blank)
            } else {
                Ok(CellValue::Text(s.clone()))
            }
        }
        Value::Object(m) if m.contains_key("error") => Ok(CellValue::Error(
            m.get("error")
                .and_then(|x| x.as_str())
                .unwrap_or("#VALUE!")
                .to_owned(),
        )),
        other => Err(EditError::Other {
            message: format!("unsupported cell value: {other}"),
        }),
    }
}

fn sheet_err(e: String) -> EditError {
    EditError::Other { message: e }
}

/// Writes typed text into a cell without opening an undo step.
pub fn write_input(
    host: &mut XlsxHost,
    sheet_index: u32,
    row: u32,
    col: u32,
    text: &str,
) -> Result<(), EditError> {
    host.mark_touched(sheet_index, row, col);
    let date1904 = host.workbook.date1904;
    let format = {
        let sheet = host.sheet(sheet_index).map_err(sheet_err)?;
        let existing = get_cell(sheet, row, col);
        let style_id = existing
            .and_then(|c| c.style_id)
            .or_else(|| base_style_id(sheet, row, col));
        style_at(&host.workbook.styles, style_id).num_fmt.clone()
    };

    if text.is_empty() {
        {
            let sheet = host.sheet_mut(sheet_index).map_err(sheet_err)?;
            if let Some(i) = find_cell_index(sheet, row, col) {
                clear_contents(&mut sheet.cells[i]);
            }
            prune_cell(sheet, row, col);
        }
        host.note_value_edit(sheet_index, row, col);
        return Ok(());
    }

    {
        let sheet = host.sheet_mut(sheet_index).map_err(sheet_err)?;
        let cell = ensure_cell(sheet, row, col);
        clear_contents(cell);
        if format == "@" {
            cell.value = DumpCellValue::String(text.to_owned());
        }
    }
    if format == "@" {
        host.note_value_edit(sheet_index, row, col);
        return Ok(());
    }

    let parsed = parse_cell_input(
        text,
        &FormatOptions {
            date1904,
            ..FormatOptions::default()
        },
    );

    let num_fmt_patch = parsed.num_fmt.filter(|_| format == "General");
    {
        let sheet = host.sheet_mut(sheet_index).map_err(sheet_err)?;
        let cell = ensure_cell(sheet, row, col);
        if let Some(formula) = parsed.formula {
            cell.formula = Some(formula);
            cell.value = DumpCellValue::Null;
        } else {
            cell.value = cell_value_to_dump(&parsed.value);
        }
    }

    if let Some(num_fmt) = num_fmt_patch {
        let style_id = {
            let sheet = host.sheet(sheet_index).map_err(sheet_err)?;
            get_cell(sheet, row, col).and_then(|c| c.style_id)
        };
        let patch = StylePatch {
            num_fmt: Some(num_fmt),
            ..StylePatch::default()
        };
        let next = apply_style_patch(
            &mut host.workbook.styles,
            &mut host.style_cache,
            style_id,
            &patch,
        );
        let sheet = host.sheet_mut(sheet_index).map_err(sheet_err)?;
        let cell = ensure_cell(sheet, row, col);
        set_style_id(cell, next);
        prune_cell(sheet, row, col);
        host.styles_dirty = true;
    }
    host.note_value_edit(sheet_index, row, col);
    Ok(())
}

/// Writes a value into a cell (no parsing) without opening an undo step.
pub fn write_value(
    host: &mut XlsxHost,
    sheet_index: u32,
    row: u32,
    col: u32,
    value: &CellValue,
) -> Result<(), EditError> {
    host.mark_touched(sheet_index, row, col);
    {
        let sheet = host.sheet_mut(sheet_index).map_err(sheet_err)?;
        let clear = matches!(value, CellValue::Blank)
            || matches!(value, CellValue::Text(s) if s.is_empty());
        if clear {
            if let Some(i) = find_cell_index(sheet, row, col) {
                clear_contents(&mut sheet.cells[i]);
            }
            prune_cell(sheet, row, col);
        } else {
            let cell = ensure_cell(sheet, row, col);
            clear_contents(cell);
            cell.value = cell_value_to_dump(value);
        }
    }
    host.note_value_edit(sheet_index, row, col);
    Ok(())
}

fn set_style_id(cell: &mut DumpCell, id: u32) {
    if id != 0 {
        cell.style_id = Some(id);
    } else {
        cell.style_id = None;
    }
}

fn normalize_range(range: CellRange) -> CellRange {
    CellRange {
        start: crate::model::CellAddress {
            row: range.start.row.min(range.end.row),
            col: range.start.col.min(range.end.col),
        },
        end: crate::model::CellAddress {
            row: range.start.row.max(range.end.row),
            col: range.start.col.max(range.end.col),
        },
    }
}

/// Applies a patch to every cell of a range in place (no undo step).
pub fn patch_range(
    host: &mut XlsxHost,
    sheet_index: u32,
    range: &CellRange,
    patch: &StylePatch,
) -> Result<(), EditError> {
    let r = normalize_range(range.clone());
    let mut memo: HashMap<u32, u32> = HashMap::new();
    for row in r.start.row..=r.end.row {
        for col in r.start.col..=r.end.col {
            host.mark_touched(sheet_index, row, col);
            let style_id = {
                let sheet = host.sheet(sheet_index).map_err(sheet_err)?;
                get_cell(sheet, row, col).and_then(|c| c.style_id)
            };
            let key = style_id.unwrap_or(0);
            let next = if let Some(&n) = memo.get(&key) {
                n
            } else {
                let n = apply_style_patch(
                    &mut host.workbook.styles,
                    &mut host.style_cache,
                    style_id,
                    patch,
                );
                memo.insert(key, n);
                n
            };
            let sheet = host.sheet_mut(sheet_index).map_err(sheet_err)?;
            let cell = ensure_cell(sheet, row, col);
            set_style_id(cell, next);
            prune_cell(sheet, row, col);
        }
    }
    host.dirty_sheets.insert(sheet_index);
    host.styles_dirty = true;
    Ok(())
}

fn arg_u32(args: &Value, key: &str) -> Result<u32, EditError> {
    args.get(key)
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .ok_or_else(|| EditError::Other {
            message: format!("missing or invalid `{key}`"),
        })
}

/// Register `setCellInput`, `setCellValue`, and `applyStyle` runners.
pub fn register_cell_commands(registry: &mut CommandRegistry) {
    registry.register(
        Command::stub("setCellInput", "setCellInput").with_runner(|host, args| {
            let h = host_mut(host)?;
            let sheet = arg_u32(args, "sheet")?;
            let row = arg_u32(args, "row")?;
            let col = arg_u32(args, "col")?;
            let text =
                args.get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EditError::Other {
                        message: "setCellInput missing text".into(),
                    })?;
            write_input(h, sheet, row, col, text)?;
            Ok(ChangeSet::new(ChangeKind::Cells, "setCellInput").with_sheet(sheet))
        }),
    );
    // One undo step for a pasted or cleared block (`rows` is a grid of cell input texts).
    registry.register(
        Command::stub("setCellBlock", "setCellBlock").with_runner(|host, args| {
            let h = host_mut(host)?;
            let sheet = arg_u32(args, "sheet")?;
            let row = arg_u32(args, "row")?;
            let col = arg_u32(args, "col")?;
            let rows =
                args.get("rows")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| EditError::Other {
                        message: "setCellBlock missing rows".into(),
                    })?;
            for (dr, line) in rows.iter().enumerate() {
                let cells = line.as_array().ok_or_else(|| EditError::Other {
                    message: "setCellBlock rows must be arrays".into(),
                })?;
                for (dc, cell) in cells.iter().enumerate() {
                    let text = cell.as_str().ok_or_else(|| EditError::Other {
                        message: "setCellBlock cells must be strings".into(),
                    })?;
                    let (r, c) = (row + dr as u32, col + dc as u32);
                    if r > MAX_ROW || c > MAX_COL {
                        continue;
                    }
                    write_input(h, sheet, r, c, text)?;
                }
            }
            Ok(ChangeSet::new(ChangeKind::Cells, "setCellBlock").with_sheet(sheet))
        }),
    );
    registry.register(
        Command::stub("setCellValue", "setCellValue").with_runner(|host, args| {
            let h = host_mut(host)?;
            let sheet = arg_u32(args, "sheet")?;
            let row = arg_u32(args, "row")?;
            let col = arg_u32(args, "col")?;
            let value = args.get("value").ok_or_else(|| EditError::Other {
                message: "setCellValue missing value".into(),
            })?;
            let cv = json_to_cell_value(value)?;
            write_value(h, sheet, row, col, &cv)?;
            Ok(ChangeSet::new(ChangeKind::Cells, "setCellValue").with_sheet(sheet))
        }),
    );
    registry.register(
        Command::stub("applyStyle", "applyStyle").with_runner(|host, args| {
            let h = host_mut(host)?;
            let sheet = arg_u32(args, "sheet")?;
            let range_str =
                args.get("range")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EditError::Other {
                        message: "applyStyle missing range".into(),
                    })?;
            let range = parse_range(range_str).ok_or_else(|| EditError::Other {
                message: format!("bad range {range_str}"),
            })?;
            let patch_v = args.get("patch").ok_or_else(|| EditError::Other {
                message: "applyStyle missing patch".into(),
            })?;
            let patch =
                style_patch_from_value(patch_v).map_err(|e| EditError::Other { message: e })?;
            patch_range(h, sheet, &range, &patch)?;
            Ok(ChangeSet::new(ChangeKind::Format, "applyStyle").with_sheet(sheet))
        }),
    );
}
