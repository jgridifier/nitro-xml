// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/metadata.ts; changes listed in PROVENANCE.md.
//! Dynamic-array cell metadata (`xl/metadata.xml`).

use std::collections::HashSet;

use nitro_xml_opc::xml::parse_xml_str;

use crate::xml_util::{att, elements, x_children, x_first};

pub const DYNAMIC_ARRAY_TYPE: &str = "XLDAPR";

/// One-based `cm` values that mark dynamic-array formulas.
pub fn parse_dynamic_array_metadata(xml: Option<&str>) -> HashSet<u32> {
    let mut out = HashSet::new();
    let Some(xml) = xml else { return out };
    let doc = match parse_xml_str(xml) {
        Ok(d) => d,
        Err(_) => return out,
    };
    let root = &doc.root;
    let mut top: std::collections::HashMap<String, Vec<&nitro_xml_opc::xml::Element>> =
        std::collections::HashMap::new();
    for child in elements(root) {
        top.entry(child.local.clone()).or_default().push(child);
    }
    let types = top
        .get("metadataTypes")
        .and_then(|v| v.first())
        .map(|p| {
            x_children(p, "metadataType")
                .into_iter()
                .map(|t| att(Some(t), "name").unwrap_or("").to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let future = top.get("futureMetadata").and_then(|list| {
        list.iter()
            .find(|f| att(Some(*f), "name") == Some(DYNAMIC_ARRAY_TYPE))
    });
    let dynamic: Vec<bool> = future
        .map(|f| {
            x_children(f, "bk")
                .into_iter()
                .map(|bk| {
                    let props = x_first(Some(bk), "dynamicArrayProperties");
                    let flag = att(props, "fDynamic");
                    flag == Some("1") || flag == Some("true")
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(cell_meta) = top.get("cellMetadata").and_then(|v| v.first()) {
        for (index, bk) in x_children(cell_meta, "bk").into_iter().enumerate() {
            for rc in x_children(bk, "rc") {
                let t_idx = num_attr(rc, "t");
                let v_idx = num_attr(rc, "v");
                if let (Some(t), Some(v)) = (t_idx, v_idx) {
                    let type_name = types.get(t as usize - 1).map(String::as_str).unwrap_or("");
                    if type_name == DYNAMIC_ARRAY_TYPE
                        && dynamic.get(v as usize).copied() == Some(true)
                    {
                        out.insert((index + 1) as u32);
                    }
                }
            }
        }
    }
    out
}

fn num_attr(element: &nitro_xml_opc::xml::Element, name: &str) -> Option<f64> {
    crate::xml_util::num_attr(Some(element), name)
}
