// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/cell-view.ts; changes listed in PROVENANCE.md.

use std::collections::HashMap;
use std::rc::Rc;

use crate::cf::cf_at;
use crate::fast_hash::FastBuild;
use crate::model::{ActiveWorksheet, CellStyle, DumpCellValue, Font, WorkbookSlice};
use crate::numfmt::{format_value, FormatOptions};
use crate::sheet_access::{
    dump_value_to_cell_value, get_cell, is_cell_error, range_contains, CellIndex,
};
use crate::styles::style_at;
use crate::view::style_view::{borders_view, fill_view, font_view, merge_border, merge_font};
use crate::view::types::{BordersView, CellView, FillView, FontView, RichRun};
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

/// Default theme palette is a static table — avoid allocating a `Vec` per cell on paint.
fn indexed_palette<'a>(workbook: &'a WorkbookSlice) -> std::borrow::Cow<'a, [&'a str]> {
    match workbook.palette.as_ref() {
        Some(p) => std::borrow::Cow::Owned(p.iter().map(String::as_str).collect()),
        None => std::borrow::Cow::Borrowed(&crate::view::colors::INDEXED_COLORS),
    }
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
        // cells_1k / default xf: General — skip compile_format + section choose (#30).
        if num_fmt.is_empty() || num_fmt.eq_ignore_ascii_case("General") {
            return (crate::numfmt::format_general(*n, None), None);
        }
    } else if let DumpCellValue::NumberRaw(s) = value {
        if let Ok(n) = s.parse::<f64>() {
            if n == 0.0 && !sheet.view.show_zeros {
                return (String::new(), None);
            }
            if num_fmt.is_empty() || num_fmt.eq_ignore_ascii_case("General") {
                return (crate::numfmt::format_general(n, None), None);
            }
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
    pub all_sheets: Option<crate::view::sheets_ref::AllSheetsRef<'a>>,
    /// Skip `sheet.merges` scan for overflow (caller already excluded covered cells).
    pub not_in_merge: bool,
    /// Optional O(1) comment lookup for the paint frame (#30 build_items).
    pub comment_at: Option<&'a dyn Fn(u32, u32) -> bool>,
    /// Per-frame cache of style→font/fill/borders (cells_1k is all style 0; #30).
    pub style_cache: Option<&'a mut StylePaintCache>,
}

/// Frame-local cache for style-derived paint views (no CF override).
#[derive(Default)]
pub struct StylePaintCache {
    entries: HashMap<u32, Rc<StylePaintParts>, FastBuild>,
}

struct StylePaintParts {
    /// Base Font for rich-run merges (before value-color override on FontView).
    font: Font,
    font_view: Rc<FontView>,
    fill: Option<FillView>,
    borders: BordersView,
}

impl StylePaintCache {
    fn get_or_compute<F>(&mut self, style_id: u32, f: F) -> Rc<StylePaintParts>
    where
        F: FnOnce() -> StylePaintParts,
    {
        self.entries
            .entry(style_id)
            .or_insert_with(|| Rc::new(f()))
            .clone()
    }
}

/// Resolve style font/fill/borders once per style_id (shared across cells_1k paint).
fn resolve_style_parts(
    workbook: &WorkbookSlice,
    style: &CellStyle,
    indexed: &[&str],
) -> StylePaintParts {
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
    // merge_font(None) is clone — keep the call for parity with the CF merge path.
    let font = merge_font(&base, None);
    let font_view_resolved = Rc::new(font_view(&font, &workbook.theme, indexed));
    let fill = fill_view(Some(&style.fill), &workbook.theme, false, indexed);
    let borders = borders_view(&style.border, &workbook.theme, indexed);
    StylePaintParts {
        font,
        font_view: font_view_resolved,
        fill,
        borders,
    }
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
    mut options: CellViewOptions<'_>,
    mut cf_cache: Option<&mut crate::cf::CfPaintCache>,
) -> CellView {
    let not_in_merge = options.not_in_merge;
    let comment_at = options.comment_at;
    let indent_opt = options.indent_px;
    let sheet_index = options.sheet_index;
    let all_sheets = options.all_sheets;

    let indexed = indexed_palette(workbook);
    let cell = get_cell(sheet, index, row, col);
    let value_ref = cell.map(|c| &c.value).unwrap_or(&DumpCellValue::Null);
    let style_id = effective_style_id(sheet, index, row, col);
    let style = style_at(&workbook.styles, Some(style_id));

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
        let (t, c) = display_text(value_ref, fmt, workbook, sheet);
        (t, c)
    };

    // num_fmt alone does not restyle font/fill/borders — still cache those (#30).
    let cf_restyles = cf.as_ref().is_some_and(|c| {
        c.color_scale.is_some()
            || c.style
                .as_ref()
                .is_some_and(|d| d.font.is_some() || d.fill.is_some() || d.border.is_some())
    });

    // Shared Rc on the cache hit path — FontView is not cloned per cell (#30).
    let mut cached_parts: Option<Rc<StylePaintParts>> = None;
    let (font_for_view, mut font_view_resolved, fill, borders) = if !cf_restyles {
        if let Some(cache) = options.style_cache.as_mut() {
            let parts =
                cache.get_or_compute(style_id, || resolve_style_parts(workbook, style, &indexed));
            let font_view_resolved = Rc::clone(&parts.font_view);
            let fill = parts.fill.clone();
            let borders = parts.borders.clone();
            // Font stays behind the Rc until rich-text needs it.
            cached_parts = Some(parts);
            (None, font_view_resolved, fill, borders)
        } else {
            let parts = resolve_style_parts(workbook, style, &indexed);
            (Some(parts.font), parts.font_view, parts.fill, parts.borders)
        }
    } else {
        let parts = resolve_style_parts(workbook, style, &indexed);
        let font_for_view = match cf.as_ref().and_then(|c| c.style.as_ref()) {
            Some(dxf) if dxf.font.is_some() => merge_font(&parts.font, dxf.font.as_ref()),
            _ => parts.font,
        };
        let font_view_resolved = Rc::new(font_view(&font_for_view, &workbook.theme, &indexed));
        let mut fill = parts.fill;
        if let Some(cf) = &cf {
            if let Some(scale) = &cf.color_scale {
                fill = Some(FillView::Background {
                    background: scale.clone(),
                });
            } else if let Some(dxf) = &cf.style {
                if let Some(cf_fill) = fill_view(dxf.fill.as_ref(), &workbook.theme, true, &indexed)
                {
                    fill = Some(cf_fill);
                }
            }
        }
        let merged_border;
        let borders = match cf.as_ref().and_then(|c| c.style.as_ref()) {
            Some(dxf) if dxf.border.is_some() => {
                merged_border = merge_border(&style.border, dxf.border.as_ref());
                borders_view(&merged_border, &workbook.theme, &indexed)
            }
            _ => parts.borders,
        };
        (Some(font_for_view), font_view_resolved, fill, borders)
    };
    if let Some(c) = shown.1 {
        Rc::make_mut(&mut font_view_resolved).color = c;
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
        general_alignment(value_ref).to_string()
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
        indent * indent_opt.unwrap_or(INDENT_PX_PER_LEVEL)
    } else {
        0.0
    };

    let is_number = matches!(
        value_ref,
        DumpCellValue::Number(_) | DumpCellValue::NumberRaw(_)
    ) && formula_text.is_none();
    let overflow = (formula_text.is_some()
        || matches!(value_ref, DumpCellValue::String(s) if !s.is_empty()))
        && !wrap
        && !shrink
        && rotation == 0.0
        && !vertical
        && h_align != "fill"
        && (not_in_merge || !sheet.merges.iter().any(|m| range_contains(m, row, col)));

    let mut view = CellView {
        text: shown.0,
        title: None,
        font: font_view_resolved,
        fill,
        borders,
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
        has_comment: match comment_at {
            Some(f) => f(row, col),
            None => sheet
                .comment_addresses
                .iter()
                .any(|a| a.row == row && a.col == col),
        },
        has_hyperlink: false,
        validation_list: sheet.data_validations.iter().any(|dv| {
            dv.validation_type == "list"
                && dv.show_drop_down != Some(false)
                && dv.ranges.iter().any(|r| range_contains(r, row, col))
        }),
        is_error: is_cell_error(value_ref) && formula_text.is_none(),
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

    if let (Some(sheet_index), Some(all_sheets)) = (sheet_index, all_sheets) {
        // Skip host-map build when this sheet has no sparklines (#30).
        if !all_sheets.is_empty() && !sheet.sparkline_groups.is_empty() {
            if let Some(sp) = crate::view::sparkline_data::sparkline_view_at_ref(
                workbook,
                all_sheets,
                sheet_index,
                row,
                col,
            ) {
                view.sparkline = Some(sp);
            }
        }
    }

    if !hide_value {
        if let Some(c) = cell {
            if let Some(rt) = &c.rich_text {
                if matches!(value_ref, DumpCellValue::String(_)) {
                    let base_font = font_for_view
                        .as_ref()
                        .or_else(|| cached_parts.as_ref().map(|p| &p.font));
                    if let Some(base_font) = base_font {
                        view.rich = Some(
                            rt.iter()
                                .map(|run| RichRun {
                                    text: run.text.clone(),
                                    font: font_view(
                                        &merge_font(base_font, run.font.as_ref()),
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
            cell_pos: Default::default(),
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
