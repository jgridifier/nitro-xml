// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/sheet-data.ts; changes listed in PROVENANCE.md.
//! Read `<sheetData>` into the columnar cell store.

use std::collections::{HashMap, HashSet};

use nitro_xml_opc::xml::{parse_xml_str, Element};

use crate::address::{parse_address, parse_range};
use crate::cells::{CellStore, CellTypeTag};
use crate::dates::iso_to_serial;
use crate::formula_text::strip_future_prefixes;
use crate::formula_translate::translate_formula;
use crate::model::{CellAddress, CellRange, DumpCell, DumpCellValue, RowInfo, SharedString};
use crate::shared_strings::parse_rich_string;
use crate::xml_util::{att, num_attr, x_children, x_first, x_text};

pub const ERROR_CODES: &[&str] = &[
    "#NULL!",
    "#DIV/0!",
    "#VALUE!",
    "#REF!",
    "#NAME?",
    "#NUM!",
    "#N/A",
    "#GETTING_DATA",
    "#SPILL!",
    "#CALC!",
];

pub fn is_error_code(value: &str) -> bool {
    ERROR_CODES.contains(&value)
}

pub struct CellContext<'a> {
    pub shared_strings: &'a [SharedString],
    pub xf_map: &'a [u32],
    pub date1904: bool,
    pub palette: Option<&'a [String]>,
    pub dynamic_cells: &'a HashSet<u32>,
    pub warnings: &'a mut Vec<String>,
}

impl<'a> CellContext<'a> {
    pub fn warn(&mut self, message: impl AsRef<str>) {
        let message = message.as_ref().to_owned();
        if !self.warnings.contains(&message) {
            self.warnings.push(message);
        }
    }
}

struct SharedMaster {
    row: u32,
    col: u32,
    formula: String,
}

struct DynamicAnchor {
    row: u32,
    col: u32,
    range: CellRange,
}

fn style_of(ctx: &CellContext<'_>, element: Option<&Element>) -> u32 {
    let s = num_attr(element, "s");
    match s {
        None => 0,
        Some(s) => ctx.xf_map.get(s as usize).copied().unwrap_or(0),
    }
}

/// Stream-parse inner `sheetData` XML (rows and cells only).
pub fn read_sheet_data_xml(
    ctx: &mut CellContext<'_>,
    sheet_data_inner: &str,
    store: &mut CellStore,
    row_info: &mut HashMap<u32, RowInfo>,
) {
    // Lazy viewport rows keep `x14ac:dyDescent` (and similar) without the outer worksheet
    // xmlns declarations, so the wrapper must re-declare x14ac/mc or parsing drops cells.
    let wrapped = format!(
        r#"<sheetData xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:x14ac="http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">{}</sheetData>"#,
        sheet_data_inner
    );
    let doc = match parse_xml_str(&wrapped) {
        Ok(d) => d,
        Err(_) => return,
    };
    read_sheet_data_element(ctx, &doc.root, store, row_info);
}

pub fn read_sheet_data_element(
    ctx: &mut CellContext<'_>,
    sheet_data: &Element,
    store: &mut CellStore,
    row_info: &mut HashMap<u32, RowInfo>,
) {
    let mut masters: HashMap<String, SharedMaster> = HashMap::new();
    let mut anchors: Vec<DynamicAnchor> = Vec::new();
    let mut row_index: i64 = -1;
    for row_el in x_children(sheet_data, "row") {
        let r = num_attr(Some(row_el), "r");
        row_index = if let Some(r) = r {
            r as i64 - 1
        } else {
            row_index + 1
        };
        let row_u = row_index as u32;
        if let Some(info) = crate::sheet_props::read_row_info(row_el, ctx.xf_map) {
            row_info.insert(row_u, info);
        }
        let mut col_index: i64 = -1;
        for c in x_children(row_el, "c") {
            let ref_ = att(Some(c), "r");
            let address = ref_.and_then(parse_address);
            col_index = if let Some(ref a) = address {
                a.col as i64
            } else {
                col_index + 1
            };
            let row = address.as_ref().map(|a| a.row).unwrap_or(row_u);
            let col = col_index as u32;
            let style_id = style_of(ctx, Some(c));
            let mut cell_idx = None;
            read_value(ctx, c, store, style_id, row, col, &mut cell_idx);
            let f = x_first(Some(c), "f");
            let cm = num_attr(Some(c), "cm");
            let dynamic = cm
                .map(|c| ctx.dynamic_cells.contains(&(c as u32)))
                .unwrap_or(false);
            if let Some(cm) = cm {
                if !dynamic {
                    if let Some(idx) = cell_idx {
                        store.set_cell_metadata(idx, cm as u32);
                    }
                }
            }
            let vm = num_attr(Some(c), "vm");
            if let Some(vm) = vm {
                if let Some(idx) = cell_idx {
                    store.set_value_metadata(idx, vm as u32);
                }
            }
            if let Some(f_el) = f {
                read_formula(
                    ctx,
                    f_el,
                    row,
                    col,
                    style_id,
                    store,
                    cell_idx,
                    &mut masters,
                    dynamic,
                    &mut anchors,
                );
            }
            if let Some(idx) = cell_idx {
                if store.formula(idx).is_none() {
                    store.clear_legacy_dynamic(idx);
                }
            }
        }
    }
    mark_spills(store, &anchors);
}

fn read_value(
    ctx: &mut CellContext<'_>,
    c: &Element,
    store: &mut CellStore,
    style_id: u32,
    row: u32,
    col: u32,
    cell_idx: &mut Option<usize>,
) {
    let type_ = att(Some(c), "t").unwrap_or("n");
    let v = x_first(Some(c), "v");
    let raw = v.map(|el| el.text_content());
    match type_ {
        "s" => {
            let idx = raw.as_deref().and_then(|r| r.parse::<usize>().ok());
            let entry = idx.and_then(|i| ctx.shared_strings.get(i));
            if entry.is_none() {
                if raw.is_some() {
                    ctx.warn("A cell refers to a missing shared string.");
                }
                return;
            }
            let sst_idx = idx.unwrap() as u32;
            let entry = entry.unwrap();
            let i = store.push_shared(row, col, style_id, sst_idx);
            if let Some(runs) = &entry.runs {
                store.set_rich_text(i, runs.clone());
            }
            *cell_idx = Some(i);
        }
        "inlineStr" => {
            let is = x_first(Some(c), "is");
            if is.is_none() {
                return;
            }
            let entry = parse_rich_string(is.unwrap(), ctx.palette);
            let i = store.push_string(row, col, style_id, entry.text.clone());
            if let Some(runs) = entry.runs {
                store.set_rich_text(i, runs);
            }
            *cell_idx = Some(i);
        }
        "str" => {
            if let Some(v) = v {
                let i = store.push_string(row, col, style_id, x_text(Some(v)));
                *cell_idx = Some(i);
            }
        }
        "b" => {
            if let Some(raw) = raw {
                let b = raw.trim() == "1" || raw.trim().eq_ignore_ascii_case("true");
                let i = store.push_bool(row, col, style_id, b);
                *cell_idx = Some(i);
            }
        }
        "e" => {
            let code = raw.as_deref().unwrap_or("").trim();
            if raw.is_some() {
                let err = if is_error_code(code) {
                    code.to_owned()
                } else {
                    "#VALUE!".to_owned()
                };
                let i = store.push_error(row, col, style_id, err);
                *cell_idx = Some(i);
            }
        }
        "d" => {
            if let Some(raw) = raw {
                if let Some(serial) = iso_to_serial(raw.as_str(), ctx.date1904) {
                    let i = store.push_number(row, col, style_id, serial, Some(raw.as_str()));
                    *cell_idx = Some(i);
                } else {
                    ctx.warn(format!("Unreadable date cell value \"{raw}\"."));
                }
            }
        }
        _ => {
            if let Some(raw) = raw {
                if raw.trim().is_empty() {
                    return;
                }
                if let Ok(n) = raw.parse::<f64>() {
                    if n.is_finite() {
                        let i = store.push_number(row, col, style_id, n, Some(raw.as_str()));
                        *cell_idx = Some(i);
                    } else {
                        let i = store.push_string(row, col, style_id, raw);
                        *cell_idx = Some(i);
                    }
                } else {
                    let i = store.push_string(row, col, style_id, raw);
                    *cell_idx = Some(i);
                }
            }
        }
    }
    if cell_idx.is_none() && style_id > 0 {
        *cell_idx = Some(store.push_style_only(row, col, style_id));
    }
}

#[allow(clippy::too_many_arguments)]
fn read_formula(
    ctx: &mut CellContext<'_>,
    f: &Element,
    row: u32,
    col: u32,
    style_id: u32,
    store: &mut CellStore,
    cell_idx: Option<usize>,
    masters: &mut HashMap<String, SharedMaster>,
    dynamic: bool,
    anchors: &mut Vec<DynamicAnchor>,
) {
    let kind = att(Some(f), "t").unwrap_or("normal");
    let mut cell_idx = cell_idx;
    let text = strip_future_prefixes(&f.text_content());
    if cell_idx.is_none() && (!text.trim().is_empty() || kind != "normal") {
        cell_idx = Some(store.push_style_only(row, col, style_id));
    }
    if let Some(idx) = cell_idx {
        if dynamic {
            store.set_dynamic_array(idx);
        } else if kind != "array" {
            store.set_legacy_formula(idx);
        }
    }
    if kind == "shared" {
        let si = att(Some(f), "si").unwrap_or("");
        if !text.trim().is_empty() {
            masters.insert(
                si.to_owned(),
                SharedMaster {
                    row,
                    col,
                    formula: text.clone(),
                },
            );
            if let Some(idx) = cell_idx {
                store.set_formula(idx, text);
            }
            return;
        }
        let master = masters.get(si);
        if master.is_none() {
            ctx.warn("A shared formula refers to a missing master cell; its formula was dropped.");
            return;
        }
        let master = master.unwrap();
        let translated = translate_formula(
            &master.formula,
            (row - master.row) as i32,
            (col - master.col) as i32,
        );
        if let Some(idx) = cell_idx {
            store.set_formula(idx, translated);
        }
        return;
    }
    if kind == "dataTable" {
        ctx.warn("What-if data table formulas are not supported; their cached values were kept.");
        if let Some(idx) = cell_idx {
            store.clear_legacy_dynamic(idx);
        }
        return;
    }
    if text.trim().is_empty() {
        return;
    }
    if let Some(idx) = cell_idx {
        store.set_formula(idx, text);
    }
    if kind == "array" {
        let range = att(Some(f), "ref").and_then(parse_range);
        if dynamic {
            if let Some(range) = range {
                anchors.push(DynamicAnchor { row, col, range });
            }
        } else if let (Some(idx), Some(range)) = (cell_idx, range) {
            store.set_array_range(idx, range);
        }
    }
}

fn mark_spills(store: &mut CellStore, anchors: &[DynamicAnchor]) {
    for anchor in anchors {
        let start = &anchor.range.start;
        let end = &anchor.range.end;
        for r in start.row..=end.row {
            for c in start.col..=end.col {
                if r == anchor.row && c == anchor.col {
                    continue;
                }
                for i in 0..store.len() {
                    if store.row(i) == r && store.col(i) == c {
                        if store.formula(i).is_some() {
                            continue;
                        }
                        store.set_spill_anchor(
                            i,
                            CellAddress {
                                row: anchor.row,
                                col: anchor.col,
                            },
                        );
                    }
                }
            }
        }
    }
}

pub fn store_to_dump_cells(store: &CellStore) -> Vec<DumpCell> {
    store_to_dump_cells_with_sst(store, &[])
}

/// Materialize dump cells. Shared-string indices resolve through `sst` once (CellStore keeps indices).
pub fn store_to_dump_cells_with_sst(store: &CellStore, sst: &[SharedString]) -> Vec<DumpCell> {
    let mut out = Vec::new();
    for i in store.iter_sorted() {
        let value = match store.cell_type(i) {
            CellTypeTag::Empty => DumpCellValue::Null,
            CellTypeTag::Number => DumpCellValue::Number(store.number(i)),
            CellTypeTag::String => DumpCellValue::String(store.string(i).to_owned()),
            CellTypeTag::Shared => match sst.get(store.shared_index(i) as usize) {
                Some(entry) => DumpCellValue::String(entry.text.clone()),
                None => DumpCellValue::Null,
            },
            CellTypeTag::Bool => DumpCellValue::Bool(store.bool_val(i)),
            CellTypeTag::Error => DumpCellValue::Error(store.error(i).to_owned()),
        };
        let type_tag = match store.cell_type(i) {
            CellTypeTag::String | CellTypeTag::Shared => Some("s"),
            CellTypeTag::Bool => Some("b"),
            CellTypeTag::Error => Some("e"),
            _ => None,
        };
        out.push(DumpCell {
            row: store.row(i),
            col: store.col(i),
            value,
            type_tag,
            formula: store.formula(i).map(str::to_owned),
            style_id: if store.style(i) > 0 {
                Some(store.style(i))
            } else {
                None
            },
            rich_text: store.rich_text(i).map(|r| r.to_vec()),
            array_range: store.array_range(i).cloned(),
            legacy_formula: store.legacy_formula(i).then_some(true),
            dynamic_array: store.dynamic_array(i).then_some(true),
            spill_anchor: store.spill_anchor(i).cloned(),
            cell_metadata: store.cell_metadata(i),
            value_metadata: store.value_metadata(i),
        });
    }
    out
}
