// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/ui/grid/number-fit.ts; changes listed in PROVENANCE.md.

use crate::model::{ActiveWorksheet, WorkbookSlice};
use crate::numfmt::{format_value, FormatOptions};
use crate::sheet_access::{dump_value_to_cell_value, get_cell, CellIndex};
use crate::styles::style_at;
use crate::view::cell_view::effective_style_id;

pub fn fit_number(
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    row: u32,
    col: u32,
    width: f64,
    measure: &dyn Fn(&str) -> f64,
) -> Option<String> {
    let cell = get_cell(sheet, index, row, col)?;
    let cv = dump_value_to_cell_value(&cell.value);
    if !matches!(cv, crate::numfmt::CellValue::Number(_)) {
        return None;
    }
    let num_fmt = style_at(
        &workbook.styles,
        Some(effective_style_id(sheet, index, row, col)),
    )
    .num_fmt
    .clone();
    let fmt = if num_fmt.is_empty() {
        "General"
    } else {
        &num_fmt
    };
    Some(
        format_value(
            &cv,
            fmt,
            &FormatOptions {
                date1904: workbook.date1904,
                width: Some(width),
                measure: Some(measure),
            },
        )
        .text,
    )
}
