// SPDX-License-Identifier: MIT
//! mergeCells and unmerge command runners (#588).

use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry, EditError};
use serde_json::Value;

use crate::address::parse_range;
use crate::model::{CellAddress, CellRange};

use super::cells::{clear_contents, ensure_cell, host_mut, prune_cell};
use super::style_patch::{apply_style_patch, StylePatch};

fn arg_u32(args: &Value, key: &str) -> Result<u32, EditError> {
    args.get(key)
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .ok_or_else(|| EditError::Other {
            message: format!("missing or invalid `{key}`"),
        })
}

fn sheet_err(e: String) -> EditError {
    EditError::Other { message: e }
}

fn ranges_overlap(a: &CellRange, b: &CellRange) -> bool {
    a.start.row <= b.end.row
        && a.end.row >= b.start.row
        && a.start.col <= b.end.col
        && a.end.col >= b.start.col
}

fn apply_merge(
    host: &mut super::host::XlsxHost,
    sheet: u32,
    range: &CellRange,
    center: bool,
) -> Result<(), EditError> {
    for row in range.start.row..=range.end.row {
        for col in range.start.col..=range.end.col {
            host.mark_touched(sheet, row, col);
        }
    }

    // Clear non-top-left cells' contents
    {
        let ws = host.sheet_mut(sheet).map_err(sheet_err)?;
        ws.ensure_cell_pos();
        let mut to_clear: Vec<(u32, u32)> = Vec::new();
        for row in range.start.row..=range.end.row {
            for col in range.start.col..=range.end.col {
                if row == range.start.row && col == range.start.col {
                    continue;
                }
                to_clear.push((row, col));
            }
        }
        for (row, col) in to_clear {
            if let Some(i) = ws.find_cell_pos(row, col) {
                clear_contents(&mut ws.cells[i]);
            }
            prune_cell(ws, row, col);
        }
    }

    // Apply center alignment to top-left cell when mode is merge-center
    if center {
        let style_id = host
            .sheet(sheet)
            .map_err(sheet_err)?
            .cells
            .iter()
            .find(|c| c.row == range.start.row && c.col == range.start.col)
            .and_then(|c| c.style_id);
        let mut m = serde_json::Map::new();
        m.insert("horizontal".into(), Value::String("center".into()));
        let patch = StylePatch {
            alignment: Some(m),
            ..StylePatch::default()
        };
        let new_style = apply_style_patch(
            &mut host.workbook.styles,
            &mut host.style_cache,
            style_id,
            &patch,
        );
        let ws = host.sheet_mut(sheet).map_err(sheet_err)?;
        let cell = ensure_cell(ws, range.start.row, range.start.col);
        if new_style != 0 {
            cell.style_id = Some(new_style);
        } else {
            cell.style_id = None;
        }
        host.styles_dirty = true;
    }

    // Register the merge (drop any overlapping merges first)
    let new_merge = range.clone();
    let ws = host.sheet_mut(sheet).map_err(sheet_err)?;
    ws.merges.retain(|m| !ranges_overlap(m, &new_merge));
    if range.start.row != range.end.row || range.start.col != range.end.col {
        ws.merges.push(new_merge);
    }
    host.dirty_sheets.insert(sheet);
    Ok(())
}

pub fn register_merge_commands(registry: &mut CommandRegistry) {
    registry.register(
        Command::stub("mergeCells", "mergeCells").with_runner(|host, args| {
            let h = host_mut(host)?;
            let sheet = arg_u32(args, "sheet")?;
            let range_str =
                args.get("range")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EditError::Other {
                        message: "mergeCells missing range".into(),
                    })?;
            let range = parse_range(range_str).ok_or_else(|| EditError::Other {
                message: format!("bad range {range_str}"),
            })?;
            let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("merge");
            match mode {
                "merge-center" => apply_merge(h, sheet, &range, true)?,
                "merge-across" => {
                    for row in range.start.row..=range.end.row {
                        let row_range = CellRange {
                            start: CellAddress {
                                row,
                                col: range.start.col,
                            },
                            end: CellAddress {
                                row,
                                col: range.end.col,
                            },
                        };
                        apply_merge(h, sheet, &row_range, false)?;
                    }
                }
                _ => apply_merge(h, sheet, &range, false)?,
            }
            Ok(ChangeSet::new(ChangeKind::Sheets, "mergeCells").with_sheet(sheet))
        }),
    );

    registry.register(
        Command::stub("unmerge", "unmerge").with_runner(|host, args| {
            let h = host_mut(host)?;
            let sheet = arg_u32(args, "sheet")?;
            let range_str =
                args.get("range")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EditError::Other {
                        message: "unmerge missing range".into(),
                    })?;
            let range = parse_range(range_str).ok_or_else(|| EditError::Other {
                message: format!("bad range {range_str}"),
            })?;
            let ws = h.sheet_mut(sheet).map_err(sheet_err)?;
            ws.merges.retain(|m| !ranges_overlap(m, &range));
            h.dirty_sheets.insert(sheet);
            Ok(ChangeSet::new(ChangeKind::Sheets, "unmerge").with_sheet(sheet))
        }),
    );
}
