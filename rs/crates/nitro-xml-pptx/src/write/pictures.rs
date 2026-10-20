// SPDX-License-Identifier: MIT
//! Media parts and slide relationships for pictures inserted this session.

use std::collections::{BTreeSet, HashMap};

use indexmap::IndexMap;

use nitro_xml_opc::opc::rel_types::{IMAGE, SLIDE_LAYOUT};
use nitro_xml_opc::opc::{relationships_part_for, RelMode, Relationship};
use nitro_xml_opc::write::{PackageWriter, PartWrite};

use crate::edit::host::EditableSlide;

const CT_PNG: &str = "image/png";

pub struct PlannedPicture {
    pub rel_id: String,
    pub part_name: String,
    pub bytes: Vec<u8>,
}

/// Assign a fresh `rId` and `ppt/media/nitro-N.png` for each element that carries PNG bytes.
pub fn plan_pictures(
    slide: &EditableSlide,
    rels: &IndexMap<String, Relationship>,
    used_parts: &mut BTreeSet<String>,
) -> (HashMap<String, String>, Vec<PlannedPicture>) {
    let mut next = 1u32;
    for id in rels.keys() {
        if let Some(n) = id.strip_prefix("rId").and_then(|n| n.parse::<u32>().ok()) {
            next = next.max(n + 1);
        }
    }
    let mut map = HashMap::new();
    let mut planned = Vec::new();
    for el in &slide.elements {
        let Some(bytes) = el.image_png.clone() else {
            continue;
        };
        let rel_id = format!("rId{next}");
        next += 1;
        let part_name = alloc_media(used_parts);
        map.insert(el.id.clone(), rel_id.clone());
        planned.push(PlannedPicture {
            rel_id,
            part_name,
            bytes,
        });
    }
    (map, planned)
}

fn alloc_media(used: &mut BTreeSet<String>) -> String {
    let mut n = 1u32;
    loop {
        let name = format!("ppt/media/nitro-{n}.png");
        if used.insert(name.clone()) {
            return name;
        }
        n += 1;
    }
}

/// Add the PNG parts and merge image relationships into the slide `.rels`.
/// No-op when `planned` is empty, so an edit that is not a picture leaves rels untouched.
pub fn attach_pictures(
    writer: &mut PackageWriter<'_>,
    slide_path: &str,
    rels: &IndexMap<String, Relationship>,
    planned: &[PlannedPicture],
) -> Result<(), String> {
    if planned.is_empty() {
        return Ok(());
    }
    let mut map = rels.clone();
    for pic in planned {
        let file = pic
            .part_name
            .rsplit('/')
            .next()
            .unwrap_or(pic.part_name.as_str());
        map.insert(
            pic.rel_id.clone(),
            Relationship {
                id: pic.rel_id.clone(),
                type_: IMAGE.to_owned(),
                target: format!("../media/{file}"),
                mode: RelMode::Internal,
            },
        );
        writer.add(
            pic.part_name.as_str(),
            PartWrite::bytes(pic.bytes.clone()).with_content_type(CT_PNG),
        );
    }
    writer.add_text(relationships_part_for(slide_path), rels_xml(&map), None);
    Ok(())
}

/// Copy `rels` with the slideLayout `Target` replaced.
/// `None` when every slideLayout relationship already uses `layout_target`.
pub(crate) fn retarget_slide_layout(
    rels: &IndexMap<String, Relationship>,
    layout_target: &str,
) -> Option<IndexMap<String, Relationship>> {
    let mut changed = false;
    let mut out = rels.clone();
    for rel in out.values_mut() {
        if rel.type_ == SLIDE_LAYOUT && rel.target != layout_target {
            rel.target = layout_target.to_owned();
            changed = true;
        }
    }
    changed.then_some(out)
}

pub(crate) fn rels_xml(rels: &IndexMap<String, Relationship>) -> String {
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

fn xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}
