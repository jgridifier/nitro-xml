// SPDX-License-Identifier: MIT
//! Slide add / duplicate / delete / reorder on the package.
//!
//! Shape saves only replace dirty `ppt/slides/slideN.xml` bytes. A slide-list
//! edit also rewrites `ppt/presentation.xml` `p:sldIdLst`, the presentation
//! relationships, `[Content_Types].xml` overrides, and the new or removed parts.

use std::collections::{BTreeSet, HashMap};

use nitro_xml_opc::opc::rel_types::SLIDE;
use nitro_xml_opc::opc::{parse_relationships, Package, RelMode, Relationship};
use nitro_xml_opc::write::PackageWriter;

use crate::edit::host::{EditableSlide, PptxHost};

use super::slide_xml::{max_numeric_id, rewrite_slide};

const SLIDE_CT: &str = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";

const BLANK_SLIDE: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#;

const LAYOUT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#;

pub fn save_with_structure(host: &PptxHost, original: &[u8]) -> Result<Vec<u8>, String> {
    let mut pkg = Package::open(original).map_err(|e| e.to_string())?;
    let pres = pkg
        .text("ppt/presentation.xml")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "missing ppt/presentation.xml".to_string())?
        .to_owned();
    let rels_xml = pkg
        .text("ppt/_rels/presentation.xml.rels")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "missing presentation relationships".to_string())?
        .to_owned();
    let rels = parse_relationships(Some(&rels_xml)).map_err(|e| e.to_string())?;

    let names: Vec<String> = pkg
        .part_names()
        .filter(|name| {
            name.starts_with("ppt/slides/slide")
                && name.ends_with(".xml")
                && !name.contains("_rels")
        })
        .map(str::to_owned)
        .collect();
    let mut part_xml: HashMap<String, String> = HashMap::new();
    let mut part_rels: HashMap<String, String> = HashMap::new();
    let mut max_id = 1i64;
    for name in &names {
        if let Some(xml) = pkg.text(name).map_err(|e| e.to_string())? {
            max_id = max_id.max(max_numeric_id(xml)?);
            part_xml.insert(name.clone(), xml.to_owned());
        }
        let rels_name = slide_rels_name(name);
        if let Some(xml) = pkg.text(&rels_name).map_err(|e| e.to_string())? {
            part_rels.insert(name.clone(), xml.to_owned());
        }
    }

    let mut used_numbers: BTreeSet<u32> = part_xml.keys().filter_map(|n| slide_number(n)).collect();
    let mut next_number = used_numbers.iter().next_back().copied().unwrap_or(0) + 1;

    let mut assigned: Vec<String> = Vec::with_capacity(host.slides.len());
    for slide in &host.slides {
        if part_xml.contains_key(&slide.path) {
            assigned.push(slide.path.clone());
        } else {
            while used_numbers.contains(&next_number) {
                next_number += 1;
            }
            let path = format!("ppt/slides/slide{next_number}.xml");
            used_numbers.insert(next_number);
            next_number += 1;
            assigned.push(path);
        }
    }

    let mut next_id = max_id + 1;
    let mut writer = PackageWriter::open(original).map_err(|e| e.to_string())?;
    let kept: BTreeSet<&str> = assigned.iter().map(String::as_str).collect();

    for (slide, path) in host.slides.iter().zip(assigned.iter()) {
        let is_new = !part_xml.contains_key(path);
        let dirty = host.dirty_parts.contains(&slide.path) || is_new;
        if !dirty && !is_new {
            continue;
        }
        let source_xml = source_xml(slide, &part_xml);
        let rewritten = rewrite_slide(&source_xml, slide, &mut next_id)?;
        if is_new {
            writer.add_text(path, rewritten, Some(SLIDE_CT));
            let rels = part_rels
                .get(slide.source_slide_id.as_deref().unwrap_or(""))
                .map(String::as_str)
                .unwrap_or(LAYOUT_RELS);
            writer.add_text(slide_rels_name(path), rels, None);
        } else {
            writer.replace(path, rewritten.into_bytes());
        }
    }

    for old in part_xml.keys() {
        if kept.contains(old.as_str()) {
            continue;
        }
        writer.remove(old);
        writer.remove(&slide_rels_name(old));
        writer
            .content_types_mut()
            .overrides
            .remove(&format!("/{old}"));
    }

    let (pres_out, rels_out) = rewrite_presentation(&pres, &rels, &assigned)?;
    writer.replace("ppt/presentation.xml", pres_out.into_bytes());
    writer.replace("ppt/_rels/presentation.xml.rels", rels_out.into_bytes());
    writer.finish().map_err(|e| e.to_string())
}

fn source_xml(slide: &EditableSlide, parts: &HashMap<String, String>) -> String {
    if let Some(xml) = parts.get(&slide.path) {
        return xml.clone();
    }
    if let Some(src) = slide.source_slide_id.as_deref() {
        if let Some(xml) = parts.get(src) {
            return xml.clone();
        }
    }
    BLANK_SLIDE.to_owned()
}

fn slide_rels_name(slide_part: &str) -> String {
    let file = slide_part.rsplit('/').next().unwrap_or(slide_part);
    format!("ppt/slides/_rels/{file}.rels")
}

fn slide_number(part: &str) -> Option<u32> {
    let file = part.rsplit('/').next()?;
    let n = file.strip_prefix("slide")?.strip_suffix(".xml")?;
    n.parse().ok()
}

fn rewrite_presentation(
    pres: &str,
    rels: &indexmap::IndexMap<String, Relationship>,
    slides: &[String],
) -> Result<(String, String), String> {
    let mut target_to_rid: HashMap<String, String> = HashMap::new();
    let mut rid_to_sld = sld_id_by_rid(pres);
    let mut max_sld = rid_to_sld.values().copied().max().unwrap_or(255);
    let mut max_rid = 0u32;
    for id in rels.keys() {
        if let Some(n) = id.strip_prefix("rId").and_then(|n| n.parse().ok()) {
            max_rid = max_rid.max(n);
        }
    }
    for rel in rels.values() {
        if rel.type_ == SLIDE {
            let part = if rel.target.starts_with("ppt/") {
                rel.target.clone()
            } else {
                format!("ppt/{}", rel.target.trim_start_matches('/'))
            };
            target_to_rid.insert(part, rel.id.clone());
        }
    }

    let mut slide_rels = Vec::new();
    let mut sld_entries = Vec::new();
    for part in slides {
        let rid = if let Some(rid) = target_to_rid.get(part) {
            rid.clone()
        } else {
            max_rid += 1;
            let rid = format!("rId{max_rid}");
            rid
        };
        let sld = if let Some(id) = rid_to_sld.get(&rid).copied() {
            id
        } else {
            max_sld += 1;
            max_sld
        };
        rid_to_sld.insert(rid.clone(), sld);
        let target = part.trim_start_matches("ppt/");
        slide_rels.push(Relationship {
            id: rid.clone(),
            type_: SLIDE.to_owned(),
            target: target.to_owned(),
            mode: RelMode::Internal,
        });
        sld_entries.push((sld, rid));
    }

    let pres_out = replace_sld_id_list(pres, &sld_entries)?;
    let rels_out = write_rels(rels, &slide_rels);
    Ok((pres_out, rels_out))
}

fn sld_id_by_rid(pres: &str) -> HashMap<String, u32> {
    let mut out = HashMap::new();
    let Some(start) = pres.find("<p:sldIdLst>") else {
        return out;
    };
    let Some(end) = pres.find("</p:sldIdLst>") else {
        return out;
    };
    let body = &pres[start..end];
    for tag in body.split("<p:sldId").skip(1) {
        let id = attr_value(tag, "id").and_then(|v| v.parse().ok());
        let rid = attr_value(tag, "r:id");
        if let (Some(id), Some(rid)) = (id, rid) {
            out.insert(rid, id);
        }
    }
    out
}

fn attr_value(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let i = tag.find(&key)? + key.len();
    let rest = &tag[i..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

fn replace_sld_id_list(pres: &str, entries: &[(u32, String)]) -> Result<String, String> {
    let start = pres
        .find("<p:sldIdLst>")
        .ok_or_else(|| "presentation has no sldIdLst".to_string())?;
    let end = pres
        .find("</p:sldIdLst>")
        .ok_or_else(|| "presentation has no sldIdLst end".to_string())?
        + "</p:sldIdLst>".len();
    let mut body = String::from("<p:sldIdLst>");
    for (id, rid) in entries {
        body.push_str(&format!(r#"<p:sldId id="{id}" r:id="{rid}"/>"#));
    }
    body.push_str("</p:sldIdLst>");
    let mut out = String::new();
    out.push_str(&pres[..start]);
    out.push_str(&body);
    out.push_str(&pres[end..]);
    Ok(out)
}

fn write_rels(
    existing: &indexmap::IndexMap<String, Relationship>,
    slides: &[Relationship],
) -> String {
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for rel in existing.values() {
        if rel.type_ == SLIDE {
            continue;
        }
        push_rel(&mut out, rel);
    }
    for rel in slides {
        push_rel(&mut out, rel);
    }
    out.push_str("</Relationships>");
    out
}

fn push_rel(out: &mut String, rel: &Relationship) {
    out.push_str("<Relationship Id=\"");
    out.push_str(&rel.id);
    out.push_str("\" Type=\"");
    out.push_str(&rel.type_);
    out.push_str("\" Target=\"");
    out.push_str(&rel.target);
    out.push('"');
    if rel.mode == RelMode::External {
        out.push_str(" TargetMode=\"External\"");
    }
    out.push_str("/>");
}
