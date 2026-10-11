// SPDX-License-Identifier: MIT
//! Thin body/table walker matching upstream `block-parser.ts` / `parse-table.ts` paragraph ids.

use serde_json::{json, Map, Value};

use nitro_xml_opc::xml::Element;

use crate::simple_types::parse_integer;
use crate::styles::parse_formatting;
use crate::styles::ParagraphProps;
use crate::xml_util::{children_w, first_w, get_w, named_w, ElementExt};

pub fn scan_paragraphs(body: Option<&Element>) -> Vec<ParagraphProps> {
    let mut out = Vec::new();
    if let Some(body) = body {
        scan_container(body, "", &mut out);
    }
    out
}

fn scan_container(container: &Element, id_prefix: &str, out: &mut Vec<ParagraphProps>) {
    let mut index = 0;
    for child in container.element_children() {
        if named_w(child, "p") {
            out.push(parse_paragraph_direct(
                child,
                format!("{}p{}", id_prefix, index),
            ));
            index += 1;
        } else if named_w(child, "tbl") {
            scan_table(child, format!("{}t{}", id_prefix, index), out);
            index += 1;
        }
    }
}

fn scan_table(table: &Element, id: String, out: &mut Vec<ParagraphProps>) {
    for (ri, row) in children_w(table, "tr").into_iter().enumerate() {
        for (ci, cell) in children_w(row, "tc").into_iter().enumerate() {
            let cell_id = format!("{}-r{}c{}", id, ri, ci);
            for (pi, p) in children_w(cell, "p").into_iter().enumerate() {
                out.push(parse_paragraph_direct(p, format!("{}p{}", cell_id, pi)));
            }
        }
    }
}

fn parse_paragraph_direct(node: &Element, id: String) -> ParagraphProps {
    let props = first_w(Some(node), "pPr");
    let mut direct = parse_formatting(props);
    let map = direct.as_object_mut().unwrap();
    insert_style(map, props);
    insert_numbering(map, props);
    ParagraphProps {
        id,
        style: get_w(first_w(props, "pStyle"), "val"),
        numbering: parse_numbering(props),
        direct,
    }
}

fn insert_style(map: &mut Map<String, Value>, props: Option<&Element>) {
    if let Some(s) = get_w(first_w(props, "pStyle"), "val") {
        map.insert("style".into(), json!(s));
    }
}

fn parse_numbering(props: Option<&Element>) -> Option<(i64, i64)> {
    let num_pr = first_w(props, "numPr")?;
    let num_id = parse_integer(get_w(first_w(Some(num_pr), "numId"), "val").as_deref())?;
    let level = parse_integer(get_w(first_w(Some(num_pr), "ilvl"), "val").as_deref()).unwrap_or(0);
    Some((num_id, level))
}

fn insert_numbering(map: &mut Map<String, Value>, props: Option<&Element>) {
    if let Some((num_id, level)) = parse_numbering(props) {
        map.insert(
            "numbering".into(),
            json!({ "numId": num_id, "level": level }),
        );
    }
}
