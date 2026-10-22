// SPDX-License-Identifier: MIT
//! Viewer engine API helpers (#19): sheets list, copy, hyperlinks, search.

use nitro_xml_opc::opc::hyperlink_policy;
use serde_json::{json, Map, Value};

use crate::address::parse_range;
use crate::model::{ActiveWorksheet, SheetState, WorkbookSlice};
use crate::sheet_access::{AllSheetsSlice, CellIndex};
use crate::view::cell_view::{cell_view, CellViewOptions};

pub fn visible_sheets_json(workbook: &WorkbookSlice) -> Value {
    let sheets: Vec<Value> = workbook
        .sheets
        .iter()
        .enumerate()
        .filter(|(_, s)| s.state == SheetState::Visible)
        .map(|(index, stub)| {
            let mut m = Map::new();
            m.insert("index".into(), json!(index));
            m.insert("name".into(), Value::String(stub.name.clone()));
            m.insert("sheetId".into(), json!(stub.sheet_id));
            m.insert("state".into(), Value::String(stub.state.as_str().into()));
            Value::Object(m)
        })
        .collect();
    Value::Array(sheets)
}

fn displayed_text(
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    row: u32,
    col: u32,
    sheet_index: usize,
    all_sheets: AllSheetsSlice<'_>,
) -> String {
    let opts = CellViewOptions {
        sheet_index: Some(sheet_index),
        all_sheets: Some(all_sheets),
        ..CellViewOptions::default()
    };
    cell_view(workbook, sheet, index, row, col, opts).text
}

pub fn cell_text_tsv(
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
    sheet_index: usize,
    all_sheets: AllSheetsSlice<'_>,
    range_ref: &str,
) -> Result<String, String> {
    let range = parse_range(range_ref).ok_or_else(|| format!("invalid range {}", range_ref))?;
    let index = CellIndex::build(sheet);
    let mut lines = Vec::new();
    for row in range.start.row..=range.end.row {
        let mut cols = Vec::new();
        for col in range.start.col..=range.end.col {
            cols.push(displayed_text(
                workbook,
                sheet,
                &index,
                row,
                col,
                sheet_index,
                all_sheets,
            ));
        }
        lines.push(cols.join("\t"));
    }
    Ok(lines.join("\n"))
}

fn link_allowed(target: &Option<String>, location: &Option<String>) -> bool {
    if location.as_ref().is_some_and(|l| !l.is_empty()) {
        return true;
    }
    let href = target.as_deref().filter(|s| !s.is_empty());
    hyperlink_policy(href).store
}

pub fn hyperlinks_in_sheet(
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
    sheet_index: usize,
) -> Value {
    let mut out = Vec::new();
    for link in &sheet.hyperlinks {
        if !link_allowed(&link.target, &link.location) {
            continue;
        }
        let mut m = Map::new();
        m.insert("sheet".into(), json!(sheet_index));
        m.insert("row".into(), json!(link.range.start.row));
        m.insert("col".into(), json!(link.range.start.col));
        if let Some(t) = &link.target {
            m.insert("target".into(), Value::String(t.clone()));
        }
        if let Some(l) = &link.location {
            m.insert("location".into(), Value::String(l.clone()));
        }
        if let Some(tip) = &link.tooltip {
            m.insert("tooltip".into(), Value::String(tip.clone()));
        }
        let open = link.location.as_ref().is_some_and(|l| !l.is_empty())
            || hyperlink_policy(link.target.as_deref()).open;
        m.insert("open".into(), json!(open));
        out.push(Value::Object(m));
    }
    let _ = workbook;
    Value::Array(out)
}

pub fn hyperlink_at(
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
    sheet_index: usize,
    row: u32,
    col: u32,
) -> Option<Value> {
    for link in &sheet.hyperlinks {
        if row < link.range.start.row
            || row > link.range.end.row
            || col < link.range.start.col
            || col > link.range.end.col
        {
            continue;
        }
        if !link_allowed(&link.target, &link.location) {
            return None;
        }
        let mut m = Map::new();
        m.insert("sheet".into(), json!(sheet_index));
        m.insert("row".into(), json!(row));
        m.insert("col".into(), json!(col));
        if let Some(t) = &link.target {
            m.insert("target".into(), Value::String(t.clone()));
        }
        if let Some(l) = &link.location {
            m.insert("location".into(), Value::String(l.clone()));
        }
        if let Some(tip) = &link.tooltip {
            m.insert("tooltip".into(), Value::String(tip.clone()));
        }
        let open = link.location.as_ref().is_some_and(|l| !l.is_empty())
            || hyperlink_policy(link.target.as_deref()).open;
        m.insert("open".into(), json!(open));
        let _ = workbook;
        return Some(Value::Object(m));
    }
    None
}

#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub match_entire: bool,
}

pub fn search_workbook(
    workbook: &WorkbookSlice,
    sheets: &[Option<ActiveWorksheet>],
    query: &str,
    opts: SearchOptions,
) -> Value {
    if query.is_empty() {
        return Value::Array(vec![]);
    }
    let mut matches = Vec::new();
    for (sheet_index, sheet_opt) in sheets.iter().enumerate() {
        let Some(sheet) = sheet_opt else {
            continue;
        };
        if workbook
            .sheets
            .get(sheet_index)
            .is_some_and(|s| s.state != SheetState::Visible)
        {
            continue;
        }
        let index = CellIndex::build(sheet);
        for cell in &sheet.cells {
            let text = displayed_text(
                workbook,
                sheet,
                &index,
                cell.row,
                cell.col,
                sheet_index,
                AllSheetsSlice::viewer(sheets),
            );
            let hit = if opts.case_sensitive {
                if opts.match_entire {
                    text == query
                } else {
                    text.contains(query)
                }
            } else if opts.match_entire {
                text.eq_ignore_ascii_case(query)
            } else {
                text.to_ascii_lowercase()
                    .contains(&query.to_ascii_lowercase())
            };
            if hit {
                let mut m = Map::new();
                m.insert("sheet".into(), json!(sheet_index));
                m.insert("row".into(), json!(cell.row));
                m.insert("col".into(), json!(cell.col));
                m.insert("text".into(), Value::String(text));
                matches.push(Value::Object(m));
            }
        }
    }
    Value::Array(matches)
}
