// SPDX-License-Identifier: MIT
//! View-IR dump (`CellItem` list) matching `parity/oracle/xlsx.ts` `dumpXlsxViewIr`.

use serde_json::{json, Value};

use crate::model::{stringify_json, ActiveWorksheet, WorkbookSlice};
use crate::sheet_access::{used_range, CellIndex};
use crate::view::cell_items::{build_items, BuildInput};
use crate::view::metrics::{create_grid_metrics, GridMetricsOptions};
use crate::view::text_measure::FakeTextMeasure;
use crate::view::types::CellItem;

pub fn dump_xlsx_view_ir(
    workbook: &WorkbookSlice,
    worksheets: &[Option<ActiveWorksheet>],
) -> Value {
    let sheets: Vec<Value> = workbook
        .sheets
        .iter()
        .enumerate()
        .map(|(sheet_index, stub)| {
            let sheet = worksheets.get(sheet_index).and_then(|o| o.as_ref());
            if let Some(sheet) = sheet {
                sheet_view_ir(workbook, worksheets, sheet, sheet_index)
            } else {
                json!({
                    "name": stub.name,
                    "sheetIndex": sheet_index,
                    "lastRow": 0,
                    "lastCol": 0,
                    "items": [],
                })
            }
        })
        .collect();
    json!({
        "format": "xlsx",
        "sheets": sheets,
    })
}

pub fn dump_xlsx_view_ir_string(
    workbook: &WorkbookSlice,
    worksheets: &[Option<ActiveWorksheet>],
) -> String {
    stringify_json(&dump_xlsx_view_ir(workbook, worksheets))
}

fn sheet_view_ir(
    workbook: &WorkbookSlice,
    all_sheets: &[Option<ActiveWorksheet>],
    sheet: &ActiveWorksheet,
    sheet_index: usize,
) -> Value {
    let metrics = create_grid_metrics(sheet, GridMetricsOptions::default());
    let used = used_range(sheet);
    let last_row = metrics
        .last_row
        .min(used.as_ref().map(|u| u.end.row + 5).unwrap_or(40))
        .min(200);
    let last_col = metrics
        .last_col
        .min(used.as_ref().map(|u| u.end.col + 2).unwrap_or(20))
        .min(60);
    let rows: Vec<u32> = (0..=last_row).collect();
    let cols: Vec<u32> = (0..=last_col).collect();
    let index = CellIndex::build(sheet);
    let zoom = metrics.zoom / 100.0;
    let mut measure = FakeTextMeasure;
    let has_formula = |row: u32, col: u32| {
        index
            .get(sheet, row, col)
            .and_then(|c| c.formula.as_ref())
            .is_some()
    };
    let input = BuildInput {
        workbook,
        sheet,
        sheet_index,
        all_sheets,
        index: &index,
        metrics: &metrics,
        zoom,
        rows: &rows,
        cols: &cols,
        has_formula: &has_formula,
        enable_number_fit: false,
        skip_blank_items: false,
    };
    let items: Vec<Value> = build_items(&input, &mut measure)
        .iter()
        .map(CellItem::to_json)
        .collect();
    json!({
        "name": sheet.name,
        "sheetIndex": sheet_index,
        "lastRow": last_row,
        "lastCol": last_col,
        "items": items,
    })
}
