// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/edit/sheets.ts,
// src/core/xlsx/edit/dimensions.ts, src/core/xlsx/edit/columns.ts, and
// sheet-name helpers in src/core/xlsx/workbook.ts; changes listed in PROVENANCE.md.
//! Row/column size and hide; sheet add, rename, delete and reorder (#67).
//!
//! Formula sheet-ref rewrite (`renameSheetInFormula` / `deleteSheetInFormula`) waits on the
//! formula transform port — the #50 structure scripts do not exercise cross-sheet refs.

use std::any::Any;
use std::collections::{BTreeMap, HashMap, HashSet};

use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry, EditError};
use serde_json::Value;

use crate::model::{ActiveWorksheet, ColumnInfo, RowInfo, SheetState, SheetStub};
use crate::sheet_props::default_sheet_view;

use super::host::XlsxHost;

const MAX_WIDTH: f64 = 255.0;
const MAX_HEIGHT: f64 = 409.0;
const INVALID_SHEET_CHARS: &[char] = &['\\', '/', '?', '*', '[', ']', ':'];

fn host_mut(host: &mut dyn Any) -> Result<&mut XlsxHost, EditError> {
    host.downcast_mut::<XlsxHost>()
        .ok_or_else(|| EditError::Other {
            message: "expected XlsxHost".into(),
        })
}

fn arg_u32(args: &Value, key: &str) -> Result<u32, EditError> {
    args.get(key)
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .ok_or_else(|| EditError::Other {
            message: format!("missing or invalid `{key}`"),
        })
}

fn assert_unlocked(host: &XlsxHost) -> Result<(), EditError> {
    if host.workbook.structure_locked == Some(true) {
        return Err(EditError::Other {
            message: "The workbook structure is protected.".into(),
        });
    }
    Ok(())
}

/// Why a sheet name is invalid, or `None` when acceptable (Excel rules).
pub fn validate_sheet_name(
    host: &XlsxHost,
    name: &str,
    except_index: Option<usize>,
) -> Option<String> {
    if name.trim().is_empty() {
        return Some("A sheet name cannot be blank.".into());
    }
    if name.len() > 31 {
        return Some("A sheet name can have at most 31 characters.".into());
    }
    if name.chars().any(|c| INVALID_SHEET_CHARS.contains(&c)) {
        return Some("A sheet name cannot contain \\ / ? * [ ] or :".into());
    }
    if name.starts_with('\'') || name.ends_with('\'') {
        return Some("A sheet name cannot start or end with an apostrophe.".into());
    }
    if name.eq_ignore_ascii_case("history") {
        return Some("\"History\" is reserved by Excel.".into());
    }
    let lower = name.to_ascii_lowercase();
    for (i, sh) in host.sheets.iter().enumerate() {
        if except_index == Some(i) {
            continue;
        }
        if sh.name.to_ascii_lowercase() == lower {
            return Some("That name is already taken.".into());
        }
    }
    None
}

fn next_sheet_id(host: &XlsxHost) -> u32 {
    host.sheets
        .iter()
        .map(|s| s.sheet_id)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn next_sheet_name(host: &XlsxHost, base: &str) -> String {
    let mut n = host.sheets.len() + 1;
    loop {
        let name = format!("{base}{n}");
        if validate_sheet_name(host, &name, None).is_none() {
            return name;
        }
        n += 1;
    }
}

fn empty_worksheet(name: String, sheet_id: u32) -> ActiveWorksheet {
    ActiveWorksheet {
        name,
        sheet_id,
        state: SheetState::Visible,
        part_name: None,
        default_row_height: 15.0,
        default_col_width: None,
        format: None,
        view: default_sheet_view(),
        columns: Vec::new(),
        merges: Vec::new(),
        row_info: BTreeMap::new(),
        cells: Vec::new(),
        data_validations: Vec::new(),
        comment_addresses: Vec::new(),
        hyperlinks: Vec::new(),
        ext_lst: Vec::new(),
        sparkline_groups: Vec::new(),
        drawings: Vec::new(),
        conditional_formats: Vec::new(),
        dimension: None,
    }
}

fn stub_from_ws(ws: &ActiveWorksheet) -> SheetStub {
    SheetStub {
        name: ws.name.clone(),
        sheet_id: ws.sheet_id,
        state: ws.state,
        part_name: ws.part_name.clone(),
        page_setup: None,
    }
}

fn sync_workbook_stubs(host: &mut XlsxHost) {
    host.workbook.sheets = host.sheets.iter().map(stub_from_ws).collect();
}

/// Remap sheet-scoped defined names after sheet order changes (`undefined` → drop).
fn remap_defined_names(host: &mut XlsxHost, map: impl Fn(u32) -> Option<u32>) {
    let mut kept = Vec::with_capacity(host.workbook.defined_names.len());
    for mut name in host.workbook.defined_names.drain(..) {
        if let Some(local) = name.local_sheet {
            match map(local) {
                Some(next) => {
                    name.local_sheet = Some(next);
                    kept.push(name);
                }
                None => { /* drop sheet-scoped name */ }
            }
        } else {
            kept.push(name);
        }
    }
    host.workbook.defined_names = kept;
}

fn visible_count(host: &XlsxHost) -> usize {
    host.sheets
        .iter()
        .filter(|s| s.state == SheetState::Visible)
        .count()
}

fn nearest_visible(host: &XlsxHost, index: usize) -> u32 {
    for i in index..host.sheets.len() {
        if host.sheets[i].state == SheetState::Visible {
            return i as u32;
        }
    }
    for i in (0..index).rev() {
        if host.sheets[i].state == SheetState::Visible {
            return i as u32;
        }
    }
    0
}

/// Adds an empty sheet (appended by default), makes it active, returns its index.
pub fn add_sheet(
    host: &mut XlsxHost,
    name: Option<&str>,
    at: Option<u32>,
) -> Result<u32, EditError> {
    assert_unlocked(host)?;
    let sheet_name = match name {
        Some(n) => n.to_owned(),
        None => next_sheet_name(host, "Sheet"),
    };
    if let Some(problem) = validate_sheet_name(host, &sheet_name, None) {
        return Err(EditError::Other { message: problem });
    }
    let index = {
        let len = host.sheets.len();
        let at = at.map(|a| a as usize).unwrap_or(len);
        at.min(len)
    };
    let ws = empty_worksheet(sheet_name, next_sheet_id(host));
    host.sheets.insert(index, ws);
    sync_workbook_stubs(host);
    remap_defined_names(host, |i| {
        let i = i as usize;
        Some(if i >= index { (i + 1) as u32 } else { i as u32 })
    });
    // Retarget touched cells whose sheet index shifted.
    let shifted: Vec<_> = host
        .touched
        .iter()
        .copied()
        .map(|(s, r, c)| {
            if s as usize >= index {
                (s + 1, r, c)
            } else {
                (s, r, c)
            }
        })
        .collect();
    host.touched = shifted.into_iter().collect();
    host.remap_sheet_indexes(|i| Some(if (i as usize) >= index { i + 1 } else { i }));
    host.mark_structure_dirty();
    host.dirty_sheets.insert(index as u32);
    host.workbook.active_sheet = index as u32;
    Ok(index as u32)
}

/// Deletes a sheet. Sheet-scoped names go with it.
///
/// TODO: rewrite formulas referencing the deleted sheet to `#REF!` once formula transform lands.
pub fn delete_sheet(host: &mut XlsxHost, index: u32) -> Result<(), EditError> {
    assert_unlocked(host)?;
    let idx = index as usize;
    let sheet = host.sheets.get(idx).ok_or_else(|| EditError::Other {
        message: format!("No sheet at index {index}"),
    })?;
    if sheet.state == SheetState::Visible && visible_count(host) <= 1 {
        return Err(EditError::Other {
            message: "A workbook must contain at least one visible sheet.".into(),
        });
    }
    host.sheets.remove(idx);
    sync_workbook_stubs(host);
    remap_defined_names(host, |i| {
        let i = i as usize;
        if i == idx {
            None
        } else if i > idx {
            Some((i - 1) as u32)
        } else {
            Some(i as u32)
        }
    });
    let shifted: Vec<_> = host
        .touched
        .iter()
        .copied()
        .filter_map(|(s, r, c)| {
            let si = s as usize;
            if si == idx {
                None
            } else if si > idx {
                Some((s - 1, r, c))
            } else {
                Some((s, r, c))
            }
        })
        .collect();
    host.touched = shifted.into_iter().collect();
    host.remap_sheet_indexes(|i| {
        let i = i as usize;
        if i == idx {
            None
        } else if i > idx {
            Some((i - 1) as u32)
        } else {
            Some(i as u32)
        }
    });
    host.mark_structure_dirty();
    let active = host.workbook.active_sheet as usize;
    if active > idx {
        host.workbook.active_sheet -= 1;
    } else if active == idx {
        host.workbook.active_sheet =
            nearest_visible(host, idx.min(host.sheets.len().saturating_sub(1)));
    }
    Ok(())
}

/// Renames a sheet.
///
/// TODO: rewrite formula / chart / hyperlink sheet refs once formula transform lands.
pub fn rename_sheet(host: &mut XlsxHost, index: u32, name: &str) -> Result<(), EditError> {
    assert_unlocked(host)?;
    let idx = index as usize;
    let sheet = host.sheets.get(idx).ok_or_else(|| EditError::Other {
        message: format!("No sheet at index {index}"),
    })?;
    if sheet.name == name {
        return Ok(());
    }
    if let Some(problem) = validate_sheet_name(host, name, Some(idx)) {
        return Err(EditError::Other { message: problem });
    }
    host.sheets[idx].name = name.to_owned();
    sync_workbook_stubs(host);
    host.mark_structure_dirty();
    host.dirty_sheets.insert(index);
    Ok(())
}

/// Moves the sheet at `from` so it ends up at index `to`.
pub fn move_sheet(host: &mut XlsxHost, from: u32, to: u32) -> Result<(), EditError> {
    assert_unlocked(host)?;
    let from_i = from as usize;
    if from_i >= host.sheets.len() {
        return Err(EditError::Other {
            message: format!("No sheet at index {from}"),
        });
    }
    let target = (to as usize).min(host.sheets.len().saturating_sub(1));
    if target == from_i {
        return Ok(());
    }
    let mut order: Vec<usize> = (0..host.sheets.len()).collect();
    let moved = order.remove(from_i);
    order.insert(target, moved);
    let sheets: Vec<_> = order.iter().map(|&i| host.sheets[i].clone()).collect();
    host.sheets = sheets;
    sync_workbook_stubs(host);
    let new_index =
        |old: u32| -> u32 { order.iter().position(|&i| i == old as usize).unwrap_or(0) as u32 };
    remap_defined_names(host, |i| Some(new_index(i)));
    let shifted: Vec<_> = host
        .touched
        .iter()
        .copied()
        .map(|(s, r, c)| (new_index(s), r, c))
        .collect();
    host.touched = shifted.into_iter().collect();
    host.remap_sheet_indexes(|i| Some(new_index(i)));
    host.mark_structure_dirty();
    host.workbook.active_sheet = new_index(host.workbook.active_sheet);
    Ok(())
}

fn props_key(info: &ColumnInfo) -> String {
    // Match upstream: properties without min/max, sorted entries, undefined omitted.
    let mut parts: Vec<String> = Vec::new();
    if let Some(w) = info.width {
        parts.push(format!("[\"width\",{w}]"));
    }
    if info.custom_width == Some(true) {
        parts.push("[\"customWidth\",true]".into());
    }
    if info.hidden == Some(true) {
        parts.push("[\"hidden\",true]".into());
    }
    if let Some(s) = info.style_id {
        parts.push(format!("[\"styleId\",{s}]"));
    }
    if let Some(o) = info.outline_level {
        parts.push(format!("[\"outlineLevel\",{o}]"));
    }
    if info.collapsed == Some(true) {
        parts.push("[\"collapsed\",true]".into());
    }
    if info.best_fit == Some(true) {
        parts.push("[\"bestFit\",true]".into());
    }
    parts.sort();
    format!("[{}]", parts.join(","))
}

fn has_props(info: &ColumnInfo) -> bool {
    props_key(info) != "[]"
}

/// Sorts entries, drops empty ones and joins adjacent entries with equal properties.
pub fn normalize_columns(columns: Vec<ColumnInfo>) -> Vec<ColumnInfo> {
    let mut sorted: Vec<ColumnInfo> = columns.into_iter().filter(has_props).collect();
    sorted.sort_by_key(|c| c.min);
    let mut out: Vec<ColumnInfo> = Vec::new();
    for info in sorted {
        if let Some(last) = out.last_mut() {
            if last.max + 1 == info.min && props_key(last) == props_key(&info) {
                last.max = info.max;
                continue;
            }
        }
        out.push(info);
    }
    out
}

/// Splits column spans, edits each target column, then normalizes.
pub fn edit_columns(
    sheet: &mut ActiveWorksheet,
    cols: &[u32],
    mut edit: impl FnMut(&mut ColumnInfo),
) {
    let targets: HashSet<u32> = cols.iter().copied().collect();
    let mut singles: HashMap<u32, ColumnInfo> = HashMap::new();
    let mut kept: Vec<ColumnInfo> = Vec::new();
    for info in &sheet.columns {
        let mut run_start = info.min;
        let end = info.max + 1;
        for c in info.min..=end {
            let is_target = c <= info.max && targets.contains(&c);
            if c > info.max || is_target {
                if c > run_start {
                    kept.push(ColumnInfo {
                        min: run_start,
                        max: c - 1,
                        ..info.clone()
                    });
                }
                run_start = c + 1;
            }
            if is_target {
                singles.insert(
                    c,
                    ColumnInfo {
                        min: c,
                        max: c,
                        ..info.clone()
                    },
                );
            }
        }
    }
    for &c in &targets {
        let mut info = singles.remove(&c).unwrap_or(ColumnInfo {
            min: c,
            max: c,
            width: None,
            custom_width: None,
            hidden: None,
            style_id: None,
            outline_level: None,
            collapsed: None,
            best_fit: None,
        });
        edit(&mut info);
        singles.insert(c, info);
    }
    let mut combined = kept;
    combined.extend(singles.into_values());
    sheet.columns = normalize_columns(combined);
}

/// Sets column widths in characters (`None` clears custom width).
pub fn set_column_width(
    host: &mut XlsxHost,
    sheet: u32,
    cols: &[u32],
    width: Option<f64>,
) -> Result<(), EditError> {
    {
        let sh = host
            .sheet_mut(sheet)
            .map_err(|e| EditError::Other { message: e })?;
        if cols.is_empty() {
            return Ok(());
        }
        let widths: HashMap<u32, Option<f64>> = cols
            .iter()
            .map(|&c| (c, width.map(|w| w.clamp(0.0, MAX_WIDTH))))
            .collect();
        edit_columns(sh, cols, |info| {
            let next = widths.get(&info.min).copied().flatten();
            match next {
                None => {
                    info.width = None;
                    info.custom_width = None;
                    info.best_fit = None;
                }
                Some(w) => {
                    info.width = Some(w);
                    info.custom_width = Some(true);
                    info.best_fit = None;
                    if w == 0.0 {
                        info.hidden = Some(true);
                    } else {
                        info.hidden = None;
                    }
                }
            }
        });
    }
    host.dirty_sheets.insert(sheet);
    Ok(())
}

/// Sets row heights in points.
pub fn set_row_height(
    host: &mut XlsxHost,
    sheet: u32,
    rows: &[u32],
    height: f64,
) -> Result<(), EditError> {
    {
        let sh = host
            .sheet_mut(sheet)
            .map_err(|e| EditError::Other { message: e })?;
        if rows.is_empty() {
            return Ok(());
        }
        let h = height.clamp(0.0, MAX_HEIGHT);
        for &row in rows {
            let mut info = sh.row_info.get(&row).cloned().unwrap_or_default();
            info.height = Some(h);
            info.custom_height = Some(true);
            if h == 0.0 {
                info.hidden = Some(true);
            } else {
                info.hidden = None;
            }
            if info == RowInfo::default() {
                sh.row_info.remove(&row);
            } else {
                sh.row_info.insert(row, info);
            }
        }
    }
    host.dirty_sheets.insert(sheet);
    Ok(())
}

/// Hide or unhide rows or columns.
pub fn set_hidden(
    host: &mut XlsxHost,
    sheet: u32,
    axis: &str,
    indices: &[u32],
    hidden: bool,
) -> Result<(), EditError> {
    {
        let sh = host
            .sheet_mut(sheet)
            .map_err(|e| EditError::Other { message: e })?;
        if indices.is_empty() {
            return Ok(());
        }
        match axis {
            "col" => {
                edit_columns(sh, indices, |info| {
                    if hidden {
                        info.hidden = Some(true);
                    } else {
                        info.hidden = None;
                        if info.width == Some(0.0) {
                            info.width = None;
                        }
                    }
                });
            }
            "row" => {
                for &row in indices {
                    let mut info = sh.row_info.get(&row).cloned().unwrap_or_default();
                    if hidden {
                        info.hidden = Some(true);
                    } else {
                        info.hidden = None;
                        if info.height == Some(0.0) {
                            info.height = None;
                        }
                    }
                    if info == RowInfo::default() {
                        sh.row_info.remove(&row);
                    } else {
                        sh.row_info.insert(row, info);
                    }
                }
            }
            other => {
                return Err(EditError::Other {
                    message: format!("setHidden axis must be row|col, got {other}"),
                });
            }
        }
    }
    host.dirty_sheets.insert(sheet);
    Ok(())
}

/// Register `addSheet`, `renameSheet`, and `deleteSheet` runners.
pub fn register_structure_commands(registry: &mut CommandRegistry) {
    registry.register(
        Command::stub("addSheet", "addSheet").with_runner(|host, args| {
            let h = host_mut(host)?;
            let name = args.get("name").and_then(|v| v.as_str()).map(str::to_owned);
            let at = args.get("at").and_then(|v| v.as_u64()).map(|n| n as u32);
            let index = add_sheet(h, name.as_deref(), at)?;
            Ok(ChangeSet::new(ChangeKind::Sheets, "addSheet").with_sheet(index))
        }),
    );
    registry.register(
        Command::stub("renameSheet", "renameSheet").with_runner(|host, args| {
            let h = host_mut(host)?;
            let index = arg_u32(args, "index")?;
            let name =
                args.get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EditError::Other {
                        message: "renameSheet missing name".into(),
                    })?;
            rename_sheet(h, index, name)?;
            Ok(ChangeSet::new(ChangeKind::Sheets, "renameSheet").with_sheet(index))
        }),
    );
    registry.register(
        Command::stub("deleteSheet", "deleteSheet").with_runner(|host, args| {
            let h = host_mut(host)?;
            let index = arg_u32(args, "index")?;
            delete_sheet(h, index)?;
            Ok(ChangeSet::new(ChangeKind::Sheets, "deleteSheet"))
        }),
    );
}
