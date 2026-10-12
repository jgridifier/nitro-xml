// SPDX-License-Identifier: MIT
//! Mutable xlsx edit host: workbook slice + parsed worksheets (#66).

use std::any::Any;
use std::collections::{BTreeSet, HashMap};

use nitro_xml_opc::edit::{EditHost, EditScope, HostSnapshot};
use serde_json::{json, Map, Value};

use crate::model::{
    ActiveWorksheet, CellAddress, CellRange, DumpCell, DumpCellValue, WorkbookSlice,
};
use crate::styles::style_at;
use crate::workbook::{load_workbook, parse_all_worksheets};

use super::style_patch::cell_style_from_json;

/// In-memory xlsx document the cell/style command runners mutate.
#[derive(Debug, Clone)]
pub struct XlsxHost {
    pub workbook: WorkbookSlice,
    pub sheets: Vec<ActiveWorksheet>,
    /// Cells written or styled during this session (for differential dumps).
    pub touched: BTreeSet<(u32, u32, u32)>,
    /// Style-key → index cache for [`super::style_patch::intern_style`].
    pub style_cache: HashMap<String, usize>,
}

impl XlsxHost {
    /// Load every worksheet so cell edits can address any sheet index.
    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        let workbook = load_workbook(bytes)?;
        let parsed = parse_all_worksheets(bytes, &workbook)?;
        let sheets: Vec<ActiveWorksheet> = parsed
            .into_iter()
            .enumerate()
            .map(|(i, ws)| {
                ws.unwrap_or_else(|| {
                    // Chart / non-worksheet stub — empty grid with workbook sheet metadata.
                    let stub = &workbook.sheets[i];
                    ActiveWorksheet {
                        name: stub.name.clone(),
                        sheet_id: stub.sheet_id,
                        state: stub.state,
                        part_name: stub.part_name.clone(),
                        default_row_height: 15.0,
                        default_col_width: None,
                        format: None,
                        view: crate::sheet_props::default_sheet_view(),
                        columns: Vec::new(),
                        merges: Vec::new(),
                        row_info: Default::default(),
                        cells: Vec::new(),
                        data_validations: Vec::new(),
                        comment_addresses: Vec::new(),
                        hyperlinks: Vec::new(),
                        ext_lst: Vec::new(),
                        sparkline_groups: Vec::new(),
                    }
                })
            })
            .collect();
        Ok(Self {
            workbook,
            sheets,
            touched: BTreeSet::new(),
            style_cache: HashMap::new(),
        })
    }

    pub fn mark_touched(&mut self, sheet: u32, row: u32, col: u32) {
        self.touched.insert((sheet, row, col));
    }

    pub fn sheet_mut(&mut self, index: u32) -> Result<&mut ActiveWorksheet, String> {
        self.sheets
            .get_mut(index as usize)
            .ok_or_else(|| format!("No sheet at index {index}"))
    }

    pub fn sheet(&self, index: u32) -> Result<&ActiveWorksheet, String> {
        self.sheets
            .get(index as usize)
            .ok_or_else(|| format!("No sheet at index {index}"))
    }

    /// Differential model dump: touched cells only, sorted, plus style table size.
    pub fn model_snapshot(&self) -> Value {
        let mut cells = Vec::new();
        for &(sheet, r, c) in &self.touched {
            let entry = match self
                .sheets
                .get(sheet as usize)
                .and_then(|sh| sh.cells.iter().find(|cell| cell.row == r && cell.col == c))
            {
                Some(cell) => {
                    let style = style_at(&self.workbook.styles, cell.style_id).to_json();
                    let mut m = Map::new();
                    m.insert("sheet".into(), json!(sheet));
                    m.insert("r".into(), json!(r));
                    m.insert("c".into(), json!(c));
                    // Match TS model numbers (not gate `__NUM__:` raw encoding).
                    m.insert("v".into(), dump_value_for_edit_snap(&cell.value));
                    if let Some(f) = &cell.formula {
                        m.insert("f".into(), Value::String(f.clone()));
                    }
                    if let Some(s) = cell.style_id {
                        m.insert("s".into(), json!(s));
                    }
                    m.insert("style".into(), style);
                    Value::Object(m)
                }
                None => json!({
                    "sheet": sheet,
                    "r": r,
                    "c": c,
                    "v": null,
                }),
            };
            cells.push(entry);
        }
        json!({
            "cells": cells,
            "stylesCount": self.workbook.styles.len(),
        })
    }

    fn state_json(&self) -> Value {
        json!({
            "styles": self.workbook.styles.iter().map(|s| s.to_json()).collect::<Vec<_>>(),
            "sheets": self.sheets.iter().map(|sh| {
                json!({
                    "cells": sh.cells.iter().map(dump_cell_to_snap).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
            "touched": self.touched.iter().map(|(s, r, c)| json!([s, r, c])).collect::<Vec<_>>(),
        })
    }

    fn apply_state_json(&mut self, v: &Value) -> Result<(), String> {
        let styles = v
            .get("styles")
            .and_then(|x| x.as_array())
            .ok_or_else(|| "snapshot missing styles".to_owned())?;
        self.workbook.styles = styles
            .iter()
            .map(cell_style_from_json)
            .collect::<Result<Vec<_>, _>>()?;
        let sheets = v
            .get("sheets")
            .and_then(|x| x.as_array())
            .ok_or_else(|| "snapshot missing sheets".to_owned())?;
        if sheets.len() != self.sheets.len() {
            return Err("snapshot sheet count mismatch".into());
        }
        for (i, sheet_v) in sheets.iter().enumerate() {
            let cells = sheet_v
                .get("cells")
                .and_then(|x| x.as_array())
                .ok_or_else(|| format!("snapshot sheet {i} missing cells"))?;
            self.sheets[i].cells = cells
                .iter()
                .map(dump_cell_from_snap)
                .collect::<Result<Vec<_>, _>>()?;
        }
        self.touched.clear();
        if let Some(touched) = v.get("touched").and_then(|x| x.as_array()) {
            for t in touched {
                let arr = t
                    .as_array()
                    .ok_or_else(|| "touched entry must be [sheet,r,c]".to_owned())?;
                if arr.len() != 3 {
                    return Err("touched entry must be [sheet,r,c]".into());
                }
                let s = arr[0].as_u64().ok_or("touched sheet")? as u32;
                let r = arr[1].as_u64().ok_or("touched row")? as u32;
                let c = arr[2].as_u64().ok_or("touched col")? as u32;
                self.touched.insert((s, r, c));
            }
        }
        self.style_cache.clear();
        Ok(())
    }
}

impl EditHost for XlsxHost {
    fn capture(&self, _scopes: &[EditScope]) -> HostSnapshot {
        HostSnapshot(serde_json::to_vec(&self.state_json()).unwrap_or_default())
    }

    fn restore(&mut self, snap: &HostSnapshot) {
        if snap.0.is_empty() {
            return;
        }
        if let Ok(v) = serde_json::from_slice::<Value>(&snap.0) {
            let _ = self.apply_state_json(&v);
        }
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}

fn dump_value_for_edit_snap(value: &DumpCellValue) -> Value {
    match value {
        DumpCellValue::NumberRaw(s) => s
            .parse::<f64>()
            .map(crate::model::js_num)
            .unwrap_or_else(|_| Value::String(s.clone())),
        other => other.to_json(),
    }
}

fn dump_cell_to_snap(cell: &DumpCell) -> Value {
    cell.to_json()
}

fn dump_cell_from_snap(v: &Value) -> Result<DumpCell, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "dump cell must be an object".to_owned())?;
    let row = obj
        .get("r")
        .and_then(|x| x.as_u64())
        .ok_or_else(|| "cell missing r".to_owned())? as u32;
    let col = obj
        .get("c")
        .and_then(|x| x.as_u64())
        .ok_or_else(|| "cell missing c".to_owned())? as u32;
    let value = match obj.get("v") {
        None | Some(Value::Null) => DumpCellValue::Null,
        Some(Value::Bool(b)) => DumpCellValue::Bool(*b),
        Some(Value::Number(n)) => DumpCellValue::Number(n.as_f64().unwrap_or(0.0)),
        Some(Value::String(s)) => {
            if let Some(raw) = s.strip_prefix("__NUM__:") {
                DumpCellValue::NumberRaw(raw.to_owned())
            } else {
                DumpCellValue::String(s.clone())
            }
        }
        Some(Value::Object(m)) if m.contains_key("error") => DumpCellValue::Error(
            m.get("error")
                .and_then(|x| x.as_str())
                .unwrap_or("#VALUE!")
                .to_owned(),
        ),
        Some(other) => return Err(format!("unsupported cell value {other}")),
    };
    let array_range = match obj.get("arrayRange") {
        Some(ar) => Some(cell_range_from_json(ar)?),
        None => None,
    };
    let spill_anchor = match obj.get("spillAnchor") {
        Some(a) => Some(cell_address_from_json(a)?),
        None => None,
    };
    Ok(DumpCell {
        row,
        col,
        value,
        type_tag: obj.get("t").and_then(|x| x.as_str()).map(str::to_owned),
        formula: obj.get("f").and_then(|x| x.as_str()).map(str::to_owned),
        style_id: obj.get("s").and_then(|x| x.as_u64()).map(|n| n as u32),
        rich_text: None, // rich text not required for #66 cell/style scripts
        array_range,
        legacy_formula: obj.get("legacyFormula").and_then(|x| x.as_bool()),
        dynamic_array: obj.get("dynamicArray").and_then(|x| x.as_bool()),
        spill_anchor,
        cell_metadata: obj
            .get("cellMetadata")
            .and_then(|x| x.as_u64())
            .map(|n| n as u32),
        value_metadata: obj
            .get("valueMetadata")
            .and_then(|x| x.as_u64())
            .map(|n| n as u32),
    })
}

fn cell_address_from_json(v: &Value) -> Result<CellAddress, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "address must be an object".to_owned())?;
    Ok(CellAddress {
        row: obj.get("row").and_then(|x| x.as_u64()).ok_or("row")? as u32,
        col: obj.get("col").and_then(|x| x.as_u64()).ok_or("col")? as u32,
    })
}

fn cell_range_from_json(v: &Value) -> Result<CellRange, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "range must be an object".to_owned())?;
    Ok(CellRange {
        start: cell_address_from_json(obj.get("start").ok_or("start")?)?,
        end: cell_address_from_json(obj.get("end").ok_or("end")?)?,
    })
}
