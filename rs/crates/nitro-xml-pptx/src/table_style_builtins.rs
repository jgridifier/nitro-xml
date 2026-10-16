// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/table-style-builtins.ts and
// table-style-builtins-data.ts (catalogue payload in table_style_builtins.json, verbatim);
// changes in PROVENANCE.md.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value;

use crate::table_style::{
    normalize_table_style_guid, TableStyleBorder, TableStyleBorders, TableStyleEntry,
    TableStyleFill, TableStylePart, TableStyleText,
};

const CATALOGUE_JSON: &str = include_str!("table_style_builtins.json");

static CATALOGUE: OnceLock<BTreeMap<String, TableStyleEntry>> = OnceLock::new();

/// PowerPoint's built-in gallery style for `style_id`, when it is one of the 74 known GUIDs.
pub(crate) fn builtin_table_style(style_id: &str) -> Option<&'static TableStyleEntry> {
    CATALOGUE
        .get_or_init(build_catalogue)
        .get(&normalize_table_style_guid(style_id))
}

/// Upstream `substituteAccent`: rewrite every `accentN` over the serialised template.
fn substitute_accent(
    template: &Value,
    from: u64,
    to: u64,
    from2: Option<u64>,
    to2: Option<u64>,
) -> Value {
    let mut json = template.to_string();
    if let (Some(from2), Some(to2)) = (from2, to2) {
        json = json.replace(&format!("accent{from2}"), "@@ACC2@@");
        json = json.replace(&format!("accent{from}"), &format!("accent{to}"));
        json = json.replace("@@ACC2@@", &format!("accent{to2}"));
    } else {
        json = json.replace(&format!("accent{from}"), &format!("accent{to}"));
    }
    serde_json::from_str(&json).unwrap_or(Value::Null)
}

fn build_catalogue() -> BTreeMap<String, TableStyleEntry> {
    let mut map = BTreeMap::new();
    let Ok(data) = serde_json::from_str::<Value>(CATALOGUE_JSON) else {
        return map;
    };
    for family in data["families"].as_array().into_iter().flatten() {
        let base = family["baseAccent"].as_u64().unwrap_or(1);
        let base2 = family["baseAccent2"].as_u64();
        for variant in family["variants"].as_array().into_iter().flatten() {
            let (Some(id), Some(a)) = (variant["id"].as_str(), variant["a"].as_u64()) else {
                continue;
            };
            let entry =
                substitute_accent(&family["template"], base, a, base2, variant["b"].as_u64());
            if let Some(entry) = entry_from_json(&entry, id) {
                map.insert(entry.style_id.clone(), entry);
            }
        }
    }
    for entry in data["standalone"].as_array().into_iter().flatten() {
        if let Some(id) = entry["styleId"].as_str() {
            if let Some(entry) = entry_from_json(entry, id) {
                map.insert(entry.style_id.clone(), entry);
            }
        }
    }
    map
}

fn entry_from_json(v: &Value, id: &str) -> Option<TableStyleEntry> {
    if !v.is_object() {
        return None;
    }
    let part = |name: &str| TableStylePart {
        fill: fill_from_json(&v[format!("{name}Fill")]),
        text: text_from_json(&v[format!("{name}Text")]),
        borders: borders_from_json(&v[format!("{name}Borders")]),
    };
    Some(TableStyleEntry {
        style_id: normalize_table_style_guid(id),
        whole_tbl: part("wholeTbl"),
        band1_h: part("band1H"),
        band2_h: part("band2H"),
        band1_v: part("band1V"),
        band2_v: part("band2V"),
        last_col: part("lastCol"),
        first_col: part("firstCol"),
        last_row: part("lastRow"),
        se_cell: part("seCell"),
        sw_cell: part("swCell"),
        first_row: part("firstRow"),
        ne_cell: part("neCell"),
        nw_cell: part("nwCell"),
    })
}

fn fill_from_json(v: &Value) -> Option<TableStyleFill> {
    if !v.is_object() {
        return None;
    }
    let no_fill = v["noFill"].as_bool().unwrap_or(false);
    let scheme = v["schemeColor"].as_str().filter(|s| !s.is_empty());
    let color = v["color"].as_str().filter(|s| !s.is_empty());
    if !no_fill && scheme.is_none() && color.is_none() {
        return None;
    }
    Some(TableStyleFill {
        scheme_color: scheme.map(str::to_owned),
        color: color.map(str::to_owned),
        no_fill,
        tint: v["tint"].as_i64().map(|t| t as i32),
        shade: v["shade"].as_i64().map(|s| s as i32),
    })
}

fn text_from_json(v: &Value) -> Option<TableStyleText> {
    if !v.is_object() {
        return None;
    }
    let scheme = v["fontSchemeColor"].as_str().filter(|s| !s.is_empty());
    let color = v["fontColor"].as_str().filter(|s| !s.is_empty());
    let bold = v["bold"].as_bool().unwrap_or(false);
    if scheme.is_none() && color.is_none() && !bold {
        return None;
    }
    Some(TableStyleText {
        scheme_color: scheme.map(str::to_owned),
        color: color.map(str::to_owned),
        bold,
    })
}

fn border_from_json(v: &Value) -> Option<TableStyleBorder> {
    if !v.is_object() {
        return None;
    }
    Some(TableStyleBorder {
        width_px: v["width"].as_f64().unwrap_or(1.0),
        fill: fill_from_json(&v["fill"]),
        color: v["color"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        no_fill: v["noFill"].as_bool().unwrap_or(false),
        dash: v["dash"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
    })
}

fn borders_from_json(v: &Value) -> Option<TableStyleBorders> {
    if !v.is_object() {
        return None;
    }
    Some(TableStyleBorders {
        left: border_from_json(&v["left"]),
        right: border_from_json(&v["right"]),
        top: border_from_json(&v["top"]),
        bottom: border_from_json(&v["bottom"]),
        inside_h: border_from_json(&v["insideH"]),
        inside_v: border_from_json(&v["insideV"]),
    })
}

#[cfg(test)]
mod tests {
    use super::builtin_table_style;

    // Catches: built-in gallery styles collapsing to one uniform 1px border, which drops the
    // 3pt header-bottom rule of Medium Style 2 (test_slides); accent variants must also differ.
    #[test]
    fn medium_style_2_keeps_thick_header_rule_and_accent_substitution() {
        let a1 = builtin_table_style("{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}").unwrap();
        let bottom = a1
            .first_row
            .borders
            .as_ref()
            .unwrap()
            .bottom
            .as_ref()
            .unwrap();
        assert_eq!(bottom.width_px, 4.0);
        let inside = a1
            .whole_tbl
            .borders
            .as_ref()
            .unwrap()
            .inside_h
            .as_ref()
            .unwrap();
        assert_eq!(inside.width_px, 1.0);
        assert_eq!(
            a1.first_row.fill.as_ref().unwrap().scheme_color.as_deref(),
            Some("accent1")
        );
        let a2 = builtin_table_style("21e4aea4-8dfa-4a89-87eb-49c32662afe0").unwrap();
        assert_eq!(
            a2.first_row.fill.as_ref().unwrap().scheme_color.as_deref(),
            Some("accent2")
        );
        // Negative control: an unknown GUID is not a built-in.
        assert!(builtin_table_style("{00000000-0000-0000-0000-000000000000}").is_none());
    }
}
