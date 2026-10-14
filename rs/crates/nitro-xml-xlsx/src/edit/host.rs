// SPDX-License-Identifier: MIT
//! Mutable xlsx edit host: workbook slice + parsed worksheets (#66).

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use nitro_xml_opc::edit::{EditHost, EditScope, HostSnapshot};
use serde_json::{json, Map, Value};

use crate::model::{
    ActiveWorksheet, CellAddress, CellRange, ColumnInfo, DataValidation, DefinedName, DumpCell,
    DumpCellValue, FreezePane, HyperlinkInfo, RowInfo, SheetSelection, SheetView, WorkbookSlice,
};
use crate::styles::style_at;
use crate::workbook::{load_workbook, parse_all_worksheets};

use super::style_patch::cell_style_from_json;

/// One row or column insert/delete, recorded so save can move carried `sqref` markup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisShiftRecord {
    pub sheet: u32,
    /// True when the shift is along rows.
    pub row: bool,
    pub at: u32,
    /// Positive inserts, negative deletes.
    pub count: i32,
}

/// In-memory xlsx document the cell/style command runners mutate.
#[derive(Debug, Clone)]
pub struct XlsxHost {
    pub workbook: WorkbookSlice,
    pub sheets: Vec<ActiveWorksheet>,
    /// Cells written or styled during this session (for differential dumps).
    pub touched: BTreeSet<(u32, u32, u32)>,
    /// Style-key → index cache for [`super::style_patch::intern_style`].
    pub style_cache: HashMap<String, usize>,
    /// Sheets whose cells, rows, or columns changed since open (or the last undo to clean).
    pub dirty_sheets: BTreeSet<u32>,
    /// The style table changed (a new xf or a number-format patch).
    pub styles_dirty: bool,
    /// Sheet list, names, or order changed.
    pub workbook_dirty: bool,
    /// Every formula cell is stale after a structure edit.
    pub all_formulas_stale: bool,
    /// Value edits. Formula cells that read these addresses are stale.
    pub dirty_precedents: BTreeSet<(u32, u32, u32)>,
    /// Formula cells marked stale, keyed by sheet index.
    pub stale: BTreeMap<u32, BTreeSet<(u32, u32)>>,
    /// Row/column shifts applied since open, in order. Save uses this to move
    /// conditional-format `sqref`s that are still copied from the source part.
    pub axis_shifts: Vec<AxisShiftRecord>,
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
                        drawings: Vec::new(),
                        conditional_formats: Vec::new(),
                        dimension: None,
                    }
                })
            })
            .collect();
        Ok(Self {
            workbook,
            sheets,
            touched: BTreeSet::new(),
            style_cache: HashMap::new(),
            dirty_sheets: BTreeSet::new(),
            styles_dirty: false,
            workbook_dirty: false,
            all_formulas_stale: false,
            dirty_precedents: BTreeSet::new(),
            stale: BTreeMap::new(),
            axis_shifts: Vec::new(),
        })
    }

    pub fn mark_touched(&mut self, sheet: u32, row: u32, col: u32) {
        self.touched.insert((sheet, row, col));
    }

    /// True when a save must rewrite modeled parts instead of returning the original bytes.
    pub fn is_dirty(&self) -> bool {
        self.styles_dirty
            || self.workbook_dirty
            || self.all_formulas_stale
            || !self.dirty_sheets.is_empty()
    }

    /// Record a value edit and recompute which formula cells read it.
    pub fn note_value_edit(&mut self, sheet: u32, row: u32, col: u32) {
        self.dirty_sheets.insert(sheet);
        self.dirty_precedents.insert((sheet, row, col));
        self.recompute_stale();
    }

    /// Sheet add, delete, move, or rename: the workbook part is dirty and every formula is stale.
    pub fn mark_structure_dirty(&mut self) {
        self.workbook_dirty = true;
        self.all_formulas_stale = true;
    }

    /// Shift dirty and stale indexes the same way sheet structure commands shift `touched`.
    pub fn remap_sheet_indexes(&mut self, mut map: impl FnMut(u32) -> Option<u32>) {
        self.dirty_sheets = self.dirty_sheets.iter().filter_map(|s| map(*s)).collect();
        self.dirty_precedents = self
            .dirty_precedents
            .iter()
            .filter_map(|(s, r, c)| map(*s).map(|sheet| (sheet, *r, *c)))
            .collect();
        let mut next: BTreeMap<u32, BTreeSet<(u32, u32)>> = BTreeMap::new();
        for (sheet, cells) in &self.stale {
            if let Some(mapped) = map(*sheet) {
                next.entry(mapped)
                    .or_default()
                    .extend(cells.iter().copied());
            }
        }
        self.stale = next;
        self.axis_shifts = self
            .axis_shifts
            .iter()
            .filter_map(|rec| map(rec.sheet).map(|sheet| AxisShiftRecord { sheet, ..*rec }))
            .collect();
    }

    /// Formula cells on `sheet` that need a recalc, sorted by row then column.
    pub fn stale_cells(&self, sheet: u32) -> Vec<(u32, u32)> {
        let mut set = self.stale.get(&sheet).cloned().unwrap_or_default();
        if self.all_formulas_stale {
            if let Some(sh) = self.sheets.get(sheet as usize) {
                for cell in &sh.cells {
                    if cell.formula.is_some() {
                        set.insert((cell.row, cell.col));
                    }
                }
            }
        }
        set.into_iter().collect()
    }

    fn recompute_stale(&mut self) {
        let names: Vec<String> = self.sheets.iter().map(|s| s.name.clone()).collect();
        let mut formulas = Vec::new();
        for (index, sheet) in self.sheets.iter().enumerate() {
            for cell in &sheet.cells {
                if let Some(formula) = &cell.formula {
                    formulas.push((index as u32, cell.row, cell.col, formula.clone()));
                }
            }
        }
        let stale = crate::write::stale_formula_cells(&names, &formulas, &self.dirty_precedents);
        self.stale.clear();
        for (sheet, row, col) in stale {
            self.stale.entry(sheet).or_default().insert((row, col));
        }
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
        let sheets: Vec<Value> = self
            .sheets
            .iter()
            .map(|sh| {
                json!({
                    "name": sh.name,
                    "sheetId": sh.sheet_id,
                    "state": sh.state.as_str(),
                })
            })
            .collect();
        json!({
            "cells": cells,
            "stylesCount": self.workbook.styles.len(),
            "sheets": sheets,
            "activeSheet": self.workbook.active_sheet,
        })
    }

    /// Canonical undo-parity projection — matches `parity/undo/snapshot.mjs` `snapshotXlsx`.
    pub fn undo_parity_snapshot(&self) -> Value {
        let sheets: Vec<Value> = self
            .sheets
            .iter()
            .enumerate()
            .map(|(index, sh)| {
                let mut cells: Vec<Value> = sh
                    .cells
                    .iter()
                    // Match upstream Worksheet.rows: only stored value/formula/style cells.
                    // Skip style-only placeholders with a null value and no formula.
                    .filter(|cell| {
                        !matches!(cell.value, DumpCellValue::Null) || cell.formula.is_some()
                    })
                    .map(|cell| {
                        let mut m = Map::new();
                        m.insert("ref".into(), json!(format!("{}:{}", cell.row, cell.col)));
                        m.insert("v".into(), dump_value_for_edit_snap(&cell.value));
                        // Upstream `Cell` has no `t`/`type` field — snapshot.mjs always yields null.
                        m.insert("t".into(), Value::Null);
                        m.insert(
                            "f".into(),
                            cell.formula
                                .as_ref()
                                .map(|f| Value::String(f.clone()))
                                .unwrap_or(Value::Null),
                        );
                        m.insert(
                            "s".into(),
                            cell.style_id.map(|s| json!(s)).unwrap_or(Value::Null),
                        );
                        Value::Object(m)
                    })
                    .collect();
                cells.sort_by(|a, b| {
                    let ar = a.get("ref").and_then(|x| x.as_str()).unwrap_or("");
                    let br = b.get("ref").and_then(|x| x.as_str()).unwrap_or("");
                    ar.cmp(br)
                });
                json!({
                    "index": index,
                    "name": sh.name,
                    "state": sh.state.as_str(),
                    "cells": cells,
                })
            })
            .collect();
        json!({ "format": "xlsx", "sheets": sheets })
    }

    fn state_json(&self) -> Value {
        let mut stale_map = Map::new();
        for (sheet, cells) in &self.stale {
            stale_map.insert(
                sheet.to_string(),
                Value::Array(cells.iter().map(|(r, c)| json!([r, c])).collect()),
            );
        }
        json!({
            "styles": self.workbook.styles.iter().map(|s| s.to_json()).collect::<Vec<_>>(),
            "activeSheet": self.workbook.active_sheet,
            "definedNames": self.workbook.defined_names.iter().map(|n| n.to_json()).collect::<Vec<_>>(),
            "sheets": self.sheets.iter().map(|sh| {
                json!({
                    "name": sh.name,
                    "sheetId": sh.sheet_id,
                    "state": sh.state.as_str(),
                    "partName": sh.part_name,
                    "defaultRowHeight": sh.default_row_height,
                    "defaultColWidth": sh.default_col_width,
                    "cells": sh.cells.iter().map(dump_cell_to_snap).collect::<Vec<_>>(),
                    "columns": sh.columns.iter().map(|c| c.to_json()).collect::<Vec<_>>(),
                    "rowInfo": sh.row_info.iter().map(|(r, info)| {
                        let mut m = info.to_json().as_object().cloned().unwrap_or_default();
                        m.insert("row".into(), json!(r));
                        Value::Object(m)
                    }).collect::<Vec<_>>(),
                    "merges": sh.merges.iter().map(CellRange::to_json).collect::<Vec<_>>(),
                    "view": view_to_snap(&sh.view),
                    "hyperlinks": sh.hyperlinks.iter().map(hyperlink_to_snap).collect::<Vec<_>>(),
                    "commentAddresses": sh.comment_addresses.iter().map(|a| json!({ "row": a.row, "col": a.col })).collect::<Vec<_>>(),
                    "dataValidations": sh.data_validations.iter().map(validation_to_snap).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
            "touched": self.touched.iter().map(|(s, r, c)| json!([s, r, c])).collect::<Vec<_>>(),
            "dirtySheets": self.dirty_sheets.iter().copied().collect::<Vec<_>>(),
            "stylesDirty": self.styles_dirty,
            "workbookDirty": self.workbook_dirty,
            "allFormulasStale": self.all_formulas_stale,
            "dirtyPrecedents": self.dirty_precedents.iter().map(|(s, r, c)| json!([s, r, c])).collect::<Vec<_>>(),
            "stale": stale_map,
            "axisShifts": self.axis_shifts.iter().map(|rec| json!({
                "sheet": rec.sheet,
                "row": rec.row,
                "at": rec.at,
                "count": rec.count,
            })).collect::<Vec<_>>(),
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
        if let Some(a) = v.get("activeSheet").and_then(|x| x.as_u64()) {
            self.workbook.active_sheet = a as u32;
        }
        if let Some(names) = v.get("definedNames").and_then(|x| x.as_array()) {
            self.workbook.defined_names = names
                .iter()
                .map(defined_name_from_json)
                .collect::<Result<Vec<_>, _>>()?;
        }
        let sheets = v
            .get("sheets")
            .and_then(|x| x.as_array())
            .ok_or_else(|| "snapshot missing sheets".to_owned())?;
        let mut restored = Vec::with_capacity(sheets.len());
        for (i, sheet_v) in sheets.iter().enumerate() {
            let obj = sheet_v
                .as_object()
                .ok_or_else(|| format!("snapshot sheet {i} not object"))?;
            let name = obj
                .get("name")
                .and_then(|x| x.as_str())
                .ok_or_else(|| format!("snapshot sheet {i} missing name"))?
                .to_owned();
            let sheet_id = obj
                .get("sheetId")
                .and_then(|x| x.as_u64())
                .ok_or_else(|| format!("snapshot sheet {i} missing sheetId"))?
                as u32;
            let state = match obj
                .get("state")
                .and_then(|x| x.as_str())
                .unwrap_or("visible")
            {
                "hidden" => crate::model::SheetState::Hidden,
                "veryHidden" => crate::model::SheetState::VeryHidden,
                _ => crate::model::SheetState::Visible,
            };
            let cells = obj
                .get("cells")
                .and_then(|x| x.as_array())
                .ok_or_else(|| format!("snapshot sheet {i} missing cells"))?;
            let mut ws = if let Some(existing) = self.sheets.get(i) {
                let mut clone = existing.clone();
                clone.name = name;
                clone.sheet_id = sheet_id;
                clone.state = state;
                clone
            } else {
                crate::model::ActiveWorksheet {
                    name,
                    sheet_id,
                    state,
                    part_name: obj
                        .get("partName")
                        .and_then(|x| x.as_str())
                        .map(str::to_owned),
                    default_row_height: obj
                        .get("defaultRowHeight")
                        .and_then(|x| x.as_f64())
                        .unwrap_or(15.0),
                    default_col_width: obj.get("defaultColWidth").and_then(|x| x.as_f64()),
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
                    drawings: Vec::new(),
                    conditional_formats: Vec::new(),
                    dimension: None,
                }
            };
            if let Some(p) = obj.get("partName") {
                ws.part_name = p.as_str().map(str::to_owned);
            }
            if let Some(h) = obj.get("defaultRowHeight").and_then(|x| x.as_f64()) {
                ws.default_row_height = h;
            }
            if obj.contains_key("defaultColWidth") {
                ws.default_col_width = obj.get("defaultColWidth").and_then(|x| x.as_f64());
            }
            ws.cells = cells
                .iter()
                .map(dump_cell_from_snap)
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(cols) = obj.get("columns").and_then(|x| x.as_array()) {
                ws.columns = cols
                    .iter()
                    .map(column_info_from_json)
                    .collect::<Result<Vec<_>, _>>()?;
            }
            if let Some(rows) = obj.get("rowInfo").and_then(|x| x.as_array()) {
                ws.row_info.clear();
                for entry in rows {
                    let (row, info) = row_info_from_json(entry)?;
                    ws.row_info.insert(row, info);
                }
            }
            if let Some(merges) = obj.get("merges").and_then(|x| x.as_array()) {
                ws.merges = merges
                    .iter()
                    .map(cell_range_from_json)
                    .collect::<Result<Vec<_>, _>>()?;
            }
            if let Some(view) = obj.get("view") {
                ws.view = view_from_snap(view)?;
            }
            if let Some(links) = obj.get("hyperlinks").and_then(|x| x.as_array()) {
                ws.hyperlinks = links
                    .iter()
                    .map(hyperlink_from_snap)
                    .collect::<Result<Vec<_>, _>>()?;
            }
            if let Some(comments) = obj.get("commentAddresses").and_then(|x| x.as_array()) {
                ws.comment_addresses = comments
                    .iter()
                    .map(cell_address_from_json)
                    .collect::<Result<Vec<_>, _>>()?;
            }
            if let Some(rules) = obj.get("dataValidations").and_then(|x| x.as_array()) {
                ws.data_validations = rules
                    .iter()
                    .map(validation_from_snap)
                    .collect::<Result<Vec<_>, _>>()?;
            }
            restored.push(ws);
        }
        self.sheets = restored;
        self.workbook.sheets = self
            .sheets
            .iter()
            .map(|sh| crate::model::SheetStub {
                name: sh.name.clone(),
                sheet_id: sh.sheet_id,
                state: sh.state,
                part_name: sh.part_name.clone(),
                page_setup: None,
            })
            .collect();
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
        self.dirty_sheets.clear();
        self.styles_dirty = false;
        self.workbook_dirty = false;
        self.all_formulas_stale = false;
        self.dirty_precedents.clear();
        self.stale.clear();
        if let Some(sheets) = v.get("dirtySheets").and_then(|x| x.as_array()) {
            for sheet in sheets {
                if let Some(index) = sheet.as_u64() {
                    self.dirty_sheets.insert(index as u32);
                }
            }
        }
        self.styles_dirty = v
            .get("stylesDirty")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        self.workbook_dirty = v
            .get("workbookDirty")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        self.all_formulas_stale = v
            .get("allFormulasStale")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        if let Some(precedents) = v.get("dirtyPrecedents").and_then(|x| x.as_array()) {
            for entry in precedents {
                let Some(arr) = entry.as_array() else {
                    continue;
                };
                if arr.len() == 3 {
                    if let (Some(s), Some(r), Some(c)) =
                        (arr[0].as_u64(), arr[1].as_u64(), arr[2].as_u64())
                    {
                        self.dirty_precedents.insert((s as u32, r as u32, c as u32));
                    }
                }
            }
        }
        if let Some(map) = v.get("stale").and_then(|x| x.as_object()) {
            for (key, cells) in map {
                let Ok(sheet) = key.parse::<u32>() else {
                    continue;
                };
                let Some(arr) = cells.as_array() else {
                    continue;
                };
                let set = self.stale.entry(sheet).or_default();
                for pair in arr {
                    let Some(pair) = pair.as_array() else {
                        continue;
                    };
                    if pair.len() == 2 {
                        if let (Some(r), Some(c)) = (pair[0].as_u64(), pair[1].as_u64()) {
                            set.insert((r as u32, c as u32));
                        }
                    }
                }
            }
        }
        self.axis_shifts.clear();
        if let Some(list) = v.get("axisShifts").and_then(|x| x.as_array()) {
            for entry in list {
                let Some(obj) = entry.as_object() else {
                    continue;
                };
                let Some(sheet) = obj.get("sheet").and_then(|x| x.as_u64()) else {
                    continue;
                };
                let Some(at) = obj.get("at").and_then(|x| x.as_u64()) else {
                    continue;
                };
                let Some(count) = obj.get("count").and_then(|x| x.as_i64()) else {
                    continue;
                };
                self.axis_shifts.push(AxisShiftRecord {
                    sheet: sheet as u32,
                    row: obj.get("row").and_then(|x| x.as_bool()).unwrap_or(true),
                    at: at as u32,
                    count: count as i32,
                });
            }
        }
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

fn view_to_snap(view: &SheetView) -> Value {
    let mut m = Map::new();
    m.insert("showGridLines".into(), Value::Bool(view.show_grid_lines));
    m.insert("showHeaders".into(), Value::Bool(view.show_headers));
    m.insert("showZeros".into(), Value::Bool(view.show_zeros));
    m.insert("rightToLeft".into(), Value::Bool(view.right_to_left));
    m.insert("zoom".into(), crate::model::js_num(view.zoom));
    if let Some(freeze) = &view.freeze {
        m.insert("freeze".into(), freeze.to_json());
    }
    if let Some(top) = &view.top_left {
        m.insert("topLeft".into(), json!({ "row": top.row, "col": top.col }));
    }
    if let Some(selection) = &view.selection {
        m.insert(
            "selection".into(),
            json!({
                "active": { "row": selection.active.row, "col": selection.active.col },
                "anchor": { "row": selection.anchor.row, "col": selection.anchor.col },
                "ranges": selection.ranges.iter().map(CellRange::to_json).collect::<Vec<_>>(),
            }),
        );
    }
    if let Some(show) = view.show_formulas {
        m.insert("showFormulas".into(), Value::Bool(show));
    }
    Value::Object(m)
}

fn view_from_snap(v: &Value) -> Result<SheetView, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "view must be an object".to_owned())?;
    let mut view = crate::sheet_props::default_sheet_view();
    if let Some(b) = obj.get("showGridLines").and_then(|x| x.as_bool()) {
        view.show_grid_lines = b;
    }
    if let Some(b) = obj.get("showHeaders").and_then(|x| x.as_bool()) {
        view.show_headers = b;
    }
    if let Some(b) = obj.get("showZeros").and_then(|x| x.as_bool()) {
        view.show_zeros = b;
    }
    if let Some(b) = obj.get("rightToLeft").and_then(|x| x.as_bool()) {
        view.right_to_left = b;
    }
    if let Some(z) = obj.get("zoom").and_then(|x| x.as_f64()) {
        view.zoom = z;
    }
    if let Some(freeze) = obj.get("freeze") {
        let f = freeze
            .as_object()
            .ok_or_else(|| "freeze must be an object".to_owned())?;
        view.freeze = Some(FreezePane {
            rows: f.get("rows").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            cols: f.get("cols").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
        });
    }
    if let Some(top) = obj.get("topLeft") {
        view.top_left = Some(cell_address_from_json(top)?);
    }
    if let Some(selection) = obj.get("selection") {
        view.selection = Some(selection_from_snap(selection)?);
    }
    if let Some(show) = obj.get("showFormulas").and_then(|x| x.as_bool()) {
        view.show_formulas = Some(show);
    }
    Ok(view)
}

fn selection_from_snap(v: &Value) -> Result<SheetSelection, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "selection must be an object".to_owned())?;
    let active = cell_address_from_json(obj.get("active").ok_or("selection missing active")?)?;
    let anchor = match obj.get("anchor") {
        Some(anchor) => cell_address_from_json(anchor)?,
        None => active,
    };
    let ranges = obj
        .get("ranges")
        .and_then(|x| x.as_array())
        .ok_or_else(|| "selection missing ranges".to_owned())?;
    Ok(SheetSelection {
        active,
        anchor,
        ranges: ranges
            .iter()
            .map(cell_range_from_json)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn hyperlink_to_snap(link: &HyperlinkInfo) -> Value {
    let mut m = Map::new();
    m.insert("range".into(), link.range.to_json());
    if let Some(tooltip) = &link.tooltip {
        m.insert("tooltip".into(), Value::String(tooltip.clone()));
    }
    if let Some(target) = &link.target {
        m.insert("target".into(), Value::String(target.clone()));
    }
    if let Some(location) = &link.location {
        m.insert("location".into(), Value::String(location.clone()));
    }
    Value::Object(m)
}

fn hyperlink_from_snap(v: &Value) -> Result<HyperlinkInfo, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "hyperlink must be an object".to_owned())?;
    Ok(HyperlinkInfo {
        range: cell_range_from_json(obj.get("range").ok_or("hyperlink missing range")?)?,
        tooltip: obj
            .get("tooltip")
            .and_then(|x| x.as_str())
            .map(str::to_owned),
        target: obj
            .get("target")
            .and_then(|x| x.as_str())
            .map(str::to_owned),
        location: obj
            .get("location")
            .and_then(|x| x.as_str())
            .map(str::to_owned),
    })
}

fn validation_to_snap(rule: &DataValidation) -> Value {
    let mut m = Map::new();
    m.insert(
        "ranges".into(),
        Value::Array(rule.ranges.iter().map(CellRange::to_json).collect()),
    );
    m.insert("type".into(), Value::String(rule.validation_type.clone()));
    if let Some(show) = rule.show_drop_down {
        m.insert("showDropDown".into(), Value::Bool(show));
    }
    Value::Object(m)
}

fn validation_from_snap(v: &Value) -> Result<DataValidation, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "data validation must be an object".to_owned())?;
    let ranges = obj
        .get("ranges")
        .and_then(|x| x.as_array())
        .ok_or_else(|| "validation missing ranges".to_owned())?;
    Ok(DataValidation {
        ranges: ranges
            .iter()
            .map(cell_range_from_json)
            .collect::<Result<Vec<_>, _>>()?,
        validation_type: obj
            .get("type")
            .and_then(|x| x.as_str())
            .unwrap_or("none")
            .to_owned(),
        show_drop_down: obj.get("showDropDown").and_then(|x| x.as_bool()),
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

fn defined_name_from_json(v: &Value) -> Result<DefinedName, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "defined name must be an object".to_owned())?;
    Ok(DefinedName {
        name: obj
            .get("name")
            .and_then(|x| x.as_str())
            .ok_or("defined name missing name")?
            .to_owned(),
        formula: obj
            .get("formula")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_owned(),
        local_sheet: obj
            .get("localSheet")
            .and_then(|x| x.as_u64())
            .map(|n| n as u32),
        hidden: obj.get("hidden").and_then(|x| x.as_bool()),
        comment: obj
            .get("comment")
            .and_then(|x| x.as_str())
            .map(str::to_owned),
    })
}

fn column_info_from_json(v: &Value) -> Result<ColumnInfo, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "column must be an object".to_owned())?;
    Ok(ColumnInfo {
        min: obj.get("min").and_then(|x| x.as_u64()).ok_or("col min")? as u32,
        max: obj.get("max").and_then(|x| x.as_u64()).ok_or("col max")? as u32,
        width: obj.get("width").and_then(|x| x.as_f64()),
        custom_width: obj.get("customWidth").and_then(|x| x.as_bool()),
        hidden: obj.get("hidden").and_then(|x| x.as_bool()),
        style_id: obj
            .get("styleId")
            .and_then(|x| x.as_u64())
            .map(|n| n as u32),
        outline_level: obj.get("outlineLevel").and_then(|x| x.as_f64()),
        collapsed: obj.get("collapsed").and_then(|x| x.as_bool()),
        best_fit: obj.get("bestFit").and_then(|x| x.as_bool()),
    })
}

fn row_info_from_json(v: &Value) -> Result<(u32, RowInfo), String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "rowInfo must be an object".to_owned())?;
    let row = obj
        .get("row")
        .and_then(|x| x.as_u64())
        .ok_or("rowInfo missing row")? as u32;
    Ok((
        row,
        RowInfo {
            height: obj.get("height").and_then(|x| x.as_f64()),
            custom_height: obj.get("customHeight").and_then(|x| x.as_bool()),
            hidden: obj.get("hidden").and_then(|x| x.as_bool()),
            style_id: obj
                .get("styleId")
                .and_then(|x| x.as_u64())
                .map(|n| n as u32),
            outline_level: obj.get("outlineLevel").and_then(|x| x.as_f64()),
            collapsed: obj.get("collapsed").and_then(|x| x.as_bool()),
        },
    ))
}
