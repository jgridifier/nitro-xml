// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/sheet-props.ts; changes listed in
// PROVENANCE.md.
//! Worksheet view, column widths, and format properties.

use nitro_xml_opc::xml::Element;

use crate::address::parse_address;
use crate::address::parse_range;
use crate::model::{
    CellAddress, CellRange, ColumnInfo, FreezePane, RowInfo, SheetFormat, SheetView,
};
use crate::xml_util::{att, bool_attr, bool_attr_or, num_attr, x_children, x_first};

pub fn default_sheet_view() -> SheetView {
    SheetView {
        show_grid_lines: true,
        show_headers: true,
        show_zeros: true,
        right_to_left: false,
        zoom: 100.0,
        freeze: None,
        top_left: None,
        selection: None,
        show_formulas: None,
    }
}

use crate::model::DataValidation;

pub fn read_data_validations(node: Option<&Element>) -> Vec<DataValidation> {
    let Some(node) = node else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for dv in x_children(node, "dataValidation") {
        let ranges = parse_sqref(att(Some(dv), "sqref").unwrap_or(""));
        if ranges.is_empty() {
            continue;
        }
        let validation_type = att(Some(dv), "type").unwrap_or("none").to_string();
        let hide_drop_down = bool_attr(Some(dv), "showDropDown") == Some(true);
        let show_drop_down = if validation_type == "list" {
            Some(!hide_drop_down)
        } else if hide_drop_down {
            Some(false)
        } else {
            None
        };
        out.push(DataValidation {
            ranges,
            validation_type,
            show_drop_down,
        });
    }
    out
}

pub fn parse_sqref(sqref: &str) -> Vec<CellRange> {
    sqref
        .split_whitespace()
        .filter(|p| !p.is_empty())
        .filter_map(parse_range)
        .collect()
}

pub fn read_sheet_view(sheet_views: Option<&Element>) -> SheetView {
    let mut view = default_sheet_view();
    let node = x_first(sheet_views, "sheetView");
    let Some(node) = node else { return view };
    view.show_grid_lines = bool_attr_or(Some(node), "showGridLines", true);
    view.show_headers = bool_attr_or(Some(node), "showRowColHeaders", true);
    view.show_zeros = bool_attr_or(Some(node), "showZeros", true);
    view.right_to_left = bool_attr_or(Some(node), "rightToLeft", false);
    view.zoom = num_attr(Some(node), "zoomScale").unwrap_or(100.0);
    if bool_attr(Some(node), "showFormulas") == Some(true) {
        view.show_formulas = Some(true);
    }
    let pane = x_first(Some(node), "pane");
    let state = att(pane, "state");
    let frozen = pane.is_some() && (state == Some("frozen") || state == Some("frozenSplit"));
    if frozen {
        let rows = num_attr(pane, "ySplit").unwrap_or(0.0).round().max(0.0) as u32;
        let cols = num_attr(pane, "xSplit").unwrap_or(0.0).round().max(0.0) as u32;
        if rows > 0 || cols > 0 {
            view.freeze = Some(FreezePane { rows, cols });
        }
    }
    let top_left_ref = if frozen {
        att(pane, "topLeftCell")
    } else {
        att(Some(node), "topLeftCell")
    };
    if let Some(addr) = top_left_ref.and_then(parse_address) {
        view.top_left = Some(addr);
    }
    let selections = x_children(node, "selection");
    let active_pane = att(pane, "activePane").unwrap_or("topLeft");
    let selection = selections
        .iter()
        .find(|sel| att(Some(*sel), "pane").unwrap_or("topLeft") == active_pane)
        .or_else(|| selections.first());
    if let Some(selection) = selection {
        let sq = att(Some(selection), "sqref")
            .or_else(|| att(Some(selection), "activeCell"))
            .unwrap_or("A1");
        let ranges = parse_sqref(sq);
        let active = att(Some(selection), "activeCell")
            .and_then(parse_address)
            .or_else(|| ranges.first().map(|r| r.start))
            .unwrap_or(CellAddress { row: 0, col: 0 });
        view.selection = Some(crate::model::SheetSelection {
            active,
            ranges: if ranges.is_empty() {
                vec![CellRange {
                    start: active,
                    end: active,
                }]
            } else {
                ranges
            },
        });
    }
    view
}

pub fn read_sheet_format(format: Option<&Element>) -> (f64, Option<f64>, Option<SheetFormat>) {
    let default_row_height = num_attr(format, "defaultRowHeight").unwrap_or(15.0);
    let default_col_width = num_attr(format, "defaultColWidth");
    let mut flags = SheetFormat::default();
    for key in ["zeroHeight", "customHeight", "thickTop", "thickBottom"] {
        if let Some(v) = bool_attr(format, key) {
            match key {
                "zeroHeight" => flags.zero_height = Some(v),
                "customHeight" => flags.custom_height = Some(v),
                "thickTop" => flags.thick_top = Some(v),
                "thickBottom" => flags.thick_bottom = Some(v),
                _ => {}
            }
        }
    }
    if let Some(base) = num_attr(format, "baseColWidth") {
        flags.base_col_width = Some(base);
    }
    let format_out = if flags == SheetFormat::default() {
        None
    } else {
        Some(flags)
    };
    (default_row_height, default_col_width, format_out)
}

pub fn read_columns(cols: Option<&Element>, xf_map: &[u32]) -> Vec<ColumnInfo> {
    let mut out = Vec::new();
    for col in cols.map(|c| x_children(c, "col")).unwrap_or_default() {
        let min = (num_attr(Some(col), "min").unwrap_or(1.0) as u32) - 1;
        let max = (num_attr(Some(col), "max").unwrap_or(min as f64 + 1.0) as u32) - 1;
        let mut info = ColumnInfo {
            min,
            max,
            width: None,
            custom_width: None,
            hidden: None,
            style_id: None,
            outline_level: None,
            collapsed: None,
            best_fit: None,
        };
        if let Some(w) = num_attr(Some(col), "width") {
            info.width = Some(w);
        }
        if bool_attr(Some(col), "customWidth") == Some(true) {
            info.custom_width = Some(true);
        }
        if bool_attr(Some(col), "hidden") == Some(true) {
            info.hidden = Some(true);
        }
        let style = num_attr(Some(col), "style");
        let style_id = match style {
            None => 0,
            Some(s) => xf_map.get(s as usize).copied().unwrap_or(0),
        };
        if style_id > 0 {
            info.style_id = Some(style_id);
        }
        if let Some(o) = num_attr(Some(col), "outlineLevel") {
            if o > 0.0 {
                info.outline_level = Some(o);
            }
        }
        if bool_attr(Some(col), "collapsed") == Some(true) {
            info.collapsed = Some(true);
        }
        if bool_attr(Some(col), "bestFit") == Some(true) {
            info.best_fit = Some(true);
        }
        out.push(info);
    }
    out
}

pub fn read_row_info(row_el: &Element, xf_map: &[u32]) -> Option<RowInfo> {
    let mut info = RowInfo::default();
    if let Some(h) = num_attr(Some(row_el), "ht") {
        info.height = Some(h);
    }
    if bool_attr(Some(row_el), "customHeight") == Some(true) {
        info.custom_height = Some(true);
    }
    if bool_attr(Some(row_el), "hidden") == Some(true) {
        info.hidden = Some(true);
    }
    if bool_attr(Some(row_el), "customFormat") == Some(true) {
        let s = num_attr(Some(row_el), "s");
        let style_id = match s {
            None => 0,
            Some(s) => xf_map.get(s as usize).copied().unwrap_or(0),
        };
        if style_id > 0 {
            info.style_id = Some(style_id);
        }
    }
    if let Some(o) = num_attr(Some(row_el), "outlineLevel") {
        if o > 0.0 {
            info.outline_level = Some(o);
        }
    }
    if bool_attr(Some(row_el), "collapsed") == Some(true) {
        info.collapsed = Some(true);
    }
    if info == RowInfo::default() {
        None
    } else {
        Some(info)
    }
}
