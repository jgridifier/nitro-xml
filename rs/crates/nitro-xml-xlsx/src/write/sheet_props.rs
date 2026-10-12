// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/write/sheet-props.ts
// (dimension, sheetViews, sheetFormat, cols, mergeCells); changes listed in PROVENANCE.md.
//! Worksheet shell fragments the simple edit fixtures need.

use crate::address::{format_address, format_range};
use crate::model::{ActiveWorksheet, CellAddress, CellRange, SheetView};

use super::xml_out::{attr_bool, attr_f64, attr_str, attr_u32, el};

pub fn dimension_xml(sheet: &ActiveWorksheet) -> String {
    let mut min_row = u32::MAX;
    let mut max_row = 0u32;
    let mut min_col = u32::MAX;
    let mut max_col = 0u32;
    let mut any = false;
    for cell in &sheet.cells {
        any = true;
        min_row = min_row.min(cell.row);
        max_row = max_row.max(cell.row);
        min_col = min_col.min(cell.col);
        max_col = max_col.max(cell.col);
    }
    let range = if any {
        format_range(&CellRange {
            start: CellAddress {
                row: min_row,
                col: min_col,
            },
            end: CellAddress {
                row: max_row,
                col: max_col,
            },
        })
    } else {
        "A1".into()
    };
    format!("<dimension ref=\"{range}\"/>")
}

pub fn sheet_views_xml(sheet: &ActiveWorksheet, tab_selected: bool) -> String {
    let view = &sheet.view;
    let (inner, pane) = pane_and_selection(view);
    let attrs = format!(
        "{}{}{}{}{}{}{}{}{}",
        attr_bool("tabSelected", tab_selected.then_some(true)),
        attr_bool("showGridLines", (!view.show_grid_lines).then_some(false)),
        attr_bool("showRowColHeaders", (!view.show_headers).then_some(false)),
        attr_bool("showZeros", (!view.show_zeros).then_some(false)),
        attr_bool("rightToLeft", view.right_to_left.then_some(true)),
        attr_str(
            "topLeftCell",
            view.freeze
                .is_none()
                .then_some(view.top_left.as_ref())
                .flatten()
                .map(format_address)
                .as_deref(),
        ),
        attr_bool("showFormulas", view.show_formulas.filter(|v| *v)),
        zoom_attrs(view.zoom),
        attr_u32("workbookViewId", Some(0)),
    );
    let _ = pane;
    let view_el = if inner.is_empty() {
        format!("<sheetView{attrs}/>")
    } else {
        format!("<sheetView{attrs}>{inner}</sheetView>")
    };
    format!("<sheetViews>{view_el}</sheetViews>")
}

fn zoom_attrs(zoom: f64) -> String {
    if (zoom - 100.0).abs() < f64::EPSILON {
        String::new()
    } else {
        format!(
            "{}{}",
            attr_f64("zoomScale", Some(zoom)),
            attr_f64("zoomScaleNormal", Some(zoom))
        )
    }
}

fn pane_and_selection(view: &SheetView) -> (String, Option<&'static str>) {
    let mut inner = String::new();
    let mut pane = None;
    if let Some(freeze) = &view.freeze {
        if freeze.rows > 0 || freeze.cols > 0 {
            let pane_name = if freeze.rows > 0 && freeze.cols > 0 {
                "bottomRight"
            } else if freeze.rows > 0 {
                "bottomLeft"
            } else {
                "topRight"
            };
            pane = Some(pane_name);
            let top_row = view
                .top_left
                .as_ref()
                .map(|addr| addr.row)
                .unwrap_or(freeze.rows)
                .max(freeze.rows);
            let top_col = view
                .top_left
                .as_ref()
                .map(|addr| addr.col)
                .unwrap_or(freeze.cols)
                .max(freeze.cols);
            let attrs = format!(
                "{}{}{}{}{}",
                attr_u32("xSplit", (freeze.cols > 0).then_some(freeze.cols)),
                attr_u32("ySplit", (freeze.rows > 0).then_some(freeze.rows)),
                attr_str(
                    "topLeftCell",
                    Some(&format_address(&CellAddress {
                        row: top_row,
                        col: top_col,
                    }))
                ),
                attr_str("activePane", Some(pane_name)),
                attr_str("state", Some("frozen")),
            );
            inner.push_str(&el("pane", &attrs, ""));
        }
    }
    if let Some(selection) = &view.selection {
        let sqref = if selection.ranges.is_empty() {
            format_address(&selection.active)
        } else {
            selection
                .ranges
                .iter()
                .map(format_range)
                .collect::<Vec<_>>()
                .join(" ")
        };
        let attrs = format!(
            "{}{}{}",
            attr_str("pane", pane),
            attr_str("activeCell", Some(&format_address(&selection.active))),
            attr_str("sqref", Some(&sqref)),
        );
        inner.push_str(&el("selection", &attrs, ""));
    }
    (inner, pane)
}

pub fn sheet_format_xml(sheet: &ActiveWorksheet) -> String {
    let format = sheet.format.as_ref();
    let row_level = sheet
        .row_info
        .values()
        .filter_map(|info| info.outline_level)
        .fold(0.0_f64, f64::max);
    let col_level = sheet
        .columns
        .iter()
        .filter_map(|col| col.outline_level)
        .fold(0.0_f64, f64::max);
    let custom_height = format.and_then(|f| f.custom_height).or(
        if (sheet.default_row_height - 15.0).abs() > f64::EPSILON {
            Some(true)
        } else {
            None
        },
    );
    let attrs = format!(
        "{}{}{}{}{}{}{}{}{}",
        attr_f64("baseColWidth", format.and_then(|f| f.base_col_width)),
        attr_f64("defaultColWidth", sheet.default_col_width),
        attr_f64("defaultRowHeight", Some(sheet.default_row_height)),
        attr_bool("customHeight", custom_height),
        attr_bool(
            "zeroHeight",
            format.and_then(|f| f.zero_height).filter(|v| *v)
        ),
        attr_bool("thickTop", format.and_then(|f| f.thick_top).filter(|v| *v)),
        attr_bool(
            "thickBottom",
            format.and_then(|f| f.thick_bottom).filter(|v| *v)
        ),
        attr_f64("outlineLevelRow", (row_level > 0.0).then_some(row_level)),
        attr_f64("outlineLevelCol", (col_level > 0.0).then_some(col_level)),
    );
    format!("<sheetFormatPr{attrs}/>")
}

pub fn cols_xml(sheet: &ActiveWorksheet, style_count: usize) -> String {
    let mut sorted = sheet.columns.clone();
    sorted.sort_by_key(|col| col.min);
    let mut previous = -1i64;
    let mut out = String::new();
    for column in sorted {
        let min = column.min.max((previous + 1) as u32);
        if column.max < min {
            continue;
        }
        previous = i64::from(column.max);
        let style = column
            .style_id
            .filter(|id| (*id as usize) < style_count && *id > 0);
        let attrs = format!(
            "{}{}{}{}{}{}{}{}{}",
            attr_u32("min", Some(min + 1)),
            attr_u32("max", Some(column.max + 1)),
            attr_f64("width", column.width),
            attr_u32("style", style),
            attr_bool("hidden", column.hidden.filter(|v| *v)),
            attr_bool("bestFit", column.best_fit.filter(|v| *v)),
            attr_bool("customWidth", column.custom_width.filter(|v| *v)),
            attr_f64("outlineLevel", column.outline_level.filter(|v| *v != 0.0)),
            attr_bool("collapsed", column.collapsed.filter(|v| *v)),
        );
        out.push_str(&el("col", &attrs, ""));
    }
    if out.is_empty() {
        String::new()
    } else {
        format!("<cols>{out}</cols>")
    }
}

pub fn merge_cells_xml(sheet: &ActiveWorksheet) -> String {
    if sheet.merges.is_empty() {
        return String::new();
    }
    let cells: String = sheet
        .merges
        .iter()
        .map(|range| format!("<mergeCell ref=\"{}\"/>", format_range(range)))
        .collect();
    format!(
        "<mergeCells count=\"{}\">{cells}</mergeCells>",
        sheet.merges.len()
    )
}
