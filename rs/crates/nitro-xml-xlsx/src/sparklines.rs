// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/sparklines.ts and
// src/core/xlsx/model-sparklines.ts; changes listed in PROVENANCE.md.

use nitro_xml_opc::xml::parse_xml_str;

use crate::address::parse_range;
use crate::model::{ActiveWorksheet, Sparkline, SparklineGroup};
use crate::style_parts::parse_color;
use crate::xml_util::{
    att, bool_attr_or, children_ns, descendants, element_text, first_ns, num_attr,
};

pub const NS_X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
pub const NS_XM: &str = "http://schemas.microsoft.com/office/excel/2006/main";

fn pick_str(value: Option<&str>, allowed: &[&str], fallback: &str) -> String {
    let value = value.unwrap_or("");
    allowed
        .iter()
        .find(|a| **a == value)
        .map(|s| s.to_string())
        .unwrap_or_else(|| fallback.to_string())
}

fn read_sparkline_group(node: &nitro_xml_opc::xml::Element) -> SparklineGroup {
    let type_ = pick_str(
        att(Some(node), "type"),
        &["line", "column", "stacked"],
        "line",
    );
    let group = SparklineGroup {
        type_,
        line_weight: num_attr(Some(node), "lineWeight").unwrap_or(0.75),
        date_axis: bool_attr_or(Some(node), "dateAxis", false),
        date_formula: None,
        markers: bool_attr_or(Some(node), "markers", false),
        high: bool_attr_or(Some(node), "high", false),
        low: bool_attr_or(Some(node), "low", false),
        first: bool_attr_or(Some(node), "first", false),
        last: bool_attr_or(Some(node), "last", false),
        negative: bool_attr_or(Some(node), "negative", false),
        display_empty_cells_as: pick_str(
            att(Some(node), "displayEmptyCellsAs"),
            &["gap", "zero", "span"],
            "zero",
        ),
        display_x_axis: bool_attr_or(Some(node), "displayXAxis", false),
        display_hidden: bool_attr_or(Some(node), "displayHidden", false),
        right_to_left: bool_attr_or(Some(node), "rightToLeft", false),
        min_axis_type: pick_str(
            att(Some(node), "minAxisType"),
            &["individual", "group", "custom"],
            "individual",
        ),
        max_axis_type: pick_str(
            att(Some(node), "maxAxisType"),
            &["individual", "group", "custom"],
            "individual",
        ),
        manual_min: num_attr(Some(node), "manualMin"),
        manual_max: num_attr(Some(node), "manualMax"),
        color_series: parse_color(first_ns(node, "colorSeries", NS_X14), None),
        color_negative: parse_color(first_ns(node, "colorNegative", NS_X14), None),
        color_axis: parse_color(first_ns(node, "colorAxis", NS_X14), None),
        color_markers: parse_color(first_ns(node, "colorMarkers", NS_X14), None),
        color_first: parse_color(first_ns(node, "colorFirst", NS_X14), None),
        color_last: parse_color(first_ns(node, "colorLast", NS_X14), None),
        color_high: parse_color(first_ns(node, "colorHigh", NS_X14), None),
        color_low: parse_color(first_ns(node, "colorLow", NS_X14), None),
        sparklines: Vec::new(),
    };
    let mut group = group;
    group.date_formula = element_text(first_ns(node, "f", NS_XM));
    let sparklines_parent = first_ns(node, "sparklines", NS_X14).unwrap_or(node);
    for line in children_ns(sparklines_parent, "sparkline", NS_X14) {
        let host = element_text(first_ns(line, "sqref", NS_XM))
            .and_then(|s| parse_range(&s.replace('$', "")));
        if host.is_none() {
            continue;
        }
        let formula = element_text(first_ns(line, "f", NS_XM));
        group.sparklines.push(Sparkline {
            formula,
            host: host.unwrap(),
        });
    }
    group
}

/// Every sparkline group under a parsed `extLst` element, in document order.
pub fn read_sparkline_groups_from_ext(ext_lst: &nitro_xml_opc::xml::Element) -> Vec<SparklineGroup> {
    descendants(ext_lst, "sparklineGroup", NS_X14)
        .into_iter()
        .map(read_sparkline_group)
        .collect()
}

/// Every sparkline group in preserved `extLst` fragments (best-effort; unknown prefixes may skip).
pub fn read_sparkline_groups(ext_lst: &[String]) -> Vec<SparklineGroup> {
    let mut groups = Vec::new();
    for xml in ext_lst {
        if !xml.contains("sparklineGroup") {
            continue;
        }
        let sanitized = strip_unknown_prefixed_attributes(xml);
        let doc = match parse_xml_str(&sanitized) {
            Ok(d) => d,
            Err(_) => continue,
        };
        for node in descendants(&doc.root, "sparklineGroup", NS_X14) {
            groups.push(read_sparkline_group(node));
        }
    }
    groups
}

fn strip_unknown_prefixed_attributes(xml: &str) -> String {
    let bytes = xml.as_bytes();
    let mut out = String::with_capacity(xml.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b" xr2:") || bytes[i..].starts_with(b" xr:") {
            while i < bytes.len() && bytes[i] != b' ' && bytes[i] != b'>' && bytes[i] != b'/' {
                i += 1;
            }
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

pub fn sheet_sparkline_groups(sheet: &ActiveWorksheet) -> &[SparklineGroup] {
    &sheet.sparkline_groups
}

/// Extract `<extLst>...</extLst>` fragments from worksheet XML (preserved extension list).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{load_workbook, parse_all_worksheets};

    #[test]
    fn reads_sparkline_groups_from_fixture() {
        // Catches: view-IR misses sparklines when extLst is not preserved on ActiveWorksheet.
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/upstream/xlsx/8b581ee14978_excel-sparklines.xlsx"
        );
        let bytes = std::fs::read(path).expect("fixture");
        let wb = load_workbook(&bytes).expect("workbook");
        let sheets = parse_all_worksheets(&bytes, &wb).expect("sheets");
        let sheet = sheets[0].as_ref().expect("sheet0");
        assert!(!sheet.ext_lst.is_empty(), "extLst fragments");
        assert_eq!(sheet.sparkline_groups.len(), 4);
    }
}

pub fn extract_ext_lst(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search = xml;
    while let Some(start) = search.find("<extLst") {
        let rest = &search[start..];
        if let Some(end_rel) = rest.find("</extLst>") {
            let end = end_rel + "</extLst>".len();
            out.push(rest[..end].to_string());
            search = &rest[end..];
        } else {
            break;
        }
    }
    out
}
