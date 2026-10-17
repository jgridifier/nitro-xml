// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/core/builders/PptxTableDataParser.ts
// and src/ui/src/pptx/render/table-style-fill.ts (Medium Style 2 - Accent 1 whole-table tint);
// changes in PROVENANCE.md.

use std::collections::BTreeMap;

use nitro_xml_opc::drawingml::{
    emu_to_pixels, parse_drawing_color_in, resolve_drawing_color, theme_color_slot_for,
    DrawingColorTheme, ThemeColorMap,
};
use nitro_xml_opc::xml::{ns, Element, Node};

use crate::inheritance::{attr, child_local, children_local};
use crate::table_style::{
    normalize_table_style_guid, resolve_package_cell_style, CellBorders, TableBandFlags,
    TableStyleMap, TableTheme,
};

/// Parsed `a:tbl` for the painter. Skipped in the issue-24 dump.
#[derive(Debug, Clone)]
pub(crate) struct TableData {
    /// Column widths as fractions of the grid (sum to 1).
    pub column_widths: Vec<f64>,
    pub rows: Vec<TableRow>,
    /// Master `p:otherStyle` lvl1 `defRPr/@sz` in points (upstream `defaultCellFontSize`).
    pub default_cell_font_size_pt: Option<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct TableRow {
    pub height_px: Option<f64>,
    pub cells: Vec<TableCell>,
}

#[derive(Debug, Clone)]
pub(crate) struct TableCell {
    pub text: String,
    pub fill: Option<String>,
    pub text_color: Option<String>,
    /// Header / style `tcTxStyle/@b` (Medium Style 2 firstRow is bold).
    pub font_bold: bool,
    /// CSS border shorthands per edge (`1px solid #FFFFFF`) from the cell or its table style.
    pub border: crate::table_style::CellBorders,
    pub grid_span: u32,
    pub row_span: u32,
    pub h_merge: bool,
    pub v_merge: bool,
}

pub fn table_row_count(table: &Element) -> usize {
    children_local(table, "tr", ns::A).len()
}

pub(crate) fn parse_table_model(
    table: &Element,
    theme_colors: Option<&BTreeMap<String, String>>,
    color_map: Option<&ThemeColorMap>,
    table_style_map: Option<&TableStyleMap>,
    default_cell_font_size_pt: Option<f64>,
) -> TableData {
    let fallback = crate::theme::default_scheme_color_map();
    let colors = theme_colors.unwrap_or(&fallback);
    let theme = SchemeMap {
        colors,
        map: color_map,
    };
    let grid = child_local(table, "tblGrid", ns::A);
    let column_widths = column_widths(grid);
    let tbl_pr = child_local(table, "tblPr", ns::A);
    let style_id = table_style_id(tbl_pr);
    let flags = band_flags(tbl_pr);
    // A deck's own `tableStyles.xml` entry wins; otherwise the built-in gallery catalogue.
    let package_entry = style_id.as_deref().and_then(|id| {
        table_style_map
            .and_then(|m| lookup_table_style(m, id))
            .or_else(|| crate::table_style_builtins::builtin_table_style(id))
    });
    let xml_rows = children_local(table, "tr", ns::A);
    let row_count = xml_rows.len();
    let col_count = column_widths.len();
    let rows = xml_rows
        .iter()
        .enumerate()
        .map(|(row_index, row)| TableRow {
            height_px: attr(row, "h")
                .and_then(|s| s.parse::<i64>().ok())
                .filter(|h| *h > 0)
                .map(emu_to_pixels),
            cells: children_local(row, "tc", ns::A)
                .into_iter()
                .enumerate()
                .map(|(col_index, cell)| {
                    parse_cell(
                        cell,
                        &theme,
                        CellPlace {
                            package_entry,
                            flags: &flags,
                            row_index,
                            col_index,
                            row_count,
                            col_count: col_count.max(1),
                        },
                    )
                })
                .collect(),
        })
        .collect();
    TableData {
        column_widths,
        rows,
        default_cell_font_size_pt,
    }
}

struct CellPlace<'a> {
    package_entry: Option<&'a crate::table_style::TableStyleEntry>,
    flags: &'a BandFlags,
    row_index: usize,
    col_index: usize,
    row_count: usize,
    col_count: usize,
}

fn lookup_table_style<'a>(
    map: &'a TableStyleMap,
    id: &str,
) -> Option<&'a crate::table_style::TableStyleEntry> {
    let key = normalize_table_style_guid(id);
    if let Some(entry) = map.get(&key) {
        return Some(entry);
    }
    let bare = id.trim().trim_matches(|c| c == '{' || c == '}');
    map.get(&normalize_table_style_guid(bare))
}

fn parse_cell(cell: &Element, theme: &SchemeMap<'_>, place: CellPlace<'_>) -> TableCell {
    let tc_pr = child_local(cell, "tcPr", ns::A);
    let no_fill = tc_pr.is_some_and(|p| child_local(p, "noFill", ns::A).is_some());
    let explicit = tc_pr
        .and_then(|p| child_local(p, "solidFill", ns::A))
        .and_then(|fill| solid_css(fill, theme));
    let (style_fill, style_text, style_border, style_bold) = if no_fill {
        (None, None, CellBorders::default(), false)
    } else if let Some(entry) = place.package_entry {
        let band = TableBandFlags {
            first_row: place.flags.first_row,
            last_row: place.flags.last_row,
            first_col: place.flags.first_col,
            last_col: place.flags.last_col,
            band_row: place.flags.band_row,
        };
        let table_theme = TableTheme {
            colors: theme.colors,
            map: theme.map,
        };
        let (fill, text, border) = resolve_package_cell_style(
            entry,
            &band,
            place.row_index,
            place.col_index,
            place.row_count,
            place.col_count,
            &table_theme,
        );
        let bold = resolve_package_cell_bold(
            entry,
            &band,
            place.row_index,
            place.col_index,
            place.row_count,
            place.col_count,
        );
        (fill, text, border, bold)
    } else {
        (None, None, CellBorders::default(), false)
    };
    let fill = if no_fill {
        None
    } else {
        explicit.or(style_fill)
    };
    let border = match cell_border_color(tc_pr, theme) {
        Some(color) => CellBorders::uniform(format!("1px solid {color}")),
        None => style_border,
    };
    TableCell {
        text: cell_text(cell),
        fill,
        text_color: style_text,
        font_bold: style_bold,
        border,
        grid_span: span_attr(attr(cell, "gridSpan")),
        row_span: span_attr(attr(cell, "rowSpan")),
        h_merge: on_off(attr(cell, "hMerge")),
        v_merge: on_off(attr(cell, "vMerge")),
    }
}

struct BandFlags {
    first_row: bool,
    last_row: bool,
    first_col: bool,
    last_col: bool,
    band_row: bool,
}

fn band_flags(tbl_pr: Option<&Element>) -> BandFlags {
    BandFlags {
        first_row: on_off(tbl_pr.and_then(|p| attr(p, "firstRow"))),
        last_row: on_off(tbl_pr.and_then(|p| attr(p, "lastRow"))),
        first_col: on_off(tbl_pr.and_then(|p| attr(p, "firstCol"))),
        last_col: on_off(tbl_pr.and_then(|p| attr(p, "lastCol"))),
        band_row: on_off(tbl_pr.and_then(|p| attr(p, "bandRow"))),
    }
}

fn resolve_package_cell_bold(
    entry: &crate::table_style::TableStyleEntry,
    flags: &TableBandFlags,
    row_index: usize,
    col_index: usize,
    row_count: usize,
    col_count: usize,
) -> bool {
    let at_top = flags.first_row && row_index == 0;
    let at_bottom = flags.last_row && row_index + 1 == row_count;
    let at_left = flags.first_col && col_index == 0;
    let at_right = flags.last_col && col_index + 1 == col_count;
    let mut bold = entry.whole_tbl.text.as_ref().is_some_and(|t| t.bold);
    let band_start_row = usize::from(flags.first_row);
    let band_end_row = if flags.last_row {
        row_count.saturating_sub(1)
    } else {
        row_count
    };
    if flags.band_row && row_index >= band_start_row && row_index < band_end_row {
        let band_index = row_index - band_start_row;
        let part = if band_index.is_multiple_of(2) {
            &entry.band1_h
        } else {
            &entry.band2_h
        };
        if let Some(t) = part.text.as_ref() {
            bold = t.bold;
        }
    }
    let mut apply = |part: &crate::table_style::TableStylePart| {
        if let Some(t) = part.text.as_ref() {
            bold = t.bold;
        }
    };
    if at_right {
        apply(&entry.last_col);
    }
    if at_left {
        apply(&entry.first_col);
    }
    if at_bottom {
        apply(&entry.last_row);
    }
    if at_bottom && at_right {
        apply(&entry.se_cell);
    }
    if at_bottom && at_left {
        apply(&entry.sw_cell);
    }
    if at_top {
        apply(&entry.first_row);
    }
    if at_top && at_right {
        apply(&entry.ne_cell);
    }
    if at_top && at_left {
        apply(&entry.nw_cell);
    }
    bold
}

fn column_widths(grid: Option<&Element>) -> Vec<f64> {
    let Some(grid) = grid else {
        return Vec::new();
    };
    let widths: Vec<i64> = children_local(grid, "gridCol", ns::A)
        .iter()
        .map(|col| attr(col, "w").and_then(|s| s.parse().ok()).unwrap_or(0))
        .collect();
    let total: i64 = widths.iter().sum();
    if total > 0 {
        widths.iter().map(|w| *w as f64 / total as f64).collect()
    } else if widths.is_empty() {
        Vec::new()
    } else {
        let share = 1.0 / widths.len() as f64;
        vec![share; widths.len()]
    }
}

fn table_style_id(tbl_pr: Option<&Element>) -> Option<String> {
    let tbl_pr = tbl_pr?;
    if let Some(id) = child_local(tbl_pr, "tableStyleId", ns::A) {
        let text = id.text_content();
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }
    if let Some(style) = child_local(tbl_pr, "tblStyle", ns::A) {
        if let Some(val) = attr(style, "val").filter(|s| !s.is_empty()) {
            return Some(val.to_owned());
        }
    }
    attr(tbl_pr, "tblStyle")
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn cell_text(cell: &Element) -> String {
    let Some(body) = child_local(cell, "txBody", ns::A) else {
        return concat_t(cell);
    };
    let paras = children_local(body, "p", ns::A);
    if paras.is_empty() {
        return concat_t(body);
    }
    paras
        .iter()
        .map(|p| concat_t(p))
        .collect::<Vec<_>>()
        .join("\n")
}

fn concat_t(el: &Element) -> String {
    let mut out = String::new();
    walk_t(el, &mut out);
    out
}

fn walk_t(el: &Element, out: &mut String) {
    if el.local == "t" && (el.ns.as_deref() == Some(ns::A) || el.ns.is_none()) {
        out.push_str(&el.text_content());
        return;
    }
    for child in &el.children {
        if let Node::Element(nested) = child {
            walk_t(nested, out);
        }
    }
}

fn cell_border_color(tc_pr: Option<&Element>, theme: &SchemeMap<'_>) -> Option<String> {
    let tc_pr = tc_pr?;
    for side in ["lnL", "lnR", "lnT", "lnB"] {
        let Some(ln) = child_local(tc_pr, side, ns::A) else {
            continue;
        };
        if child_local(ln, "noFill", ns::A).is_some() {
            continue;
        }
        if let Some(fill) = child_local(ln, "solidFill", ns::A) {
            if let Some(css) = solid_css(fill, theme) {
                return Some(css);
            }
        }
    }
    None
}

fn solid_css(fill: &Element, theme: &SchemeMap<'_>) -> Option<String> {
    let color = parse_drawing_color_in(Some(fill))?;
    let resolved = resolve_drawing_color(&color, Some(theme), None)?;
    Some(resolved.hex)
}

fn span_attr(value: Option<&str>) -> u32 {
    value
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|n| *n > 1)
        .unwrap_or(1)
}

fn on_off(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim).map(|s| s.to_ascii_lowercase()),
        Some(ref v) if v == "1" || v == "true" || v == "on"
    )
}

struct SchemeMap<'a> {
    colors: &'a BTreeMap<String, String>,
    map: Option<&'a ThemeColorMap>,
}

impl DrawingColorTheme for SchemeMap<'_> {
    fn scheme(&self, name: &str) -> Option<String> {
        let slot = mapped_slot(name, self.map);
        self.colors
            .get(&slot)
            .cloned()
            .or_else(|| self.colors.get(name).cloned())
    }
}

fn mapped_slot(name: &str, map: Option<&ThemeColorMap>) -> String {
    if let Some(map) = map {
        if let Some(slot) = theme_color_slot_for(name, map) {
            return slot.as_str().to_owned();
        }
    }
    match name {
        "bg1" => "lt1",
        "tx1" => "dk1",
        "bg2" => "lt2",
        "tx2" => "dk2",
        other => other,
    }
    .to_owned()
}
