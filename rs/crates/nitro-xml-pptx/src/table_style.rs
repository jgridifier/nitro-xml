// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec
//   src/core/pptx/core/core/runtime/table-style-entry-parse.ts,
//   src/core/pptx/core/core/runtime/table-style-fill-parse.ts,
//   src/core/pptx/core/core/runtime/table-style-border-parse.ts,
//   src/ui/src/pptx/render/table-style.ts and table-style-borders.ts;
// changes in PROVENANCE.md.

use std::collections::BTreeMap;

use nitro_xml_opc::drawingml::EMU_PER_PIXEL;
use nitro_xml_opc::drawingml::{
    hex_to_rgb_channels, theme_color_slot_for, DrawingColorTheme, ThemeColorMap,
};
use nitro_xml_opc::xml::{ns, parse_xml_str, Element};

use crate::inheritance::{attr, child_local, children_local};

const TABLE_STYLE_PARTS: &[&str] = &[
    "wholeTbl", "band1H", "band2H", "band1V", "band2V", "lastCol", "firstCol", "lastRow", "seCell",
    "swCell", "firstRow", "neCell", "nwCell",
];

/// GUID → parsed `a:tblStyle` from `ppt/tableStyles.xml`.
pub(crate) type TableStyleMap = BTreeMap<String, TableStyleEntry>;

#[derive(Debug, Clone, Default)]
pub(crate) struct TableStyleEntry {
    pub style_id: String,
    pub whole_tbl: TableStylePart,
    pub band1_h: TableStylePart,
    pub band2_h: TableStylePart,
    pub band1_v: TableStylePart,
    pub band2_v: TableStylePart,
    pub last_col: TableStylePart,
    pub first_col: TableStylePart,
    pub last_row: TableStylePart,
    pub se_cell: TableStylePart,
    pub sw_cell: TableStylePart,
    pub first_row: TableStylePart,
    pub ne_cell: TableStylePart,
    pub nw_cell: TableStylePart,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TableStylePart {
    pub fill: Option<TableStyleFill>,
    pub text: Option<TableStyleText>,
    pub borders: Option<TableStyleBorders>,
}

#[derive(Debug, Clone)]
pub(crate) struct TableStyleFill {
    pub scheme_color: Option<String>,
    pub color: Option<String>,
    pub no_fill: bool,
    pub tint: Option<i32>,
    pub shade: Option<i32>,
}

#[derive(Debug, Clone)]
pub(crate) struct TableStyleText {
    pub scheme_color: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TableStyleBorders {
    pub left: Option<TableStyleBorder>,
    pub right: Option<TableStyleBorder>,
    pub top: Option<TableStyleBorder>,
    pub bottom: Option<TableStyleBorder>,
    pub inside_h: Option<TableStyleBorder>,
    pub inside_v: Option<TableStyleBorder>,
}

#[derive(Debug, Clone)]
pub(crate) struct TableStyleBorder {
    pub width_px: f64,
    pub fill: Option<TableStyleFill>,
    pub color: Option<String>,
    pub no_fill: bool,
    pub dash: Option<String>,
}

pub(crate) fn normalize_table_style_guid(guid: &str) -> String {
    let trimmed = guid.trim().to_uppercase();
    if trimmed.starts_with('{') {
        trimmed
    } else {
        format!("{{{}}}", trimmed)
    }
}

pub(crate) fn parse_table_style_list(xml: &str) -> TableStyleMap {
    let doc = match parse_xml_str(xml) {
        Ok(d) => d,
        Err(_) => return TableStyleMap::new(),
    };
    let lst = if doc.root.local == "tblStyleLst" {
        &doc.root
    } else {
        child_local(&doc.root, "tblStyleLst", ns::A).unwrap_or(&doc.root)
    };
    let mut map = TableStyleMap::new();
    for style in children_local(lst, "tblStyle", ns::A) {
        if let Some(entry) = parse_table_style_entry(style) {
            map.insert(entry.style_id.clone(), entry);
        }
    }
    map
}

fn parse_table_style_entry(style: &Element) -> Option<TableStyleEntry> {
    let raw_id = attr(style, "styleId")?;
    let trimmed = raw_id.trim();
    if trimmed.is_empty() {
        return None;
    }
    let style_id = normalize_table_style_guid(trimmed);
    let mut entry = TableStyleEntry {
        style_id,
        ..Default::default()
    };
    for part_name in TABLE_STYLE_PARTS {
        let section = child_local(style, part_name, ns::A);
        let parsed = section.map(parse_table_style_part).unwrap_or_default();
        assign_part(&mut entry, part_name, parsed);
    }
    Some(entry)
}

fn assign_part(entry: &mut TableStyleEntry, name: &str, part: TableStylePart) {
    match name {
        "wholeTbl" => entry.whole_tbl = part,
        "band1H" => entry.band1_h = part,
        "band2H" => entry.band2_h = part,
        "band1V" => entry.band1_v = part,
        "band2V" => entry.band2_v = part,
        "lastCol" => entry.last_col = part,
        "firstCol" => entry.first_col = part,
        "lastRow" => entry.last_row = part,
        "seCell" => entry.se_cell = part,
        "swCell" => entry.sw_cell = part,
        "firstRow" => entry.first_row = part,
        "neCell" => entry.ne_cell = part,
        "nwCell" => entry.nw_cell = part,
        _ => {}
    }
}

fn parse_table_style_part(section: &Element) -> TableStylePart {
    let tc_style = child_local(section, "tcStyle", ns::A);
    let fill = tc_style
        .and_then(|s| child_local(s, "fill", ns::A))
        .and_then(parse_section_fill);
    let text = child_local(section, "tcTxStyle", ns::A).and_then(parse_section_text);
    let borders = tc_style
        .and_then(|s| child_local(s, "tcBdr", ns::A))
        .and_then(parse_table_style_borders);
    TableStylePart {
        fill,
        text,
        borders,
    }
}

fn parse_section_fill(fill_container: &Element) -> Option<TableStyleFill> {
    if child_local(fill_container, "noFill", ns::A).is_some() {
        return Some(TableStyleFill {
            scheme_color: None,
            color: None,
            no_fill: true,
            tint: None,
            shade: None,
        });
    }
    let solid = child_local(fill_container, "solidFill", ns::A);
    parse_color_choice_fill(solid)
}

fn parse_section_text(tc_tx: &Element) -> Option<TableStyleText> {
    let scheme_color = child_local(tc_tx, "schemeClr", ns::A)
        .and_then(|c| attr(c, "val"))
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let color = child_local(tc_tx, "srgbClr", ns::A)
        .and_then(|c| attr(c, "val"))
        .filter(|s| !s.is_empty())
        .map(srgb_to_hex);
    if scheme_color.is_none() && color.is_none() {
        return None;
    }
    Some(TableStyleText {
        scheme_color,
        color,
    })
}

fn parse_color_choice_fill(solid: Option<&Element>) -> Option<TableStyleFill> {
    let solid = solid?;
    if let Some(scheme) = child_local(solid, "schemeClr", ns::A) {
        let scheme_color = attr(scheme, "val").filter(|s| !s.is_empty())?;
        let tint = child_local(scheme, "tint", ns::A).and_then(parse_ooxml_percent_attr);
        let shade = child_local(scheme, "shade", ns::A).and_then(parse_ooxml_percent_attr);
        return Some(TableStyleFill {
            scheme_color: Some(scheme_color.to_owned()),
            color: None,
            no_fill: false,
            tint,
            shade,
        });
    }
    if let Some(srgb) = child_local(solid, "srgbClr", ns::A) {
        let color = attr(srgb, "val").filter(|s| !s.is_empty())?;
        let tint = child_local(srgb, "tint", ns::A).and_then(parse_ooxml_percent_attr);
        let shade = child_local(srgb, "shade", ns::A).and_then(parse_ooxml_percent_attr);
        return Some(TableStyleFill {
            scheme_color: None,
            color: Some(srgb_to_hex(color)),
            no_fill: false,
            tint,
            shade,
        });
    }
    None
}

fn parse_ooxml_percent_attr(el: &Element) -> Option<i32> {
    let raw = attr(el, "val")?;
    if let Ok(n) = raw.parse::<i32>() {
        return Some(n);
    }
    let trimmed = raw.trim();
    if trimmed.ends_with('%') {
        let num = trimmed.trim_end_matches('%').trim().parse::<f64>().ok()?;
        return Some((num * 1000.0).round() as i32);
    }
    None
}

fn srgb_to_hex(raw: &str) -> String {
    let hex = raw.trim();
    if hex.starts_with('#') {
        hex.to_uppercase()
    } else {
        format!("#{}", hex.to_uppercase())
    }
}

fn parse_table_style_borders(tc_bdr: &Element) -> Option<TableStyleBorders> {
    let borders = TableStyleBorders {
        left: parse_border_side(child_local(tc_bdr, "left", ns::A)),
        right: parse_border_side(child_local(tc_bdr, "right", ns::A)),
        top: parse_border_side(child_local(tc_bdr, "top", ns::A)),
        bottom: parse_border_side(child_local(tc_bdr, "bottom", ns::A)),
        inside_h: parse_border_side(child_local(tc_bdr, "insideH", ns::A)),
        inside_v: parse_border_side(child_local(tc_bdr, "insideV", ns::A)),
    };
    if borders.left.is_none()
        && borders.right.is_none()
        && borders.top.is_none()
        && borders.bottom.is_none()
        && borders.inside_h.is_none()
        && borders.inside_v.is_none()
    {
        None
    } else {
        Some(borders)
    }
}

fn parse_border_side(side: Option<&Element>) -> Option<TableStyleBorder> {
    let side = side?;
    let ln = child_local(side, "ln", ns::A)?;
    let mut border = TableStyleBorder {
        width_px: 1.0,
        fill: None,
        color: None,
        no_fill: child_local(ln, "noFill", ns::A).is_some(),
        dash: child_local(ln, "prstDash", ns::A)
            .and_then(|d| attr(d, "val"))
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
    };
    let mut has = border.no_fill || border.dash.is_some();
    if let Some(w) = attr(ln, "w")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|w| *w > 0)
    {
        border.width_px = (w as f64 / EMU_PER_PIXEL as f64).max(1.0);
        has = true;
    }
    if let Some(solid) = child_local(ln, "solidFill", ns::A) {
        if let Some(fill) = parse_color_choice_fill(Some(solid)) {
            border.fill = Some(fill);
            has = true;
        } else if let Some(srgb) = child_local(solid, "srgbClr", ns::A) {
            if let Some(val) = attr(srgb, "val") {
                border.color = Some(srgb_to_hex(val));
                has = true;
            }
        }
    }
    has.then_some(border)
}

pub(crate) struct TableBandFlags {
    pub first_row: bool,
    pub last_row: bool,
    pub first_col: bool,
    pub last_col: bool,
    pub band_row: bool,
}

pub(crate) struct TableTheme<'a> {
    pub colors: &'a BTreeMap<String, String>,
    pub map: Option<&'a ThemeColorMap>,
}

impl DrawingColorTheme for TableTheme<'_> {
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

pub(crate) fn resolve_package_cell_style(
    entry: &TableStyleEntry,
    flags: &TableBandFlags,
    row_index: usize,
    col_index: usize,
    row_count: usize,
    col_count: usize,
    theme: &TableTheme<'_>,
) -> (Option<String>, Option<String>, Option<String>) {
    let mut fill: Option<String> = None;
    let mut text: Option<String> = None;

    let mut paint_fill = |part: &TableStylePart| {
        if let Some(f) = part.fill.as_ref() {
            if f.no_fill {
                return;
            }
            if let Some(css) = resolve_style_fill(f, theme) {
                fill = Some(css);
            }
        }
    };
    let mut paint_text = |part: &TableStylePart| {
        if let Some(t) = part.text.as_ref() {
            if let Some(css) = resolve_style_text(t, theme) {
                text = Some(css);
            }
        }
    };

    paint_fill(&entry.whole_tbl);
    paint_text(&entry.whole_tbl);

    let band_start_row = usize::from(flags.first_row);
    let band_end_row = if flags.last_row {
        row_count.saturating_sub(1)
    } else {
        row_count
    };
    if flags.band_row && row_index >= band_start_row && row_index < band_end_row {
        let band_index = row_index - band_start_row;
        if band_index.is_multiple_of(2) {
            paint_fill(&entry.band1_h);
            paint_text(&entry.band1_h);
        } else {
            paint_fill(&entry.band2_h);
            paint_text(&entry.band2_h);
        }
    }

    let at_top = flags.first_row && row_index == 0;
    let at_bottom = flags.last_row && row_index + 1 == row_count;
    let at_left = flags.first_col && col_index == 0;
    let at_right = flags.last_col && col_index + 1 == col_count;

    if at_right {
        paint_fill(&entry.last_col);
        paint_text(&entry.last_col);
    }
    if at_left {
        paint_fill(&entry.first_col);
        paint_text(&entry.first_col);
    }
    if at_bottom {
        paint_fill(&entry.last_row);
        paint_text(&entry.last_row);
    }
    if at_bottom && at_right {
        paint_fill(&entry.se_cell);
        paint_text(&entry.se_cell);
    }
    if at_bottom && at_left {
        paint_fill(&entry.sw_cell);
        paint_text(&entry.sw_cell);
    }
    if at_top {
        paint_fill(&entry.first_row);
        paint_text(&entry.first_row);
    }
    if at_top && at_right {
        paint_fill(&entry.ne_cell);
        paint_text(&entry.ne_cell);
    }
    if at_top && at_left {
        paint_fill(&entry.nw_cell);
        paint_text(&entry.nw_cell);
    }

    let border = resolve_cell_border_css(
        entry, flags, row_index, col_index, row_count, col_count, theme,
    );
    (fill, text, border)
}

fn resolve_style_fill(fill: &TableStyleFill, theme: &TableTheme<'_>) -> Option<String> {
    if fill.no_fill {
        return None;
    }
    let mut base = if let Some(scheme) = fill.scheme_color.as_deref() {
        theme.scheme(scheme)
    } else {
        fill.color.clone()
    };
    if let Some(hex) = base {
        base = Some(apply_tint_shade(&hex, fill.tint, fill.shade));
        return base;
    }
    None
}

fn resolve_style_text(text: &TableStyleText, theme: &TableTheme<'_>) -> Option<String> {
    if let Some(scheme) = text.scheme_color.as_deref() {
        return theme.scheme(scheme);
    }
    text.color.clone()
}

fn apply_tint_shade(hex: &str, tint: Option<i32>, shade: Option<i32>) -> String {
    let mut color = hex.to_owned();
    if let Some(t) = tint.filter(|t| *t > 0) {
        color = tint_srgb(&color, t as f64 / 100_000.0);
    }
    if let Some(s) = shade.filter(|s| *s > 0) {
        color = shade_srgb(&color, 1.0 - s as f64 / 100_000.0);
    }
    color
}

fn tint_srgb(hex: &str, tint_factor: f64) -> String {
    let (r, g, b) = hex_to_rgb_channels(hex).unwrap_or((0.0, 0.0, 0.0));
    let ch = |c: f64| {
        (255.0 - (255.0 - c) * tint_factor)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    format!("#{:02X}{:02X}{:02X}", ch(r), ch(g), ch(b))
}

fn shade_srgb(hex: &str, shade_factor: f64) -> String {
    let (r, g, b) = hex_to_rgb_channels(hex).unwrap_or((0.0, 0.0, 0.0));
    let ch = |c: f64| (c * (1.0 - shade_factor)).round().clamp(0.0, 255.0) as u8;
    format!("#{:02X}{:02X}{:02X}", ch(r), ch(g), ch(b))
}

fn resolve_cell_border_css(
    entry: &TableStyleEntry,
    flags: &TableBandFlags,
    row_index: usize,
    col_index: usize,
    row_count: usize,
    col_count: usize,
    theme: &TableTheme<'_>,
) -> Option<String> {
    let layers = collect_border_layers(entry, flags, row_index, col_index, row_count, col_count);
    if layers.is_empty() {
        return None;
    }
    let region_top = row_index == 0;
    let region_bottom = row_index + 1 == row_count;
    let region_left = col_index == 0;
    let region_right = col_index + 1 == col_count;

    let mut top_winner: Option<&TableStyleBorder> = None;
    let mut bottom_winner: Option<&TableStyleBorder> = None;
    let mut left_winner: Option<&TableStyleBorder> = None;
    let mut right_winner: Option<&TableStyleBorder> = None;

    for borders in &layers {
        let top = if region_top {
            borders.top.as_ref().or(borders.inside_h.as_ref())
        } else {
            borders.inside_h.as_ref()
        };
        if top.is_some() {
            top_winner = top;
        }
        let bottom = if region_bottom {
            borders.bottom.as_ref().or(borders.inside_h.as_ref())
        } else {
            borders.inside_h.as_ref()
        };
        if bottom.is_some() {
            bottom_winner = bottom;
        }
        let left = if region_left {
            borders.left.as_ref().or(borders.inside_v.as_ref())
        } else {
            borders.inside_v.as_ref()
        };
        if left.is_some() {
            left_winner = left;
        }
        let right = if region_right {
            borders.right.as_ref().or(borders.inside_v.as_ref())
        } else {
            borders.inside_v.as_ref()
        };
        if right.is_some() {
            right_winner = right;
        }
    }

    let border = top_winner
        .or(bottom_winner)
        .or(left_winner)
        .or(right_winner)?;
    border_to_css(border, theme)
}

fn push_border_layer<'a>(out: &mut Vec<&'a TableStyleBorders>, part: &'a TableStylePart) {
    if let Some(b) = part.borders.as_ref() {
        out.push(b);
    }
}

fn collect_border_layers<'a>(
    entry: &'a TableStyleEntry,
    flags: &TableBandFlags,
    row_index: usize,
    col_index: usize,
    row_count: usize,
    col_count: usize,
) -> Vec<&'a TableStyleBorders> {
    let mut out: Vec<&'a TableStyleBorders> = Vec::new();

    push_border_layer(&mut out, &entry.whole_tbl);

    let band_start_row = usize::from(flags.first_row);
    let band_end_row = if flags.last_row {
        row_count.saturating_sub(1)
    } else {
        row_count
    };
    if flags.band_row && row_index >= band_start_row && row_index < band_end_row {
        let band_index = row_index - band_start_row;
        if band_index.is_multiple_of(2) {
            push_border_layer(&mut out, &entry.band1_h);
        } else {
            push_border_layer(&mut out, &entry.band2_h);
        }
    }

    let at_top = flags.first_row && row_index == 0;
    let at_bottom = flags.last_row && row_index + 1 == row_count;
    let at_left = flags.first_col && col_index == 0;
    let at_right = flags.last_col && col_index + 1 == col_count;

    if at_right {
        push_border_layer(&mut out, &entry.last_col);
    }
    if at_left {
        push_border_layer(&mut out, &entry.first_col);
    }
    if at_bottom {
        push_border_layer(&mut out, &entry.last_row);
    }
    if at_bottom && at_right {
        push_border_layer(&mut out, &entry.se_cell);
    }
    if at_bottom && at_left {
        push_border_layer(&mut out, &entry.sw_cell);
    }
    if at_top {
        push_border_layer(&mut out, &entry.first_row);
    }
    if at_top && at_right {
        push_border_layer(&mut out, &entry.ne_cell);
    }
    if at_top && at_left {
        push_border_layer(&mut out, &entry.nw_cell);
    }
    out
}

fn border_to_css(border: &TableStyleBorder, theme: &TableTheme<'_>) -> Option<String> {
    if border.no_fill {
        return None;
    }
    let color = border
        .color
        .clone()
        .or_else(|| {
            border
                .fill
                .as_ref()
                .and_then(|f| resolve_style_fill(f, theme))
        })
        .unwrap_or_else(|| "#000000".to_owned());
    let width = border.width_px.max(1.0);
    let style = ooxml_dash_to_css(border.dash.as_deref());
    Some(format!("{width:.0}px {style} {color}"))
}

fn ooxml_dash_to_css(dash: Option<&str>) -> &'static str {
    match dash {
        Some("dot") | Some("sysDot") => "dotted",
        Some("dash")
        | Some("sysDash")
        | Some("lgDash")
        | Some("dashDot")
        | Some("lgDashDot")
        | Some("sysDashDot")
        | Some("lgDashDotDot")
        | Some("sysDashDotDot") => "dashed",
        _ => "solid",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    // Catches: package Table_0 from poi-bug68703 not applied (unstyled table cells).
    #[test]
    fn bug68703_table_0_whole_and_band_fills() {
        let path = format!(
            "{}/../../../fixtures/real/pptx/poi-bug68703.pptx",
            env!("CARGO_MANIFEST_DIR")
        );
        let mut file = std::fs::File::open(path).expect("fixture");
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).expect("read");
        let mut package = nitro_xml_opc::opc::Package::open(&bytes).expect("open");
        let xml = package
            .text("ppt/tableStyles.xml")
            .expect("read part")
            .expect("tableStyles");
        let map = parse_table_style_list(xml);
        let id = "{80500D46-97BE-4507-B105-7B12F621EFCF}";
        let entry = map.get(id).expect("Table_0 style");
        let theme = crate::theme::default_scheme_color_map();
        let table_theme = TableTheme {
            colors: &theme,
            map: None,
        };
        let flags = TableBandFlags {
            first_row: true,
            last_row: false,
            first_col: true,
            last_col: false,
            band_row: true,
        };
        let (fill, text, border) =
            resolve_package_cell_style(entry, &flags, 1, 1, 4, 4, &table_theme);
        assert_eq!(fill.as_deref(), Some("#CFD7E7"));
        assert_eq!(text.as_deref(), Some("#000000"));
        assert!(border.as_ref().is_some_and(|b| b.contains("solid")));
        let (header_fill, header_text, _) =
            resolve_package_cell_style(entry, &flags, 0, 0, 4, 4, &table_theme);
        assert_eq!(header_fill.as_deref(), Some("#4472C4"));
        assert_eq!(header_text.as_deref(), Some("#FFFFFF"));
    }
}
