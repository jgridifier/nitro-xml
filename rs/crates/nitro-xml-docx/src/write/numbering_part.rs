// SPDX-License-Identifier: MIT
//! Write `word/numbering.xml` and the document relationship that points at it.

use indexmap::IndexMap;
use serde_json::Value;

use nitro_xml_opc::opc::rel_types::NUMBERING;
use nitro_xml_opc::opc::{relationships_part_for, Package, RelMode, Relationship};
use nitro_xml_opc::write::PackageWriter;

use crate::edit::host::DocxHost;

const CT_NUMBERING: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
const CT_RELS: &str = "application/vnd.openxmlformats-package.relationships+xml";
const NUMBERING_PART: &str = "word/numbering.xml";
const DOCUMENT_PART: &str = "word/document.xml";

/// Definitions the host catalog can round-trip through `parse_numbering_catalog`.
struct ListDef {
    num_id: u32,
    abs_id: String,
    fmt: String,
    lvl_text: String,
    start: u32,
}

/// Append any catalog list the package does not already contain, and link the part.
pub fn write_numbering(
    writer: &mut PackageWriter<'_>,
    original: &[u8],
    host: &DocxHost,
) -> Result<(), String> {
    if !numbering_changed(host) {
        return Ok(());
    }
    let Some(catalog) = host.numbering_catalog.as_ref() else {
        return Ok(());
    };
    let defs = defs_from_catalog(catalog);
    if defs.is_empty() {
        return Ok(());
    }
    let mut pkg = Package::open(original).map_err(|e| e.to_string())?;
    let existing = pkg
        .text(NUMBERING_PART)
        .map_err(|e| e.to_string())?
        .map(str::to_owned);
    let xml = merge_numbering_xml(existing.as_deref(), &defs);
    writer.add_text(NUMBERING_PART, xml, Some(CT_NUMBERING));
    ensure_numbering_rel(writer, &mut pkg)?;
    Ok(())
}

fn numbering_changed(host: &DocxHost) -> bool {
    host.blocks.iter().enumerate().any(|(i, block)| {
        let now = block.get("numbering");
        let base = host.base_blocks.get(i).and_then(|b| b.get("numbering"));
        now != base
    })
}

fn defs_from_catalog(catalog: &Value) -> Vec<ListDef> {
    let Some(nums) = catalog.get("nums").and_then(|n| n.as_object()) else {
        return Vec::new();
    };
    let Some(abs) = catalog.get("abstractNums").and_then(|n| n.as_object()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (id, num) in nums {
        let Some(num_id) = id.parse::<u32>().ok() else {
            continue;
        };
        let Some(abs_id) = num.get("abstractNumId").and_then(|v| v.as_str()) else {
            continue;
        };
        let lvl = abs
            .get(abs_id)
            .and_then(|a| a.get("levels"))
            .and_then(|l| l.get("0"));
        let fmt = lvl
            .and_then(|l| l.get("numFmt"))
            .and_then(|v| v.as_str())
            .unwrap_or("decimal");
        let lvl_text = lvl
            .and_then(|l| l.get("lvlText"))
            .and_then(|v| v.as_str())
            .unwrap_or(if fmt == "bullet" { "•" } else { "%1." });
        let start = lvl
            .and_then(|l| l.get("start"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;
        out.push(ListDef {
            num_id,
            abs_id: abs_id.to_owned(),
            fmt: fmt.to_owned(),
            lvl_text: lvl_text.to_owned(),
            start,
        });
    }
    out.sort_by_key(|d| d.num_id);
    out
}

fn merge_numbering_xml(existing: Option<&str>, defs: &[ListDef]) -> String {
    let missing: Vec<&ListDef> = defs
        .iter()
        .filter(|d| match existing {
            Some(xml) => !xml.contains(&format!("w:numId=\"{}\"", d.num_id)),
            None => true,
        })
        .collect();
    if let Some(xml) = existing {
        if missing.is_empty() {
            return xml.to_owned();
        }
        let snippet: String = missing.iter().map(|d| def_xml(d)).collect();
        if let Some(at) = xml.rfind("</w:numbering>") {
            let mut out = String::new();
            out.push_str(&xml[..at]);
            out.push_str(&snippet);
            out.push_str(&xml[at..]);
            return out;
        }
    }
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    );
    for def in defs {
        out.push_str(&def_xml(def));
    }
    out.push_str("</w:numbering>");
    out
}

fn def_xml(def: &ListDef) -> String {
    format!(
        r#"<w:abstractNum w:abstractNumId="{abs}"><w:lvl w:ilvl="0"><w:start w:val="{start}"/><w:numFmt w:val="{fmt}"/><w:lvlText w:val="{text}"/></w:lvl></w:abstractNum><w:num w:numId="{id}"><w:abstractNumId w:val="{abs}"/></w:num>"#,
        abs = xml_attr(&def.abs_id),
        start = def.start,
        fmt = xml_attr(&def.fmt),
        text = xml_attr(&def.lvl_text),
        id = def.num_id,
    )
}

fn xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

fn ensure_numbering_rel(writer: &mut PackageWriter<'_>, pkg: &mut Package) -> Result<(), String> {
    let rels = pkg.rels(DOCUMENT_PART).map_err(|e| e.to_string())?;
    if rels.values().any(|r| r.type_ == NUMBERING) {
        return Ok(());
    }
    let mut next = 1u32;
    for id in rels.keys() {
        if let Some(n) = id.strip_prefix("rId").and_then(|n| n.parse::<u32>().ok()) {
            next = next.max(n + 1);
        }
    }
    let mut map: IndexMap<String, Relationship> = rels.clone();
    let id = format!("rId{next}");
    map.insert(
        id.clone(),
        Relationship {
            id,
            type_: NUMBERING.to_owned(),
            target: "numbering.xml".to_owned(),
            mode: RelMode::Internal,
        },
    );
    let part = relationships_part_for(DOCUMENT_PART);
    writer.add_text(part, rels_xml(&map), Some(CT_RELS));
    Ok(())
}

fn rels_xml(rels: &IndexMap<String, Relationship>) -> String {
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for rel in rels.values() {
        out.push_str("<Relationship Id=\"");
        out.push_str(&xml_attr(&rel.id));
        out.push_str("\" Type=\"");
        out.push_str(&xml_attr(&rel.type_));
        out.push_str("\" Target=\"");
        out.push_str(&xml_attr(&rel.target));
        out.push('"');
        if rel.mode == RelMode::External {
            out.push_str(" TargetMode=\"External\"");
        }
        out.push_str("/>");
    }
    out.push_str("</Relationships>");
    out
}
