// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/editor/render/table-layout.ts
// and src/core/pptx/editor/render/table-cell-edit.ts; changes listed in PROVENANCE.md.

//! Slide table edits. Op names are pptx-specific so they do not overload docx table ops (#85).

use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry};
use serde::{Deserialize, Serialize};

use super::host::{EditableElement, PptxHost};
use super::shapes::{arg_id, arg_str, arg_u32, arg_u32_opt, host_mut, map_err};

/// Default pixel height of a freshly inserted row (`DEFAULT_INSERTED_ROW_HEIGHT`).
pub const DEFAULT_INSERTED_ROW_HEIGHT: f64 = 40.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TableModel {
    #[serde(rename = "columnWidths")]
    pub column_widths: Vec<f64>,
    pub rows: Vec<TableRowModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TableRowModel {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    pub cells: Vec<TableCellModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TableCellModel {
    pub text: String,
    #[serde(default, skip_serializing_if = "span_is_one", rename = "gridSpan")]
    pub grid_span: u32,
    #[serde(default, skip_serializing_if = "span_is_one", rename = "rowSpan")]
    pub row_span: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not", rename = "hMerge")]
    pub h_merge: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not", rename = "vMerge")]
    pub v_merge: bool,
}

fn span_is_one(n: &u32) -> bool {
    *n <= 1
}

impl TableCellModel {
    fn blank() -> Self {
        Self {
            text: String::new(),
            grid_span: 1,
            row_span: 1,
            h_merge: false,
            v_merge: false,
        }
    }

    fn continuation(vertical: bool) -> Self {
        let mut cell = Self::blank();
        if vertical {
            cell.v_merge = true;
        } else {
            cell.h_merge = true;
        }
        cell
    }
}

impl TableModel {
    pub(crate) fn from_parsed(data: &crate::table::TableData) -> Self {
        Self {
            column_widths: data.column_widths.clone(),
            rows: data
                .rows
                .iter()
                .map(|row| TableRowModel {
                    height: row.height_px.filter(|h| *h > 0.0),
                    cells: row
                        .cells
                        .iter()
                        .map(|cell| TableCellModel {
                            text: cell.text.clone(),
                            grid_span: cell.grid_span.max(1),
                            row_span: cell.row_span.max(1),
                            h_merge: cell.h_merge,
                            v_merge: cell.v_merge,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

/// `setCellText`. Rejected when the element is not a table (negative control).
pub fn set_cell_text(
    el: &mut EditableElement,
    row: u32,
    col: u32,
    text: &str,
) -> Result<(), String> {
    if el.type_ != "table" {
        return Err("setSlideCellText rejected: not a table".into());
    }
    let table = el
        .table
        .as_mut()
        .ok_or_else(|| "setSlideCellText rejected: not a table".to_owned())?;
    if let Some(row) = table.rows.get_mut(row as usize) {
        if let Some(cell) = row.cells.get_mut(col as usize) {
            cell.text = text.to_owned();
        }
    }
    Ok(())
}

/// `insertTableRow`. Merge anchors that span the insertion grow by one.
pub fn insert_row(table: &mut TableModel, row_idx: usize, position: &str) {
    let insert_idx = if position == "above" {
        row_idx
    } else {
        row_idx + 1
    };
    let col_count = table.column_widths.len();
    let mut new_cells = Vec::with_capacity(col_count);
    for c in 0..col_count {
        let mut inside = false;
        for r in 0..insert_idx {
            let Some(cell) = table.rows.get(r).and_then(|row| row.cells.get(c)) else {
                continue;
            };
            let rs = cell.row_span.max(1);
            if rs > 1 && r + rs as usize > insert_idx && !cell.v_merge && !cell.h_merge {
                inside = true;
                break;
            }
        }
        new_cells.push(if inside {
            TableCellModel::continuation(true)
        } else {
            TableCellModel::blank()
        });
    }
    for ri in 0..insert_idx.min(table.rows.len()) {
        let mut needs = false;
        let updated: Vec<TableCellModel> = table.rows[ri]
            .cells
            .iter()
            .map(|cell| {
                let rs = cell.row_span.max(1);
                if rs > 1 && ri + rs as usize > insert_idx && !cell.v_merge && !cell.h_merge {
                    needs = true;
                    let mut next = cell.clone();
                    next.row_span = rs + 1;
                    next
                } else {
                    cell.clone()
                }
            })
            .collect();
        if needs {
            table.rows[ri].cells = updated;
        }
    }
    let row = TableRowModel {
        height: Some(DEFAULT_INSERTED_ROW_HEIGHT),
        cells: new_cells,
    };
    let at = insert_idx.min(table.rows.len());
    table.rows.insert(at, row);
}

/// `deleteTableRow`. One-row tables and out-of-range indexes are unchanged.
pub fn delete_row(table: &mut TableModel, row_idx: usize) {
    if table.rows.len() <= 1 || row_idx >= table.rows.len() {
        return;
    }
    let removed = table.rows[row_idx].clone();
    for c in 0..removed.cells.len() {
        let cell = &removed.cells[c];
        if cell.v_merge {
            for r in (0..row_idx).rev() {
                let Some(above) = table.rows.get(r).and_then(|row| row.cells.get(c)) else {
                    break;
                };
                if !above.v_merge {
                    let rs = above.row_span.max(1);
                    if rs > 1 {
                        let new_span = if rs - 1 > 1 { rs - 1 } else { 1 };
                        table.rows[r].cells[c].row_span = new_span;
                    }
                    break;
                }
            }
        } else {
            let rs = cell.row_span.max(1);
            if rs > 1 {
                let next_row = row_idx + 1;
                if next_row < table.rows.len() {
                    if let Some(next) = table.rows[next_row].cells.get_mut(c) {
                        if !cell.text.is_empty() {
                            next.text = cell.text.clone();
                        }
                        let new_rs = rs - 1;
                        next.row_span = if new_rs > 1 { new_rs } else { 1 };
                        next.v_merge = false;
                        next.grid_span = cell.grid_span;
                    }
                }
            }
        }
    }
    table.rows.remove(row_idx);
}

fn normalize_widths(widths: &[f64]) -> Vec<f64> {
    let sum: f64 = widths.iter().sum();
    if sum > 0.0 {
        widths.iter().map(|w| w / sum).collect()
    } else {
        widths.to_vec()
    }
}

/// `insertTableColumn`. The source column's width is split in half, then renormalised.
pub fn insert_column(table: &mut TableModel, col_idx: usize, position: &str) {
    let insert_idx = if position == "left" {
        col_idx
    } else {
        col_idx + 1
    };
    let mut new_widths = table.column_widths.clone();
    if new_widths.is_empty() {
        return;
    }
    let split_source = if insert_idx < new_widths.len() {
        insert_idx
    } else {
        new_widths.len() - 1
    };
    let original = new_widths[split_source];
    let half = original / 2.0;
    new_widths[split_source] = half;
    let at = insert_idx.min(new_widths.len());
    new_widths.insert(at, half);
    table.column_widths = normalize_widths(&new_widths);

    for row in &mut table.rows {
        let mut inside = false;
        for c in 0..insert_idx.min(row.cells.len()) {
            let cell = &row.cells[c];
            let gs = cell.grid_span.max(1);
            if gs > 1 && c + gs as usize > insert_idx && !cell.h_merge {
                inside = true;
                break;
            }
        }
        let cell = if inside {
            TableCellModel::continuation(false)
        } else {
            TableCellModel::blank()
        };
        let at = insert_idx.min(row.cells.len());
        row.cells.insert(at, cell);
    }

    for row in &mut table.rows {
        let mut needs = false;
        let updated: Vec<TableCellModel> = row
            .cells
            .iter()
            .enumerate()
            .map(|(ci, cell)| {
                if ci >= insert_idx {
                    return cell.clone();
                }
                let gs = cell.grid_span.max(1);
                if gs > 1 && ci + gs as usize > insert_idx && !cell.h_merge && !cell.v_merge {
                    needs = true;
                    let mut next = cell.clone();
                    next.grid_span = gs + 1;
                    next
                } else {
                    cell.clone()
                }
            })
            .collect();
        if needs {
            row.cells = updated;
        }
    }
}

/// `deleteTableColumn`. One-column tables and out-of-range indexes are unchanged.
pub fn delete_column(table: &mut TableModel, col_idx: usize) {
    if table.column_widths.len() <= 1 || col_idx >= table.column_widths.len() {
        return;
    }
    for row in &mut table.rows {
        if col_idx >= row.cells.len() {
            continue;
        }
        let cell = row.cells[col_idx].clone();
        if cell.h_merge {
            for c in (0..col_idx).rev() {
                let left = &row.cells[c];
                if !left.h_merge {
                    let gs = left.grid_span.max(1);
                    if gs > 1 {
                        row.cells[c].grid_span = if gs - 1 > 1 { gs - 1 } else { 1 };
                    }
                    break;
                }
            }
        } else {
            let gs = cell.grid_span.max(1);
            if gs > 1 {
                let next_col = col_idx + 1;
                if next_col < row.cells.len() {
                    let next = &mut row.cells[next_col];
                    if !cell.text.is_empty() {
                        next.text = cell.text.clone();
                    }
                    let new_gs = gs - 1;
                    next.grid_span = if new_gs > 1 { new_gs } else { 1 };
                    next.h_merge = false;
                    next.row_span = cell.row_span;
                }
            }
        }
        row.cells.remove(col_idx);
    }
    let mut widths = table.column_widths.clone();
    widths.remove(col_idx);
    table.column_widths = normalize_widths(&widths);
}

fn require_table(el: &mut EditableElement) -> Result<&mut TableModel, String> {
    if el.type_ != "table" {
        return Err("element is not a table".into());
    }
    el.table
        .as_mut()
        .ok_or_else(|| "element is not a table".to_owned())
}

fn change(kind: ChangeKind, label: &str) -> ChangeSet {
    ChangeSet::new(kind, label)
}

pub fn register_table_commands(registry: &mut CommandRegistry) {
    registry.register(
        Command::stub("setSlideCellText", "setSlideCellText").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            let row = arg_u32(args, "row")?;
            let col = arg_u32(args, "col")?;
            let text = arg_str(args, "text")?.to_owned();
            h.set_slide_cell_text(
                slide,
                arg_id(args),
                arg_u32_opt(args, "elementIndex"),
                row,
                col,
                &text,
            )
            .map_err(map_err)?;
            Ok(change(ChangeKind::Other, "setSlideCellText"))
        }),
    );
    registry.register(
        Command::stub("insertSlideTableRow", "insertSlideTableRow").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            let row = arg_u32(args, "row")?;
            let position = arg_str(args, "position")?.to_owned();
            h.insert_slide_row(
                slide,
                arg_id(args),
                arg_u32_opt(args, "elementIndex"),
                row,
                &position,
            )
            .map_err(map_err)?;
            Ok(change(ChangeKind::Structure, "insertSlideTableRow"))
        }),
    );
    registry.register(
        Command::stub("deleteSlideTableRow", "deleteSlideTableRow").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            let row = arg_u32(args, "row")?;
            h.delete_slide_row(slide, arg_id(args), arg_u32_opt(args, "elementIndex"), row)
                .map_err(map_err)?;
            Ok(change(ChangeKind::Structure, "deleteSlideTableRow"))
        }),
    );
    registry.register(
        Command::stub("insertSlideTableColumn", "insertSlideTableColumn").with_runner(
            |host, args| {
                let h = host_mut(host)?;
                let slide = arg_u32(args, "slideIndex")?;
                let col = arg_u32(args, "col")?;
                let position = arg_str(args, "position")?.to_owned();
                h.insert_slide_column(
                    slide,
                    arg_id(args),
                    arg_u32_opt(args, "elementIndex"),
                    col,
                    &position,
                )
                .map_err(map_err)?;
                Ok(change(ChangeKind::Structure, "insertSlideTableColumn"))
            },
        ),
    );
    registry.register(
        Command::stub("deleteSlideTableColumn", "deleteSlideTableColumn").with_runner(
            |host, args| {
                let h = host_mut(host)?;
                let slide = arg_u32(args, "slideIndex")?;
                let col = arg_u32(args, "col")?;
                h.delete_slide_column(slide, arg_id(args), arg_u32_opt(args, "elementIndex"), col)
                    .map_err(map_err)?;
                Ok(change(ChangeKind::Structure, "deleteSlideTableColumn"))
            },
        ),
    );
}

impl PptxHost {
    pub fn set_slide_cell_text(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        row: u32,
        col: u32,
        text: &str,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        set_cell_text(&mut slide.elements[idx], row, col, text)
    }

    pub fn insert_slide_row(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        row: u32,
        position: &str,
    ) -> Result<(), String> {
        if !matches!(position, "above" | "below") {
            return Err(format!("insertSlideTableRow position {position}"));
        }
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        let table = require_table(&mut slide.elements[idx])?;
        insert_row(table, row as usize, position);
        Ok(())
    }

    pub fn delete_slide_row(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        row: u32,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        let table = require_table(&mut slide.elements[idx])?;
        delete_row(table, row as usize);
        Ok(())
    }

    pub fn insert_slide_column(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        col: u32,
        position: &str,
    ) -> Result<(), String> {
        if !matches!(position, "left" | "right") {
            return Err(format!("insertSlideTableColumn position {position}"));
        }
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        let table = require_table(&mut slide.elements[idx])?;
        insert_column(table, col as usize, position);
        Ok(())
    }

    pub fn delete_slide_column(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        col: u32,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        let table = require_table(&mut slide.elements[idx])?;
        delete_column(table, col as usize);
        Ok(())
    }
}
