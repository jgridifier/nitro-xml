// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/comments.ts (legacy comment refs);
// changes listed in PROVENANCE.md.

use nitro_xml_opc::opc::relationship_types;
use nitro_xml_opc::opc::Package;
use nitro_xml_opc::xml::parse_xml_str;

use crate::address::parse_address;
use crate::model::CellAddress;
use crate::xml_util::{att, x_children, x_first};

/// Thin wrapper: skip the XML parse body when the sheet has no comments part so cold open
/// of comment-free fixtures does not TurboFan-compile comment parsing (#30 peak_mem).
pub fn load_comment_addresses(pkg: &mut Package, worksheet_part: &str) -> Vec<CellAddress> {
    let part = pkg
        .target_of_type(worksheet_part, relationship_types::COMMENTS)
        .ok()
        .flatten();
    let Some(part) = part else {
        return Vec::new();
    };
    let Some(xml) = pkg.text(&part).ok().flatten() else {
        return Vec::new();
    };
    parse_legacy_comment_addresses_outlined(xml)
}

#[inline(never)]
fn parse_legacy_comment_addresses_outlined(xml: &str) -> Vec<CellAddress> {
    parse_legacy_comment_addresses(Some(xml))
}

fn parse_legacy_comment_addresses(xml: Option<&str>) -> Vec<CellAddress> {
    let Some(xml) = xml else {
        return Vec::new();
    };
    let doc = match parse_xml_str(xml) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let root = &doc.root;
    let list = x_first(Some(root), "commentList").unwrap_or(root);
    x_children(list, "comment")
        .into_iter()
        .filter_map(|node| att(Some(node), "ref").and_then(parse_address))
        .collect()
}
