// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/write-table.ts; changes listed in PROVENANCE.md.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use nitro_xml_opc::xml::Element;

use super::element::{children_w, first_w, make_w, replace_children_local};
use super::paragraph::{create_paragraph, write_paragraph};
use crate::table::can_edit_table_structure;

fn table_layout(table: &Value) -> Vec<Vec<Vec<String>>> {
    let mut out = Vec::new();
    let empty: &[Value] = &[];
    let rows = table.get("rows").and_then(|r| r.as_array());
    for row in rows.map(|v| v.as_slice()).unwrap_or(empty) {
        let mut row_ids = Vec::new();
        for cell in row.as_array().map(|v| v.as_slice()).unwrap_or(empty) {
            let ids = cell
                .get("paragraphs")
                .and_then(|p| p.as_array())
                .map(|ps| {
                    ps.iter()
                        .filter_map(|p| p.get("id").and_then(|id| id.as_str()).map(String::from))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            row_ids.push(ids);
        }
        out.push(row_ids);
    }
    out
}

struct SourceCell {
    row: usize,
    col: usize,
    cell: Element,
    paragraphs: Vec<Value>,
}

pub fn write_table(table: &Value, node: &Element, base: Option<&Value>) -> Result<Element, String> {
    let empty: &[Value] = &[];
    if base.is_some_and(|b| table == b) {
        return Ok(node.clone());
    }
    let structural = base.is_some_and(|b| table_layout(table) != table_layout(b));
    if structural && !can_edit_table_structure(node) {
        return Err("Cannot change the structure of a merged, nested, or complex table".into());
    }

    let old_rows: Vec<Element> = children_w(node, "tr").into_iter().cloned().collect();
    let mut source_by_id: HashMap<String, SourceCell> = HashMap::new();
    if let Some(base) = base {
        let base_rows = base.get("rows").and_then(|r| r.as_array());
        for (ri, row) in base_rows
            .map(|v| v.as_slice())
            .unwrap_or(empty)
            .iter()
            .enumerate()
        {
            for (ci, cell) in row
                .as_array()
                .map(|v| v.as_slice())
                .unwrap_or(empty)
                .iter()
                .enumerate()
            {
                let old_tc: Option<Element> = old_rows
                    .get(ri)
                    .and_then(|tr| children_w(tr, "tc").get(ci).map(|cell| (*cell).clone()));
                let paras = cell
                    .get("paragraphs")
                    .and_then(|p| p.as_array())
                    .cloned()
                    .unwrap_or_default();
                for p in &paras {
                    if let Some(id) = p.get("id").and_then(|v| v.as_str()) {
                        if let Some(tc) = old_tc.clone() {
                            source_by_id.insert(
                                id.to_owned(),
                                SourceCell {
                                    row: ri,
                                    col: ci,
                                    cell: tc,
                                    paragraphs: paras.clone(),
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    let mut out = node.clone();
    let mut used_rows = HashSet::new();
    let mut next_rows: Vec<Element> = Vec::new();
    let rows = table
        .get("rows")
        .and_then(|r| r.as_array())
        .map(|v| v.as_slice())
        .unwrap_or(empty);

    for (row_index, row) in rows.iter().enumerate() {
        let cells = row.as_array().map(|v| v.as_slice()).unwrap_or(empty);
        if cells.is_empty() {
            return Err("Tables must contain rectangular rows".into());
        }
        let mut source_row: Option<usize> = None;
        for cell in cells {
            let id = cell
                .get("paragraphs")
                .and_then(|p| p.as_array())
                .and_then(|ps| ps.first())
                .and_then(|p| p.get("id"))
                .and_then(|id| id.as_str());
            if let Some(id) = id {
                if let Some(src) = source_by_id.get(id) {
                    if source_row.is_some() && source_row != Some(src.row) {
                        return Err("Moving paragraphs between table rows is not supported".into());
                    }
                    source_row = Some(src.row);
                }
            }
        }
        if let Some(sr) = source_row {
            if used_rows.contains(&sr) {
                return Err("Splitting an existing table row is not supported".into());
            }
            used_rows.insert(sr);
        }

        let tr = source_row
            .and_then(|sr| old_rows.get(sr).cloned())
            .unwrap_or_else(|| make_w("tr"));
        let old_cells: Vec<Element> = children_w(&tr, "tc").into_iter().cloned().collect();
        let mut next_cells: Vec<Element> = Vec::new();

        for (ci, cell) in cells.iter().enumerate() {
            let paras = cell
                .get("paragraphs")
                .and_then(|p| p.as_array())
                .ok_or_else(|| format!("cell {row_index},{ci} missing paragraphs"))?;
            if paras.is_empty() {
                return Err("cell must have at least one paragraph".into());
            }
            let para_id = paras[0].get("id").and_then(|v| v.as_str());
            let source = para_id.and_then(|id| source_by_id.get(id));
            let tc = source
                .map(|s| s.cell.clone())
                .unwrap_or_else(|| make_w("tc"));
            let old_paragraphs: Vec<Element> = children_w(&tc, "p").into_iter().cloned().collect();
            let base_cell = base.and_then(|b| {
                b.get("rows")
                    .and_then(|r| r.as_array())
                    .and_then(|rows| rows.get(row_index))
                    .and_then(|r| r.as_array())
                    .and_then(|row| row.get(ci))
            });
            if base_cell.is_some_and(|bc| bc == cell) {
                next_cells.push(tc);
                continue;
            }
            let base_paras = base_cell.and_then(|c| c.get("paragraphs").and_then(|p| p.as_array()));
            let mut next_paragraphs: Vec<Element> = Vec::new();
            for (pi, para) in paras.iter().enumerate() {
                let base_para = base_paras.and_then(|bp| bp.get(pi));
                let written = if let Some(old) = old_paragraphs.get(pi) {
                    write_paragraph(para, old, base_para)?
                } else {
                    create_paragraph(para)?
                };
                next_paragraphs.push(written);
            }
            let mut tc_mut = tc;
            replace_children_local(&mut tc_mut, "p", old_paragraphs.len(), &next_paragraphs);
            next_cells.push(tc_mut);
        }
        let mut tr_mut = tr;
        replace_children_local(&mut tr_mut, "tc", old_cells.len(), &next_cells);
        next_rows.push(tr_mut);
    }

    let old_row_count = children_w(&out, "tr").len();
    replace_children_local(&mut out, "tr", old_row_count, &next_rows);

    if structural {
        let width = rows
            .first()
            .and_then(|r| r.as_array())
            .map(|c| c.len())
            .unwrap_or(0);
        if let Some(grid) = first_w(&out, "tblGrid").cloned() {
            let old_cols: Vec<Element> =
                children_w(&grid, "gridCol").into_iter().cloned().collect();
            if width != old_cols.len() {
                let mut next_cols: Vec<Element> = Vec::new();
                for index in 0..width {
                    let old = old_cols
                        .get(index)
                        .cloned()
                        .or_else(|| old_cols.last().cloned());
                    next_cols.push(old.unwrap_or_else(|| make_w("gridCol")));
                }
                let mut grid_mut = grid;
                replace_children_local(&mut grid_mut, "gridCol", old_cols.len(), &next_cols);
                for child in &mut out.children {
                    if let nitro_xml_opc::xml::Node::Element(e) = child {
                        if e.local == "tblGrid" {
                            *e = grid_mut;
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(out)
}
