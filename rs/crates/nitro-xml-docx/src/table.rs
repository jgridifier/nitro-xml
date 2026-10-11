// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/parse-table.ts and write-table.ts
// (canEditTableStructure); changes listed in PROVENANCE.md.

use serde_json::{json, Value};

use nitro_xml_opc::xml::{ns, Element};

use crate::borders::{parse_shading_fill, parse_table_borders};
use crate::simple_types::{
    on_off_attribute, on_off_element, parse_signed_twips, parse_twips, parse_unsigned_integer,
};
use crate::xml_util::{children_w, first_w, get_w, named_w, ElementExt};

pub fn parse_table(
    node: &Element,
    id: &str,
    parse_paragraph: &mut dyn FnMut(&Element, &str) -> Value,
) -> Value {
    let rows: Vec<Vec<Value>> = children_w(node, "tr")
        .iter()
        .enumerate()
        .map(|(ri, row)| {
            children_w(row, "tc")
                .iter()
                .enumerate()
                .map(|(ci, cell)| {
                    parse_cell(cell, parse_paragraph, format!("{}-r{}c{}", id, ri, ci))
                })
                .collect()
        })
        .collect();
    let mut table = json!({
        "type": "table",
        "id": id,
        "rows": rows,
        "structureEditable": can_edit_table_structure(node),
    });
    if let Some(obj) = table.as_object_mut() {
        let grid_element = first_w(Some(node), "tblGrid");
        if let Some(grid_el) = grid_element {
            let grid: Vec<u64> = children_w(grid_el, "gridCol")
                .iter()
                .filter_map(|col| parse_twips(get_w(Some(col), "w").as_deref()))
                .collect();
            if !grid.is_empty() {
                obj.insert("grid".into(), json!(grid));
            }
        }
        let tbl_pr = first_w(Some(node), "tblPr");
        let table_width = first_w(tbl_pr, "tblW");
        if get_w(table_width, "type").as_deref().unwrap_or("dxa") == "dxa" {
            if let Some(w) = parse_twips(get_w(table_width, "w").as_deref()) {
                obj.insert("widthTwips".into(), json!(w));
            }
        }
        let justification = get_w(first_w(tbl_pr, "jc"), "val");
        if let Some(j) = justification {
            if is_st_jc_table(&j) {
                obj.insert("justification".into(), json!(j));
                let rtl = on_off_element(first_w(tbl_pr, "bidiVisual")) == Some(true);
                let alignment = match j.as_str() {
                    "start" => {
                        if rtl {
                            "right"
                        } else {
                            "left"
                        }
                    }
                    "end" => {
                        if rtl {
                            "left"
                        } else {
                            "right"
                        }
                    }
                    "center" => "center",
                    _ => j.as_str(),
                };
                obj.insert("alignment".into(), json!(alignment));
            }
        }
        if let Some(indent) = parse_signed_twips(get_w(first_w(tbl_pr, "tblInd"), "w").as_deref()) {
            obj.insert("indentTwips".into(), json!(indent));
        }
        if let Some(b) = parse_table_borders(first_w(tbl_pr, "tblBorders")) {
            obj.insert("borders".into(), b);
        }
        if let Some(style) = get_w(first_w(tbl_pr, "tblStyle"), "val") {
            obj.insert("style".into(), json!(style));
        }
        if let Some(look) = parse_look(first_w(tbl_pr, "tblLook")) {
            obj.insert("look".into(), look);
        }
        if let Some(m) = parse_margins(first_w(tbl_pr, "tblCellMar")) {
            obj.insert("cellMargins".into(), m);
        }
        let row_properties: Vec<Value> = children_w(node, "tr")
            .iter()
            .map(|r| parse_row_properties(r))
            .collect();
        if row_properties
            .iter()
            .any(|r| !r.as_object().map(|o| o.is_empty()).unwrap_or(true))
        {
            obj.insert("rowProperties".into(), json!(row_properties));
        }
    }
    table
}

fn parse_cell(
    cell: &Element,
    parse_paragraph: &mut dyn FnMut(&Element, &str) -> Value,
    id: String,
) -> Value {
    let props = first_w(Some(cell), "tcPr");
    let paragraphs: Vec<Value> = children_w(cell, "p")
        .iter()
        .enumerate()
        .map(|(pi, p)| parse_paragraph(p, &format!("{}p{}", id, pi)))
        .collect();
    let mut result = json!({ "paragraphs": paragraphs });
    if let Some(obj) = result.as_object_mut() {
        if let Some(gs) =
            parse_unsigned_integer(get_w(first_w(props, "gridSpan"), "val").as_deref())
        {
            obj.insert("gridSpan".into(), json!(gs));
        }
        if let Some(vm) = first_w(props, "vMerge") {
            let val = if get_w(Some(vm), "val").as_deref() == Some("restart") {
                "restart"
            } else {
                "continue"
            };
            obj.insert("verticalMerge".into(), json!(val));
        }
        let tc_w = first_w(props, "tcW");
        if get_w(tc_w, "type").as_deref().unwrap_or("dxa") == "dxa" {
            if let Some(w) = parse_twips(get_w(tc_w, "w").as_deref()) {
                obj.insert("widthTwips".into(), json!(w));
            }
        }
        let v_align = get_w(first_w(props, "vAlign"), "val");
        if matches!(
            v_align.as_deref(),
            Some("top") | Some("center") | Some("bottom")
        ) {
            obj.insert("verticalAlign".into(), json!(v_align.unwrap()));
        }
        let shd = first_w(props, "shd");
        let fill = parse_shading_fill(shd).or_else(|| {
            if get_w(shd, "fill").as_deref() == Some("auto") && get_w(shd, "themeFill").is_none() {
                Some("auto".to_string())
            } else {
                None
            }
        });
        if let Some(fill) = fill {
            obj.insert("shadingFill".into(), json!(fill));
        }
        if let Some(b) = parse_table_borders(first_w(props, "tcBorders")) {
            obj.insert("borders".into(), b);
        }
        if let Some(m) = parse_margins(first_w(props, "tcMar")) {
            obj.insert("margins".into(), m);
        }
        let nested: Vec<Value> = children_w(cell, "tbl")
            .iter()
            .map(|n| nested_preview(n))
            .collect();
        if !nested.is_empty() {
            obj.insert("nestedTables".into(), json!(nested));
        }
    }
    result
}

fn nested_preview(node: &Element) -> Value {
    let rows: Vec<Vec<Value>> = children_w(node, "tr")
        .iter()
        .map(|row| {
            children_w(row, "tc")
                .iter()
                .map(|cell| {
                    let text = by_name_w_text(cell).join("");
                    json!({ "text": text })
                })
                .collect()
        })
        .collect();
    json!({ "rows": rows })
}

fn by_name_w_text(cell: &Element) -> Vec<String> {
    let mut out = Vec::new();
    collect_t(cell, &mut out);
    out
}

fn collect_t(el: &Element, out: &mut Vec<String>) {
    if el.local == "t" && el.ns.as_deref() == Some(ns::W) {
        out.push(el.text_content());
    }
    for c in el.element_children() {
        collect_t(c, out);
    }
}

fn parse_margins(element: Option<&Element>) -> Option<Value> {
    let element = element?;
    let mut parsed = serde_json::Map::new();
    for (side, logical) in [
        ("top", "top"),
        ("bottom", "bottom"),
        ("left", "start"),
        ("right", "end"),
    ] {
        let edge = first_w(Some(element), logical).or_else(|| first_w(Some(element), side));
        let edge_type = get_w(edge, "type").unwrap_or_else(|| "dxa".to_string());
        let value = if edge_type == "nil" {
            parse_signed_twips(Some("0"))
        } else if edge_type == "dxa" {
            parse_signed_twips(get_w(edge, "w").as_deref())
        } else {
            None
        };
        if let Some(v) = value {
            parsed.insert(side.into(), json!(v));
        }
    }
    if parsed.is_empty() {
        None
    } else {
        Some(Value::Object(parsed))
    }
}

fn parse_look(element: Option<&Element>) -> Option<Value> {
    let element = element?;
    let flags: [(&str, u16); 6] = [
        ("firstRow", 0x0020),
        ("lastRow", 0x0040),
        ("firstColumn", 0x0080),
        ("lastColumn", 0x0100),
        ("noHBand", 0x0200),
        ("noVBand", 0x0400),
    ];
    let named_flags = flags
        .iter()
        .any(|(name, _)| get_w(Some(element), name).is_some());
    let raw = get_w(Some(element), "val");
    let packed = if !named_flags {
        raw.filter(|r| r.len() == 4 && r.chars().all(|c| c.is_ascii_hexdigit()))
            .and_then(|r| u16::from_str_radix(&r, 16).ok())
    } else {
        None
    };
    let mut look = serde_json::Map::new();
    for (name, bit) in flags {
        let value = if let Some(p) = packed {
            (p & bit) != 0
        } else {
            on_off_attribute(element, name).unwrap_or(false)
        };
        look.insert(name.into(), json!(value));
    }
    if look.is_empty() {
        None
    } else {
        Some(Value::Object(look))
    }
}

fn parse_row_properties(row: &Element) -> Value {
    let tr_pr = first_w(Some(row), "trPr");
    let mut result = serde_json::Map::new();
    let height = first_w(tr_pr, "trHeight");
    if let Some(ht) = parse_twips(get_w(height, "val").as_deref()) {
        result.insert("heightTwips".into(), json!(ht));
        let rule = if get_w(height, "hRule").as_deref() == Some("exact") {
            "exact"
        } else {
            "atLeast"
        };
        result.insert("heightRule".into(), json!(rule));
    }
    if on_off_element(first_w(tr_pr, "cantSplit")) == Some(true) {
        result.insert("cantSplit".into(), json!(true));
    }
    if on_off_element(first_w(tr_pr, "tblHeader")) == Some(true) {
        result.insert("header".into(), json!(true));
    }
    Value::Object(result)
}

fn is_st_jc_table(value: &str) -> bool {
    matches!(
        value,
        "left"
            | "center"
            | "right"
            | "both"
            | "mediumKashida"
            | "distribute"
            | "numTab"
            | "highKashida"
            | "lowKashida"
            | "thaiDistribute"
            | "start"
            | "end"
    )
}

pub fn can_edit_table_structure(table: &Element) -> bool {
    fn only(parent: &Element, allowed: &[&str]) -> bool {
        parent
            .element_children()
            .iter()
            .all(|node| allowed.iter().any(|name| named_w(node, name)))
    }
    if !only(table, &["tblPr", "tblGrid", "tr"]) {
        return false;
    }
    let grid = first_w(Some(table), "tblGrid");
    if let Some(g) = grid {
        if !only(g, &["gridCol"]) {
            return false;
        }
    }
    let rows = children_w(table, "tr");
    if rows.is_empty() {
        return false;
    }
    let width = children_w(rows[0], "tc").len();
    if width == 0 {
        return false;
    }
    if let Some(g) = grid {
        if children_w(g, "gridCol").len() != width {
            return false;
        }
    }
    for row in rows {
        if !only(row, &["trPr", "tc"]) || children_w(row, "tc").len() != width {
            return false;
        }
        let row_props = first_w(Some(row), "trPr");
        if let Some(rp) = row_props {
            for key in ["gridBefore", "gridAfter", "trPrChange"] {
                if first_w(Some(rp), key).is_some() {
                    return false;
                }
            }
        }
        for cell in children_w(row, "tc") {
            if !only(cell, &["tcPr", "p"]) || children_w(cell, "p").is_empty() {
                return false;
            }
            let props = first_w(Some(cell), "tcPr");
            if let Some(p) = props {
                for key in ["gridSpan", "vMerge", "hMerge", "tcPrChange"] {
                    if first_w(Some(p), key).is_some() {
                        return false;
                    }
                }
            }
            for paragraph in children_w(cell, "p") {
                if !only(paragraph, &["pPr", "r"]) {
                    return false;
                }
                for run in children_w(paragraph, "r") {
                    if !only(run, &["rPr", "t", "tab", "br", "cr", "noBreakHyphen"]) {
                        return false;
                    }
                }
            }
        }
    }
    true
}
