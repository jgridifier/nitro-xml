// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/cell-view.ts; changes listed in PROVENANCE.md.

use crate::cf::cf_at;
use crate::model::{ActiveWorksheet, DumpCellValue, WorkbookSlice};
use crate::numfmt::{format_value, FormatOptions};
use crate::sheet_access::{
    dump_value_to_cell_value, get_cell, is_cell_error, range_contains, CellIndex,
};
use crate::styles::style_at;
use crate::view::sparkline_data::sparkline_view_at;
use crate::view::style_view::{borders_view, fill_view, font_view, merge_border, merge_font};
use crate::view::types::{CellView, FillView, RichRun};
use crate::view::units::INDENT_PX_PER_LEVEL;

pub fn effective_style_id(sheet: &ActiveWorksheet, index: &CellIndex, row: u32, col: u32) -> u32 {
    if let Some(cell) = get_cell(sheet, index, row, col) {
        if let Some(s) = cell.style_id {
            return s;
        }
        return 0;
    }
    if let Some(info) = sheet.row_info.get(&row) {
        if let Some(s) = info.style_id {
            return s;
        }
    }
    for c in &sheet.columns {
        if col >= c.min && col <= c.max {
            return c.style_id.unwrap_or(0);
        }
    }
    0
}

pub fn general_alignment(value: &DumpCellValue) -> &'static str {
    match value {
        DumpCellValue::Number(_) | DumpCellValue::NumberRaw(_) => "right",
        DumpCellValue::Bool(_) | DumpCellValue::Error(_) => "center",
        _ => "left",
    }
}

pub fn rotation_degrees(text_rotation: Option<f64>) -> (f64, bool) {
    let tr = text_rotation.unwrap_or(0.0);
    if tr == 0.0 {
        return (0.0, false);
    }
    if tr == 255.0 {
        return (0.0, true);
    }
    if tr > 90.0 && tr <= 180.0 {
        return (-(tr - 90.0), false);
    }
    (tr.clamp(-90.0, 90.0), false)
}

#[allow(clippy::map_clone)]
fn indexed_palette(workbook: &WorkbookSlice) -> Vec<&str> {
    workbook
        .palette
        .as_ref()
        .map(|p| p.iter().map(|s| s.as_str()).collect())
        .unwrap_or_else(|| {
            crate::view::colors::INDEXED_COLORS
                .iter()
                .map(|s| *s)
                .collect()
        })
}

fn display_text(
    value: &DumpCellValue,
    num_fmt: &str,
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
) -> (String, Option<String>) {
    if matches!(value, DumpCellValue::Null) {
        return (String::new(), None);
    }
    if let DumpCellValue::Number(n) = value {
        if *n == 0.0 && !sheet.view.show_zeros {
            return (String::new(), None);
        }
    }
    if let DumpCellValue::Bool(b) = value {
        return (if *b { "TRUE" } else { "FALSE" }.into(), None);
    }
    if let DumpCellValue::Error(e) = value {
        return (e.clone(), None);
    }
    let cv = dump_value_to_cell_value(value);
    let formatted = format_value(
        &cv,
        if num_fmt.is_empty() {
            "General"
        } else {
            num_fmt
        },
        &FormatOptions {
            date1904: workbook.date1904,
            width: None,
            measure: None,
        },
    );
    if let Some(c) = formatted.color {
        (formatted.text, Some(c))
    } else {
        (formatted.text, None)
    }
}

#[derive(Default)]
pub struct CellViewOptions<'a> {
    pub indent_px: Option<f64>,
    pub sheet_index: Option<usize>,
    pub all_sheets: Option<&'a [Option<ActiveWorksheet>]>,
}

/// Core cell view without conditional formatting (oracle parity path).
pub fn cell_view(
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    row: u32,
    col: u32,
    options: CellViewOptions<'_>,
) -> CellView {
    cell_view_with_cf_cache(workbook, sheet, index, row, col, options, None)
}

/// Like [`cell_view`], sharing a [`CfPaintCache`] across cells in one paint frame.
pub fn cell_view_with_cf_cache(
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    row: u32,
    col: u32,
    options: CellViewOptions<'_>,
    mut cf_cache: Option<&mut crate::cf::CfPaintCache>,
) -> CellView {
    let indexed = indexed_palette(workbook);
    let cell = get_cell(sheet, index, row, col);
    let value = cell.map(|c| c.value.clone()).unwrap_or(DumpCellValue::Null);
    let value_for_align = value.clone();
    let style = style_at(
        &workbook.styles,
        Some(effective_style_id(sheet, index, row, col)),
    );

    let default_font = workbook.styles.first().map(|s| &s.font);
    let mut base = style.font.clone();
    if base.name.is_none() && base.scheme.is_none() {
        if let Some(df) = default_font {
            if df.name.is_some() {
                base.name = df.name.clone();
            }
            if df.scheme.is_some() {
                base.scheme = df.scheme.clone();
            }
        }
    }
    if base.size.is_none() {
        if let Some(df) = default_font {
            if df.size.is_some() {
                base.size = df.size;
            }
        }
    }
    let font = merge_font(&base, None);
    let formula_text = if sheet.view.show_formulas == Some(true) {
        cell.and_then(|c| c.formula.as_ref())
            .map(|f| format!("={}", f))
    } else {
        None
    };
    let cf = match cf_cache.as_mut() {
        Some(cache) => {
            crate::cf::cf_at_cached(sheet, index, row, col, &workbook.theme, &indexed, cache)
        }
        None => cf_at(sheet, index, row, col, &workbook.theme, &indexed),
    };
    let hide_value = cf.as_ref().is_some_and(|c| c.hide_value);
    let shown = if hide_value {
        (String::new(), None)
    } else if let Some(ft) = &formula_text {
        (ft.clone(), None)
    } else {
        // Upstream cell-view.ts: `dxf?.numFmt ?? style.numFmt` (CF can override e.g. #,##0 → #,##0.0).
        let fmt = cf
            .as_ref()
            .and_then(|c| c.style.as_ref())
            .and_then(|s| s.num_fmt.as_deref())
            .filter(|s| !s.is_empty())
            .unwrap_or(style.num_fmt.as_str());
        let (t, c) = display_text(&value, fmt, workbook, sheet);
        (t, c)
    };
    let font_for_view = if let Some(cf) = &cf {
        if let Some(dxf) = &cf.style {
            merge_font(&font, dxf.font.as_ref())
        } else {
            font.clone()
        }
    } else {
        font.clone()
    };
    let mut font_view_resolved = font_view(&font_for_view, &workbook.theme, &indexed);
    if let Some(c) = shown.1 {
        font_view_resolved.color = c;
    }

    let alignment = style.alignment.as_ref();
    let horizontal = alignment
        .and_then(|a| a.horizontal.as_deref())
        .unwrap_or("general");
    let h_align = if horizontal != "general" {
        horizontal.to_string()
    } else if formula_text.is_some() {
        "left".into()
    } else {
        general_alignment(&value_for_align).to_string()
    };
    let text_rotation = alignment.and_then(|a| a.text_rotation);
    let (rotation, vertical) = rotation_degrees(text_rotation);
    let wrap = alignment.and_then(|a| a.wrap_text) == Some(true)
        || h_align == "justify"
        || h_align == "distributed";
    let shrink = alignment.and_then(|a| a.shrink_to_fit) == Some(true) && !wrap;
    let indentable = h_align == "left" || h_align == "right" || h_align == "distributed";
    let indent = alignment.and_then(|a| a.indent).unwrap_or(0.0);
    let indent_px = if indentable {
        indent * options.indent_px.unwrap_or(INDENT_PX_PER_LEVEL)
    } else {
        0.0
    };

    let mut fill = fill_view(Some(&style.fill), &workbook.theme, false, &indexed);
    if let Some(cf) = &cf {
        if let Some(scale) = &cf.color_scale {
            fill = Some(FillView::Background {
                background: scale.clone(),
            });
        } else if let Some(dxf) = &cf.style {
            if let Some(cf_fill) = fill_view(dxf.fill.as_ref(), &workbook.theme, true, &indexed) {
                fill = Some(cf_fill);
            }
        }
    }

    let is_number = matches!(
        value,
        DumpCellValue::Number(_) | DumpCellValue::NumberRaw(_)
    ) && formula_text.is_none();
    let overflow = (formula_text.is_some()
        || matches!(&value_for_align, DumpCellValue::String(s) if !s.is_empty()))
        && !wrap
        && !shrink
        && rotation == 0.0
        && !vertical
        && h_align != "fill"
        && !sheet.merges.iter().any(|m| range_contains(m, row, col));

    // Upstream: `bordersView(mergeBorder(style.border, dxf?.border), …)`.
    let border_for_view = if let Some(dxf) = cf.as_ref().and_then(|c| c.style.as_ref()) {
        merge_border(&style.border, dxf.border.as_ref())
    } else {
        style.border.clone()
    };
    let mut view = CellView {
        text: shown.0,
        title: None,
        font: font_view_resolved,
        fill,
        borders: borders_view(&border_for_view, &workbook.theme, &indexed),
        h_align,
        v_align: alignment
            .and_then(|a| a.vertical.as_deref())
            .unwrap_or("bottom")
            .to_string(),
        wrap,
        shrink,
        indent_px,
        rotation,
        vertical_text: vertical,
        is_number,
        overflow,
        data_bar: cf.as_ref().and_then(|c| c.data_bar.clone()),
        icon: cf.as_ref().and_then(|c| c.icon.clone().map(Into::into)),
        has_comment: sheet
            .comment_addresses
            .iter()
            .any(|a| a.row == row && a.col == col),
        has_hyperlink: false,
        validation_list: sheet.data_validations.iter().any(|dv| {
            dv.validation_type == "list"
                && dv.show_drop_down != Some(false)
                && dv.ranges.iter().any(|r| range_contains(r, row, col))
        }),
        is_error: is_cell_error(&value) && formula_text.is_none(),
        rich: None,
        sparkline: None,
    };

    for link in &sheet.hyperlinks {
        if range_contains(&link.range, row, col) {
            view.has_hyperlink = true;
            let title = link
                .tooltip
                .clone()
                .or_else(|| link.target.clone())
                .or_else(|| link.location.clone());
            if let Some(t) = title {
                view.title = Some(t);
            }
            break;
        }
    }

    if let (Some(sheet_index), Some(all_sheets)) = (options.sheet_index, options.all_sheets) {
        if !all_sheets.is_empty() {
            if let Some(sp) = sparkline_view_at(workbook, all_sheets, sheet_index, row, col) {
                view.sparkline = Some(sp);
            }
        }
    }

    if !hide_value {
        if let Some(c) = cell {
            if let Some(rt) = &c.rich_text {
                if matches!(value, DumpCellValue::String(_)) {
                    view.rich = Some(
                        rt.iter()
                            .map(|run| RichRun {
                                text: run.text.clone(),
                                font: font_view(
                                    &merge_font(&font_for_view, run.font.as_ref()),
                                    &workbook.theme,
                                    &indexed,
                                ),
                            })
                            .collect(),
                    );
                }
            }
        }
    }

    view
}

pub fn is_blank_view(view: &CellView) -> bool {
    view.text.is_empty()
        && view.fill.is_none()
        && !view.borders.has_borders()
        && view.data_bar.is_none()
        && view.icon.is_none()
        && !view.has_comment
        && view.sparkline.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        CellAddress, CellRange, ConditionalFormat, ConditionalRule, DifferentialStyle, DumpCell,
        SheetState,
    };
    use crate::sheet_props::default_sheet_view;
    use crate::workbook::{default_cell_style, default_theme};
    use std::collections::BTreeMap;

    #[test]
    fn cf_dxf_num_fmt_overrides_cell_style() {
        // Catches: CF dxf numFmt (e.g. #,##0.0 when H4=DEC) ignored so Value paints 71 vs TS 70.5.
        let mut style = default_cell_style();
        style.num_fmt = "#,##0".into();
        let workbook = WorkbookSlice {
            format: "xlsx".into(),
            date1904: false,
            active_sheet: 0,
            sheets: vec![],
            defined_names: vec![],
            theme: default_theme(),
            styles: vec![default_cell_style(), style],
            named_styles: vec![],
            xf_map: vec![0, 1],
            palette: None,
            dxfs: vec![],
            shared_strings: vec![],
            warnings: vec![],
            full_calc_on_load: None,
            structure_locked: None,
            calc_mode: None,
            iterate: None,
            workbook_password_hash: None,
            workbook_modern_hash: None,
            active_worksheet: None,
        };
        let sheet = ActiveWorksheet {
            name: "t".into(),
            sheet_id: 1,
            state: SheetState::Visible,
            part_name: None,
            default_row_height: 15.0,
            default_col_width: None,
            format: None,
            view: default_sheet_view(),
            columns: vec![],
            merges: vec![],
            row_info: BTreeMap::new(),
            cell_store: None,
            cells: vec![
                DumpCell {
                    row: 3,
                    col: 7,
                    value: DumpCellValue::String("DEC".into()),
                    type_tag: None,
                    formula: None,
                    style_id: None,
                    rich_text: None,
                    array_range: None,
                    legacy_formula: None,
                    dynamic_array: None,
                    spill_anchor: None,
                    cell_metadata: None,
                    value_metadata: None,
                },
                DumpCell {
                    row: 8,
                    col: 2,
                    value: DumpCellValue::Number(70.5234),
                    type_tag: None,
                    formula: None,
                    style_id: Some(1),
                    rich_text: None,
                    array_range: None,
                    legacy_formula: None,
                    dynamic_array: None,
                    spill_anchor: None,
                    cell_metadata: None,
                    value_metadata: None,
                },
            ],
            data_validations: vec![],
            comment_addresses: vec![],
            hyperlinks: vec![],
            ext_lst: vec![],
            sparkline_groups: vec![],
            drawings: vec![],
            conditional_formats: vec![ConditionalFormat {
                ranges: vec![CellRange {
                    start: CellAddress { row: 8, col: 2 },
                    end: CellAddress { row: 8, col: 2 },
                }],
                rules: vec![ConditionalRule::Expression {
                    formula: "OR($H$4=\"DEC\",$H$4=\"PERC\")".into(),
                    style: DifferentialStyle {
                        font: None,
                        fill: None,
                        border: None,
                        num_fmt: Some("#,##0.0".into()),
                    },
                    priority: 2,
                    stop_if_true: false,
                }],
            }],
            dimension: None,
        };
        let index = CellIndex::build(&sheet);
        let view = cell_view(&workbook, &sheet, &index, 8, 2, CellViewOptions::default());
        assert_eq!(view.text, "70.5");
    }
}
