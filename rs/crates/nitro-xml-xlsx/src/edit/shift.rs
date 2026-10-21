// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/shift-formula.ts,
// src/core/xlsx/edit/{structure,range-math,shift-sheet,shift-formulas,insert-guard}.ts;
// changes listed in PROVENANCE.md.
//! Insert and delete rows and columns, shifting cells and formula references (#76).
//!
//! E1 subset of the upstream sheet shift: cells (including array ranges and spill
//! anchors), merges, column spans, row info, freeze panes, selection, top-left,
//! hyperlinks, comments, and data-validation ranges. Defined names and every
//! sheet's cell formulas are rewritten with the #71 reference model.
//!
//! TODO(#76): tables, drawings, charts, auto-filter, and print area are not
//! shifted (not on [`ActiveWorksheet`]). Conditional-format and other carried
//! `sqref` markup is rewritten on save from [`XlsxHost::axis_shifts`].

use std::any::Any;
use std::collections::{BTreeMap, HashMap, HashSet};

use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry, EditError};

use crate::address::{format_range, parse_range, MAX_COL, MAX_ROW};
use crate::formula::{rewrite_ref, same_spec, tokenize, RefKind, RefSpec, Token, TokenKind};
use crate::model::{
    ActiveWorksheet, CellAddress, CellRange, ColumnInfo, DumpCell, DumpCellValue, FreezePane,
    RowInfo, SheetSelection, SheetView,
};

use super::host::{AxisShiftRecord, XlsxHost};
use super::structure::normalize_columns;

/// Row or column axis, matching upstream `Axis`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Row,
    Col,
}

impl Axis {
    fn as_str(self) -> &'static str {
        match self {
            Self::Row => "row",
            Self::Col => "col",
        }
    }
}

/// An axis shift: positive `count` inserts before `at`, negative deletes `-count` from `at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisShift {
    pub axis: Axis,
    pub at: u32,
    pub count: i32,
}

/// Formula rewrite target. `count` is positive to insert and negative to delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShiftSpec {
    pub sheet: String,
    pub axis: Axis,
    pub at: i32,
    pub count: i32,
}

/// Debug switches for the shift path. Production commands leave formulas rewriting on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShiftFlags {
    /// When false, cells move but formula text is left unchanged (negative control).
    pub rewrite_formulas: bool,
}

impl Default for ShiftFlags {
    fn default() -> Self {
        Self {
            rewrite_formulas: true,
        }
    }
}

fn axis_max(axis: Axis) -> u32 {
    match axis {
        Axis::Row => MAX_ROW,
        Axis::Col => MAX_COL,
    }
}

fn host_mut(host: &mut dyn Any) -> Result<&mut XlsxHost, EditError> {
    host.downcast_mut::<XlsxHost>()
        .ok_or_else(|| EditError::Other {
            message: "expected XlsxHost".into(),
        })
}

fn arg_u32(args: &serde_json::Value, key: &str) -> Result<u32, EditError> {
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

/// Validates an inclusive axis span before the edit runs.
fn check_span(axis: Axis, at: u32, count: u32) -> Result<(), EditError> {
    let max = axis_max(axis) as u64;
    let span = at as u64 + count as u64;
    if count < 1 || at as u64 > max || span > max + 1 {
        return Err(EditError::Other {
            message: format!("Invalid {} span {at} + {count}", axis.as_str()),
        });
    }
    Ok(())
}

/// Shifts one coordinate interval for a formula reference.
/// `None` when the interval is deleted entirely.
fn shift_interval(a: i32, b: i32, at: i32, count: i32, max: i32) -> Option<(i32, i32)> {
    if count > 0 {
        let na = if a >= at { a + count } else { a };
        let nb = if b >= at { b + count } else { b };
        if nb > max {
            return if na > max { None } else { Some((na, max)) };
        }
        return Some((na, nb));
    }
    let n = -count;
    let last = at + n - 1;
    if a >= at && b <= last {
        return None;
    }
    let na = if a < at {
        a
    } else if a <= last {
        at
    } else {
        a - n
    };
    let nb = if b < at {
        b
    } else if b <= last {
        at - 1
    } else {
        b - n
    };
    Some((na, nb))
}

/// A reference moved for the shift. `None` when it is deleted entirely.
fn shift_ref(reference: &RefSpec, spec: &ShiftSpec) -> Option<RefSpec> {
    let is_row = spec.axis == Axis::Row;
    if (is_row && reference.kind == RefKind::Cols) || (!is_row && reference.kind == RefKind::Rows) {
        return Some(*reference);
    }
    let start_first = if is_row {
        reference.start.row <= reference.end.row
    } else {
        reference.start.col <= reference.end.col
    };
    let lo = if start_first {
        reference.start
    } else {
        reference.end
    };
    let hi = if start_first {
        reference.end
    } else {
        reference.start
    };
    let max = axis_max(spec.axis) as i32;
    let (a, b) = shift_interval(
        if is_row { lo.row } else { lo.col },
        if is_row { hi.row } else { hi.col },
        spec.at,
        spec.count,
        max,
    )?;
    let mut nlo = lo;
    let mut nhi = hi;
    if is_row {
        nlo.row = a;
        nhi.row = b;
    } else {
        nlo.col = a;
        nhi.col = b;
    }
    let mut next = RefSpec {
        kind: reference.kind,
        start: if start_first { nlo } else { nhi },
        end: if start_first { nhi } else { nlo },
    };
    if reference.kind == RefKind::Cell {
        next.end = next.start;
    }
    Some(next)
}

fn plain_cell(token: &Token) -> bool {
    token.kind == TokenKind::Ref
        && token
            .reference
            .is_some_and(|reference| reference.kind == RefKind::Cell)
        && !token.spill
}

/// Joins `A1 : A5` (spaces around the colon) into one area, the way Excel reads it
/// when rows or columns shift.
fn merge_spaced_areas(tokens: Vec<Token>) -> Vec<Token> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let mut j = i + 1;
        while tokens.get(j).is_some_and(|t| t.kind == TokenKind::Ws) {
            j += 1;
        }
        let mut k = j + 1;
        while tokens.get(k).is_some_and(|t| t.kind == TokenKind::Ws) {
            k += 1;
        }
        let colon = tokens
            .get(j)
            .is_some_and(|t| t.kind == TokenKind::Op && t.text == ":");
        let last_plain = tokens
            .get(k)
            .is_some_and(|t| plain_cell(t) && t.prefix.is_none());
        if plain_cell(&tokens[i]) && colon && last_plain && k > i + 2 {
            if let (Some(first), Some(last)) = (tokens[i].reference, tokens[k].reference) {
                let text: String = tokens[i..=k].iter().map(|t| t.text.as_str()).collect();
                let mut merged = tokens[i].clone();
                merged.text = text;
                merged.reference = Some(RefSpec {
                    kind: RefKind::Area,
                    start: first.start,
                    end: last.start,
                });
                out.push(merged);
                i = k + 1;
                continue;
            }
        }
        out.push(tokens[i].clone());
        i += 1;
    }
    out
}

/// Adjusts references for inserted (`count` > 0) or deleted (`count` < 0) rows or
/// columns on `spec.sheet`. References into deleted cells become `#REF!`.
pub fn shift_formula(formula: &str, formula_sheet: &str, spec: &ShiftSpec) -> String {
    if spec.count == 0 || formula.is_empty() {
        return formula.to_owned();
    }
    let Ok(raw) = tokenize(formula) else {
        return formula.to_owned();
    };
    let sheet = spec.sheet.to_lowercase();
    let own = formula_sheet.to_lowercase() == sheet;
    let mut out = String::with_capacity(formula.len());
    for token in merge_spaced_areas(raw) {
        if token.kind != TokenKind::Ref {
            out.push_str(&token.text);
            continue;
        }
        let applies = match &token.prefix {
            Some(prefix) if prefix.book.is_none() => prefix.sheet.to_lowercase() == sheet,
            Some(_) => false,
            None => own,
        };
        let Some(reference) = token.reference else {
            out.push_str(&token.text);
            continue;
        };
        if !applies {
            out.push_str(&token.text);
            continue;
        }
        let next = shift_ref(&reference, spec);
        match next {
            Some(next) if same_spec(&next, &reference) => out.push_str(&token.text),
            other => {
                let prefix = token.prefix.as_ref().map(|p| p.text.as_str()).unwrap_or("");
                out.push_str(&rewrite_ref(prefix, token.spill, other.as_ref()));
            }
        }
    }
    out
}

/// Shifts a one-dimensional inclusive span. A span deleted entirely returns `None`.
fn shift_span(start: i32, end: i32, shift: &AxisShift) -> Option<(i32, i32)> {
    let at = shift.at as i32;
    let count = shift.count;
    let max = axis_max(shift.axis) as i32;
    if count > 0 {
        if start >= at {
            if start + count > max {
                return None;
            }
            return Some((start + count, (end + count).min(max)));
        }
        if end >= at {
            return Some((start, (end + count).min(max)));
        }
        return Some((start, end));
    }
    let n = -count;
    let last = at + n - 1;
    if start >= at && end <= last {
        return None;
    }
    let s = if start < at {
        start
    } else if start > last {
        start - n
    } else {
        at
    };
    // A span reaching the grid edge (a whole row or column) keeps reaching it.
    let e = if end >= max {
        max
    } else if end < at {
        end
    } else if end > last {
        end - n
    } else {
        at - 1
    };
    if s > e {
        None
    } else {
        Some((s, e))
    }
}

fn shift_index(index: u32, shift: &AxisShift) -> Option<u32> {
    if index < shift.at {
        return Some(index);
    }
    let max = axis_max(shift.axis) as i64;
    let moved = index as i64 + shift.count as i64;
    if shift.count > 0 {
        return if moved > max {
            None
        } else {
            Some(moved as u32)
        };
    }
    let threshold = shift.at as i64 - shift.count as i64;
    if (index as i64) < threshold || moved < 0 {
        None
    } else {
        Some(moved as u32)
    }
}

fn shift_index_clamped(index: u32, shift: &AxisShift) -> u32 {
    if let Some(moved) = shift_index(index, shift) {
        return moved;
    }
    if shift.count > 0 {
        axis_max(shift.axis)
    } else {
        shift.at
    }
}

fn shift_range(range: &CellRange, shift: &AxisShift) -> Option<CellRange> {
    if shift.axis == Axis::Row {
        let (start, end) = shift_span(range.start.row as i32, range.end.row as i32, shift)?;
        Some(CellRange {
            start: CellAddress {
                row: start as u32,
                col: range.start.col,
            },
            end: CellAddress {
                row: end as u32,
                col: range.end.col,
            },
        })
    } else {
        let (start, end) = shift_span(range.start.col as i32, range.end.col as i32, shift)?;
        Some(CellRange {
            start: CellAddress {
                row: range.start.row,
                col: start as u32,
            },
            end: CellAddress {
                row: range.end.row,
                col: end as u32,
            },
        })
    }
}

fn shift_address(address: CellAddress, shift: &AxisShift) -> Option<CellAddress> {
    match shift.axis {
        Axis::Row => shift_index(address.row, shift).map(|row| CellAddress {
            row,
            col: address.col,
        }),
        Axis::Col => shift_index(address.col, shift).map(|col| CellAddress {
            row: address.row,
            col,
        }),
    }
}

fn shift_address_clamped(address: CellAddress, shift: &AxisShift) -> CellAddress {
    match shift.axis {
        Axis::Row => CellAddress {
            row: shift_index_clamped(address.row, shift),
            col: address.col,
        },
        Axis::Col => CellAddress {
            row: address.row,
            col: shift_index_clamped(address.col, shift),
        },
    }
}

fn ranges_intersect(a: &CellRange, b: &CellRange) -> bool {
    a.start.row <= b.end.row
        && b.start.row <= a.end.row
        && a.start.col <= b.end.col
        && b.start.col <= a.end.col
}

fn point_in_range(row: u32, col: u32, range: &CellRange) -> bool {
    row >= range.start.row && row <= range.end.row && col >= range.start.col && col <= range.end.col
}

fn cell_blocks_insert(cell: &DumpCell) -> bool {
    // `styleId` 0 is falsy in the TS guard (`!cell.styleId`).
    let has_style = cell.style_id.is_some_and(|id| id != 0);
    let empty = matches!(cell.value, DumpCellValue::Null)
        && cell.formula.is_none()
        && !has_style
        && cell.rich_text.is_none();
    !empty || cell.cell_metadata.is_some() || cell.value_metadata.is_some()
}

fn row_info_nonempty(info: &RowInfo) -> bool {
    info.height.is_some()
        || info.custom_height.is_some()
        || info.hidden.is_some()
        || info.style_id.is_some()
        || info.outline_level.is_some()
        || info.collapsed.is_some()
}

fn pushed_off() -> EditError {
    EditError::Other {
        message: "Cannot insert cells: non-empty cells would be pushed off the sheet.".into(),
    }
}

/// Refuses an insertion that would discard cells or clip positioned metadata at the grid edge.
fn assert_insert_fits(sheet: &ActiveWorksheet, shift: &AxisShift) -> Result<(), EditError> {
    if shift.count <= 0 {
        return Ok(());
    }
    let max = axis_max(shift.axis);
    let count = shift.count as u32;
    if shift.at as u64 + count as u64 > max as u64 + 1 {
        return Err(EditError::Other {
            message: "The insert area extends beyond the sheet.".into(),
        });
    }
    let first = max - count + 1;
    let lost = if shift.axis == Axis::Row {
        CellRange {
            start: CellAddress { row: first, col: 0 },
            end: CellAddress {
                row: MAX_ROW,
                col: MAX_COL,
            },
        }
    } else {
        CellRange {
            start: CellAddress { row: 0, col: first },
            end: CellAddress {
                row: MAX_ROW,
                col: MAX_COL,
            },
        }
    };
    for cell in &sheet.cells {
        if point_in_range(cell.row, cell.col, &lost) && cell_blocks_insert(cell) {
            return Err(pushed_off());
        }
    }
    if sheet
        .comment_addresses
        .iter()
        .any(|c| point_in_range(c.row, c.col, &lost))
        || sheet
            .hyperlinks
            .iter()
            .any(|h| ranges_intersect(&h.range, &lost))
        || sheet.merges.iter().any(|m| ranges_intersect(m, &lost))
        || sheet
            .data_validations
            .iter()
            .any(|rule| rule.ranges.iter().any(|r| ranges_intersect(r, &lost)))
    {
        return Err(pushed_off());
    }
    // TODO(#76): drawings, tables, conditional formats, auto-filter, print area.
    if shift.axis == Axis::Row {
        if sheet
            .row_info
            .iter()
            .any(|(row, info)| *row >= first && row_info_nonempty(info))
        {
            return Err(pushed_off());
        }
    } else if normalize_columns(sheet.columns.clone())
        .iter()
        .any(|c| c.min >= first)
    {
        return Err(pushed_off());
    }
    Ok(())
}

fn blank_styled(row: u32, col: u32, style_id: u32) -> DumpCell {
    DumpCell {
        row,
        col,
        value: DumpCellValue::Null,
        type_tag: None,
        formula: None,
        style_id: Some(style_id),
        rich_text: None,
        array_range: None,
        legacy_formula: None,
        dynamic_array: None,
        spill_anchor: None,
        cell_metadata: None,
        value_metadata: None,
    }
}

fn shift_cells(sheet: &mut ActiveWorksheet, shift: &AxisShift) {
    let old = std::mem::take(&mut sheet.cells);
    let mut placed: HashMap<(u32, u32), DumpCell> = HashMap::with_capacity(old.len());
    for mut cell in old {
        let along = match shift.axis {
            Axis::Row => cell.row,
            Axis::Col => cell.col,
        };
        let Some(moved) = shift_index(along, shift) else {
            continue;
        };
        if let Some(range) = cell.array_range.clone() {
            cell.array_range = shift_range(&range, shift);
        }
        if let Some(anchor) = cell.spill_anchor {
            cell.spill_anchor = shift_address(anchor, shift);
        }
        match shift.axis {
            Axis::Row => cell.row = moved,
            Axis::Col => cell.col = moved,
        }
        placed.insert((cell.row, cell.col), cell);
    }
    let mut cells: Vec<DumpCell> = placed.into_values().collect();
    cells.sort_by_key(|c| (c.row, c.col));
    sheet.replace_cells(cells);
}

fn single_cell(range: &CellRange) -> bool {
    range.start.row == range.end.row && range.start.col == range.end.col
}

fn shift_row_info(info: &BTreeMap<u32, RowInfo>, shift: &AxisShift) -> BTreeMap<u32, RowInfo> {
    let mut out = BTreeMap::new();
    for (row, info) in info {
        if let Some(moved) = shift_index(*row, shift) {
            out.insert(moved, info.clone());
        }
    }
    out
}

fn shift_columns(columns: &[ColumnInfo], shift: &AxisShift) -> Vec<ColumnInfo> {
    let mut out = Vec::new();
    for info in columns {
        if let Some((start, end)) = shift_span(info.min as i32, info.max as i32, shift) {
            out.push(ColumnInfo {
                min: start as u32,
                max: end as u32,
                ..info.clone()
            });
        }
    }
    normalize_columns(out)
}

fn shift_freeze(freeze: &mut FreezePane, shift: &AxisShift) {
    let frozen = match shift.axis {
        Axis::Row => freeze.rows,
        Axis::Col => freeze.cols,
    };
    if shift.at >= frozen {
        return;
    }
    let next = if shift.count > 0 {
        frozen as i64 + shift.count as i64
    } else {
        let removed = (-(shift.count as i64)).min(frozen as i64 - shift.at as i64);
        frozen as i64 - removed
    };
    let next = next.max(0) as u32;
    match shift.axis {
        Axis::Row => freeze.rows = next,
        Axis::Col => freeze.cols = next,
    }
}

fn shift_selection(selection: &mut SheetSelection, shift: &AxisShift) {
    selection.active = shift_address_clamped(selection.active, shift);
    selection.anchor = shift_address_clamped(selection.anchor, shift);
    let mut ranges = Vec::new();
    for range in &selection.ranges {
        if let Some(moved) = shift_range(range, shift) {
            ranges.push(moved);
        }
    }
    if ranges.is_empty() {
        ranges.push(CellRange {
            start: selection.active,
            end: selection.active,
        });
    }
    selection.ranges = ranges;
}

fn shift_view(view: &mut SheetView, shift: &AxisShift) {
    if let Some(freeze) = view.freeze.as_mut() {
        shift_freeze(freeze, shift);
    }
    if let Some(top) = view.top_left {
        view.top_left = Some(shift_address_clamped(top, shift));
    }
    if let Some(selection) = view.selection.as_mut() {
        shift_selection(selection, shift);
    }
}

fn shift_sheet_content(sheet: &mut ActiveWorksheet, shift: &AxisShift) -> Result<(), EditError> {
    assert_insert_fits(sheet, shift)?;
    shift_cells(sheet, shift);
    sheet.merges = sheet
        .merges
        .iter()
        .filter_map(|m| shift_range(m, shift))
        .filter(|m| !single_cell(m))
        .collect();
    sheet.hyperlinks = sheet
        .hyperlinks
        .iter()
        .filter_map(|link| {
            shift_range(&link.range, shift).map(|range| {
                let mut next = link.clone();
                next.range = range;
                next
            })
        })
        .collect();
    sheet.comment_addresses = sheet
        .comment_addresses
        .iter()
        .filter_map(|address| shift_address(*address, shift))
        .collect();
    sheet.data_validations = sheet
        .data_validations
        .iter()
        .filter_map(|rule| {
            let ranges: Vec<CellRange> = rule
                .ranges
                .iter()
                .filter_map(|range| shift_range(range, shift))
                .collect();
            if ranges.is_empty() {
                None
            } else {
                let mut next = rule.clone();
                next.ranges = ranges;
                Some(next)
            }
        })
        .collect();
    match shift.axis {
        Axis::Row => {
            let shifted = shift_row_info(&sheet.row_info, shift);
            sheet.row_info = shifted;
        }
        Axis::Col => {
            sheet.columns = shift_columns(&sheet.columns, shift);
        }
    }
    shift_view(&mut sheet.view, shift);
    Ok(())
}

fn location_is_formula(location: &str) -> bool {
    let bytes = location.as_bytes();
    bytes
        .windows(2)
        .any(|pair| pair[0] == b'!' && (pair[1].is_ascii_alphabetic() || pair[1] == b'$'))
}

fn rewrite_formulas(host: &mut XlsxHost, spec: &ShiftSpec) {
    let sheet_names: Vec<String> = host.sheets.iter().map(|s| s.name.clone()).collect();
    for (index, sheet) in host.sheets.iter_mut().enumerate() {
        let name = &sheet_names[index];
        for cell in &mut sheet.cells {
            if let Some(formula) = &cell.formula {
                let next = shift_formula(formula, name, spec);
                if next != *formula {
                    cell.formula = Some(next);
                }
            }
        }
        for link in &mut sheet.hyperlinks {
            if let Some(location) = &link.location {
                if location_is_formula(location) {
                    link.location = Some(shift_formula(location, name, spec));
                }
            }
        }
    }
    for defined in &mut host.workbook.defined_names {
        let formula_sheet = defined
            .local_sheet
            .and_then(|idx| sheet_names.get(idx as usize))
            .map(|s| s.as_str())
            .unwrap_or("");
        defined.formula = shift_formula(&defined.formula, formula_sheet, spec);
    }
}

/// Inserted rows and columns take the format of the row above or the column to the left.
fn inherit_formats(sheet: &mut ActiveWorksheet, axis: Axis, at: u32, count: u32) {
    if at == 0 || count == 0 {
        return;
    }
    let mut occupied: HashSet<(u32, u32)> = sheet.cells.iter().map(|c| (c.row, c.col)).collect();
    if axis == Axis::Row {
        let above: Vec<(u32, u32)> = sheet
            .cells
            .iter()
            .filter(|c| c.row == at - 1)
            .filter_map(|c| c.style_id.filter(|id| *id != 0).map(|id| (c.col, id)))
            .collect();
        let row_style = sheet
            .row_info
            .get(&(at - 1))
            .and_then(|info| info.style_id)
            .filter(|id| *id != 0);
        for row in at..at + count {
            if let Some(style_id) = row_style {
                sheet.row_info.insert(
                    row,
                    RowInfo {
                        style_id: Some(style_id),
                        ..RowInfo::default()
                    },
                );
            }
            for &(col, style_id) in &above {
                if occupied.contains(&(row, col)) {
                    continue;
                }
                sheet.push_cell(blank_styled(row, col, style_id));
                occupied.insert((row, col));
            }
        }
        return;
    }
    let rows: Vec<u32> = {
        let mut rows: Vec<u32> = sheet.cells.iter().map(|c| c.row).collect();
        rows.sort_unstable();
        rows.dedup();
        rows
    };
    let left: Vec<(u32, u32)> = rows
        .into_iter()
        .filter_map(|row| {
            sheet
                .cells
                .iter()
                .find(|c| c.row == row && c.col == at - 1)
                .and_then(|c| c.style_id.filter(|id| *id != 0))
                .map(|style_id| (row, style_id))
        })
        .collect();
    for &(row, style_id) in &left {
        for col in at..at + count {
            if occupied.contains(&(row, col)) {
                continue;
            }
            sheet.push_cell(blank_styled(row, col, style_id));
            occupied.insert((row, col));
        }
    }
}

fn apply_axis_shift(
    host: &mut XlsxHost,
    sheet_index: u32,
    shift: AxisShift,
    flags: ShiftFlags,
) -> Result<(), EditError> {
    let sheet_name = host.sheet(sheet_index).map_err(sheet_err)?.name.clone();
    assert_insert_fits(host.sheet(sheet_index).map_err(sheet_err)?, &shift)?;
    if flags.rewrite_formulas {
        let spec = ShiftSpec {
            sheet: sheet_name,
            axis: shift.axis,
            at: shift.at as i32,
            count: shift.count,
        };
        rewrite_formulas(host, &spec);
    }
    {
        let sheet = host.sheet_mut(sheet_index).map_err(sheet_err)?;
        shift_sheet_content(sheet, &shift)?;
        if shift.count > 0 {
            inherit_formats(sheet, shift.axis, shift.at, shift.count as u32);
        }
    }
    host.axis_shifts.push(AxisShiftRecord {
        sheet: sheet_index,
        row: shift.axis == Axis::Row,
        at: shift.at,
        count: shift.count,
    });
    Ok(())
}

/// Rewrites `sqref` attributes and `<sqref>` text in one carried worksheet child.
/// `None` when the element's own `sqref` attribute loses every range (the block is dropped).
pub(crate) fn rewrite_carried_sqrefs(xml: &str, shifts: &[AxisShift]) -> Option<String> {
    if shifts.is_empty() || !xml.contains("sqref") {
        return Some(xml.to_owned());
    }
    let rewritten = rewrite_sqrefs(xml, shifts);
    if root_sqref_empty(&rewritten) {
        None
    } else {
        Some(rewritten)
    }
}

fn root_sqref_empty(xml: &str) -> bool {
    let bytes = xml.as_bytes();
    let Some(start) = bytes.iter().position(|b| *b == b'<') else {
        return false;
    };
    let Some(end) = bytes[start..].iter().position(|b| *b == b'>') else {
        return false;
    };
    let tag = &xml[start..start + end];
    tag.contains("sqref=\"\"") || tag.contains("sqref=''")
}

fn rewrite_sqrefs(xml: &str, shifts: &[AxisShift]) -> String {
    let chars: Vec<char> = xml.chars().collect();
    let mut out = String::with_capacity(xml.len());
    let mut i = 0;
    while i < chars.len() {
        if is_sqref_name(&chars, i) {
            let name_end = i + 5;
            let mut j = name_end;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if chars.get(j) == Some(&'=') {
                j += 1;
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                if let Some(quote) = chars.get(j).copied().filter(|c| *c == '"' || *c == '\'') {
                    let value_start = j + 1;
                    let mut k = value_start;
                    while k < chars.len() && chars[k] != quote {
                        k += 1;
                    }
                    let value: String = chars[value_start..k].iter().collect();
                    out.extend(chars[i..value_start].iter());
                    out.push_str(&shift_sqref_value(&value, shifts));
                    i = k;
                    continue;
                }
            } else if chars.get(j) == Some(&'>') && !tag_is_closing(&chars, i) {
                let content_start = j + 1;
                let mut k = content_start;
                while k + 1 < chars.len() && !(chars[k] == '<' && chars[k + 1] == '/') {
                    k += 1;
                }
                let value: String = chars[content_start..k].iter().collect();
                out.extend(chars[i..content_start].iter());
                out.push_str(&shift_sqref_value(value.trim(), shifts));
                i = k;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn is_sqref_name(chars: &[char], i: usize) -> bool {
    let name = ['s', 'q', 'r', 'e', 'f'];
    if i + name.len() > chars.len() || chars[i..i + name.len()] != name {
        return false;
    }
    let prev_ok = i == 0 || !is_name_char(chars[i - 1]);
    let next = chars.get(i + name.len()).copied();
    prev_ok && next.is_none_or(|c| !is_name_char(c))
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'
}

fn tag_is_closing(chars: &[char], name_at: usize) -> bool {
    let mut k = name_at;
    while k > 0 {
        k -= 1;
        if chars[k] == '<' {
            return chars.get(k + 1) == Some(&'/');
        }
        if !(chars[k].is_whitespace() || chars[k] == ':' || chars[k].is_ascii_alphanumeric()) {
            return false;
        }
    }
    false
}

fn shift_sqref_value(value: &str, shifts: &[AxisShift]) -> String {
    let mut changed = false;
    let mut parts = Vec::new();
    for token in value.split_whitespace() {
        match shift_sqref_token(token, shifts) {
            Some(next) => {
                if next != token {
                    changed = true;
                }
                parts.push(next);
            }
            None => changed = true,
        }
    }
    if changed {
        parts.join(" ")
    } else {
        value.to_owned()
    }
}

fn shift_sqref_token(token: &str, shifts: &[AxisShift]) -> Option<String> {
    let Some(mut range) = parse_range(token) else {
        return Some(token.to_owned());
    };
    let original = range.clone();
    for shift in shifts {
        range = shift_range(&range, shift)?;
    }
    if range == original {
        Some(token.to_owned())
    } else {
        Some(format_range(&range))
    }
}

fn shift_axis(
    host: &mut XlsxHost,
    sheet: u32,
    axis: Axis,
    at: u32,
    count: u32,
    insert: bool,
    flags: ShiftFlags,
) -> Result<(), EditError> {
    check_span(axis, at, count)?;
    let signed = if insert {
        count as i32
    } else {
        -(count as i32)
    };
    apply_axis_shift(
        host,
        sheet,
        AxisShift {
            axis,
            at,
            count: signed,
        },
        flags,
    )
}

/// Inserts `count` rows before zero-based row `at`.
pub fn insert_rows(host: &mut XlsxHost, sheet: u32, at: u32, count: u32) -> Result<(), EditError> {
    shift_axis(
        host,
        sheet,
        Axis::Row,
        at,
        count,
        true,
        ShiftFlags::default(),
    )
}

/// Deletes `count` rows starting at zero-based row `at`.
pub fn delete_rows(host: &mut XlsxHost, sheet: u32, at: u32, count: u32) -> Result<(), EditError> {
    shift_axis(
        host,
        sheet,
        Axis::Row,
        at,
        count,
        false,
        ShiftFlags::default(),
    )
}

/// Inserts `count` columns before zero-based column `at`.
pub fn insert_columns(
    host: &mut XlsxHost,
    sheet: u32,
    at: u32,
    count: u32,
) -> Result<(), EditError> {
    shift_axis(
        host,
        sheet,
        Axis::Col,
        at,
        count,
        true,
        ShiftFlags::default(),
    )
}

/// Deletes `count` columns starting at zero-based column `at`.
pub fn delete_columns(
    host: &mut XlsxHost,
    sheet: u32,
    at: u32,
    count: u32,
) -> Result<(), EditError> {
    shift_axis(
        host,
        sheet,
        Axis::Col,
        at,
        count,
        false,
        ShiftFlags::default(),
    )
}

fn register_axis(registry: &mut CommandRegistry, id: &'static str, insert: bool, axis: Axis) {
    registry.register(Command::stub(id, id).with_runner(move |host, args| {
        let h = host_mut(host)?;
        let sheet = arg_u32(args, "sheet")?;
        let at = arg_u32(args, "at")?;
        let count = arg_u32(args, "count")?;
        shift_axis(h, sheet, axis, at, count, insert, ShiftFlags::default())?;
        Ok(ChangeSet::new(ChangeKind::Structure, id)
            .with_sheet(sheet)
            .structural())
    }));
}

/// Register `insertRows`, `deleteRows`, `insertColumns`, and `deleteColumns`.
pub fn register_shift_commands(registry: &mut CommandRegistry) {
    register_axis(registry, "insertRows", true, Axis::Row);
    register_axis(registry, "deleteRows", false, Axis::Row);
    register_axis(registry, "insertColumns", true, Axis::Col);
    register_axis(registry, "deleteColumns", false, Axis::Col);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::cells::write_input;
    use std::fs;
    use std::path::PathBuf;

    fn spec(axis: Axis, at: i32, count: i32) -> ShiftSpec {
        ShiftSpec {
            sheet: "Sheet1".into(),
            axis,
            at,
            count,
        }
    }

    #[test]
    fn catches_spaced_area_join_on_row_delete() {
        // Catches: `A1 : A5` (spaces around the colon) becoming `#REF! : A4` when the first row is deleted.
        assert_eq!(
            shift_formula("A1 : A5", "Sheet1", &spec(Axis::Row, 0, -1)),
            "A1:A4"
        );
        // Absolute markers do not freeze a reference against insert/delete.
        assert_eq!(
            shift_formula("$A$1", "Sheet1", &spec(Axis::Row, 0, 1)),
            "$A$2"
        );
        assert_eq!(
            shift_formula("A:A", "Sheet1", &spec(Axis::Row, 0, 1)),
            "A:A"
        );
        assert_eq!(
            shift_formula("Sheet2!A1", "Sheet1", &spec(Axis::Row, 0, 1)),
            "Sheet2!A1"
        );
    }

    fn fixture_bytes() -> Vec<u8> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/generated/xlsx/cells_1k.xlsx");
        fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    }

    fn formula_at(host: &XlsxHost, row: u32, col: u32) -> Option<String> {
        host.sheets[0]
            .cells
            .iter()
            .find(|c| c.row == row && c.col == col)
            .and_then(|c| c.formula.clone())
    }

    #[test]
    fn catches_unshifted_formula_after_row_insert() {
        // Catches: insertRows moving B1 down while leaving formula `A1` unshifted (negative control).
        let bytes = fixture_bytes();
        let mut broken_host = XlsxHost::open(&bytes).expect("open");
        write_input(&mut broken_host, 0, 0, 1, "=A1").expect("formula");
        shift_axis(
            &mut broken_host,
            0,
            Axis::Row,
            0,
            1,
            true,
            ShiftFlags {
                rewrite_formulas: false,
            },
        )
        .expect("insert without rewrite");
        assert!(
            formula_at(&broken_host, 0, 1).is_none(),
            "original B1 must move"
        );
        assert_eq!(formula_at(&broken_host, 1, 1).as_deref(), Some("A1"));

        let mut host = XlsxHost::open(&bytes).expect("open");
        write_input(&mut host, 0, 0, 1, "=A1").expect("formula");
        insert_rows(&mut host, 0, 0, 1).expect("insert");
        let shifted = formula_at(&host, 1, 1);
        assert_eq!(shifted.as_deref(), Some("A2"));
        assert_ne!(
            shifted.as_deref(),
            Some("A1"),
            "negative control: an unshifted formula must not compare equal to a real row insert"
        );
    }
}
