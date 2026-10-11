// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/worksheet.ts (readHyperlinks);
// changes listed in PROVENANCE.md.

use nitro_xml_opc::opc::{Package, RelMode, Relationship};
use nitro_xml_opc::xml::Element;
use std::collections::HashMap;

use crate::address::parse_range;
use crate::model::HyperlinkInfo;
use crate::xml_util::{att, rel_attr, x_children};

pub fn read_hyperlinks(
    pkg: &mut Package,
    worksheet_part: &str,
    node: Option<&Element>,
) -> Vec<HyperlinkInfo> {
    let Some(node) = node else {
        return Vec::new();
    };
    let rels: HashMap<String, Relationship> = pkg
        .rels(worksheet_part)
        .ok()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    let mut out = Vec::new();
    for link in x_children(node, "hyperlink") {
        let range = att(Some(link), "ref").and_then(parse_range);
        let Some(range) = range else {
            continue;
        };
        let target = rel_attr(Some(link), "id").and_then(|id| rels.get(id)).map(|rel| {
            if rel.mode == RelMode::External {
                rel.target.clone()
            } else {
                pkg.target(worksheet_part, rel)
                    .unwrap_or_else(|| rel.target.clone())
            }
        });
        out.push(HyperlinkInfo {
            range,
            tooltip: att(Some(link), "tooltip").map(str::to_owned),
            target,
            location: att(Some(link), "location").map(str::to_owned),
        });
    }
    out
}
