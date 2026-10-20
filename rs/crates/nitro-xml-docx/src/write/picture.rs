// SPDX-License-Identifier: MIT
//! Inline pictures: `w:drawing` on the run, plus `word/media` and the document relationship.

use indexmap::IndexMap;
use serde_json::Value;

use nitro_xml_opc::drawingml::pixels_to_emu;
use nitro_xml_opc::opc::rel_types::IMAGE;
use nitro_xml_opc::opc::{relationships_part_for, Package, RelMode, Relationship};
use nitro_xml_opc::write::PackageWriter;
use nitro_xml_opc::xml::{ns, Attr, Element, Node};

use crate::edit::host::DocxHost;

const CT_PNG: &str = "image/png";
const CT_RELS: &str = "application/vnd.openxmlformats-package.relationships+xml";
const DOCUMENT_PART: &str = "word/document.xml";
const PICTURE_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";

struct PictureJob {
    token: String,
    part_name: String,
    bytes: Vec<u8>,
}

/// `w:drawing` / `wp:inline` for a run image that carries PNG bytes and an embed token.
/// Parser-loaded images have neither, so this returns `None` and the caller keeps the old drawing.
pub fn inline_drawing(image: &Value) -> Option<Element> {
    let token = image
        .get("embedToken")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())?;
    png_bytes(image)?;
    let cx = pixels_to_emu(px_of(image, "widthPx") as f64);
    let cy = pixels_to_emu(px_of(image, "heightPx") as f64);
    let doc_id = token
        .strip_prefix("nitroPic")
        .and_then(|n| n.parse::<i64>().ok())
        .unwrap_or(1);
    Some(drawing_tree(token, doc_id, cx, cy))
}

/// Replace embed tokens in `document_xml` and add the media parts plus the image relationship.
pub fn write_inline_pictures(
    writer: &mut PackageWriter<'_>,
    original: &[u8],
    host: &DocxHost,
    document_xml: &mut String,
) -> Result<(), String> {
    let jobs = jobs_from_host(host);
    if jobs.is_empty() {
        return Ok(());
    }
    let mut pkg = Package::open(original).map_err(|e| e.to_string())?;
    let existing = pkg.rels(DOCUMENT_PART).map_err(|e| e.to_string())?.clone();
    let mut next = 1u32;
    for id in existing.keys() {
        if let Some(n) = id.strip_prefix("rId").and_then(|n| n.parse::<u32>().ok()) {
            next = next.max(n + 1);
        }
    }
    let mut map: IndexMap<String, Relationship> = existing;
    for job in &jobs {
        let needle = format!("r:embed=\"{}\"", job.token);
        if !document_xml.contains(&needle) {
            return Err(format!(
                "picture token {} missing from document.xml",
                job.token
            ));
        }
        let rel_id = format!("rId{next}");
        next += 1;
        let repl = format!("r:embed=\"{rel_id}\"");
        *document_xml = document_xml.replace(&needle, &repl);
        let file = job
            .part_name
            .rsplit('/')
            .next()
            .unwrap_or(job.part_name.as_str());
        map.insert(
            rel_id.clone(),
            Relationship {
                id: rel_id,
                type_: IMAGE.to_owned(),
                target: format!("media/{file}"),
                mode: RelMode::Internal,
            },
        );
        writer.add(
            job.part_name.as_str(),
            nitro_xml_opc::write::PartWrite::bytes(job.bytes.clone()).with_content_type(CT_PNG),
        );
    }
    let rels_part = relationships_part_for(DOCUMENT_PART);
    if pkg.has(&rels_part) {
        writer.replace(&rels_part, rels_xml(&map).into_bytes());
    } else {
        writer.add_text(&rels_part, rels_xml(&map), Some(CT_RELS));
    }
    Ok(())
}

fn jobs_from_host(host: &DocxHost) -> Vec<PictureJob> {
    let mut used: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut jobs = Vec::new();
    for block in &host.blocks {
        let Some(runs) = block.get("runs").and_then(|r| r.as_array()) else {
            continue;
        };
        for run in runs {
            let Some(image) = run.get("image") else {
                continue;
            };
            let Some(token) = image
                .get("embedToken")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            else {
                continue;
            };
            let Some(bytes) = png_bytes(image) else {
                continue;
            };
            let preferred = image.get("partName").and_then(|v| v.as_str());
            let part_name = alloc_part(&mut used, preferred);
            jobs.push(PictureJob {
                token: token.to_owned(),
                part_name,
                bytes,
            });
        }
    }
    jobs
}

fn alloc_part(used: &mut std::collections::BTreeSet<String>, preferred: Option<&str>) -> String {
    if let Some(name) = preferred {
        if !name.is_empty() && used.insert(name.to_owned()) {
            return name.to_owned();
        }
    }
    let mut n = 1u32;
    loop {
        let name = format!("word/media/nitro-edit-{n}.png");
        if used.insert(name.clone()) {
            return name;
        }
        n += 1;
    }
}

fn png_bytes(image: &Value) -> Option<Vec<u8>> {
    let arr = image.get("png")?.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for v in arr {
        let n = v.as_u64()?;
        if n > 255 {
            return None;
        }
        out.push(n as u8);
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn px_of(image: &Value, key: &str) -> i64 {
    image
        .get(key)
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|n| n.round() as i64)))
        .filter(|n| *n > 0)
        .unwrap_or(64)
}

fn drawing_tree(token: &str, doc_id: i64, cx: i64, cy: i64) -> Element {
    let mut blip = el(ns::A, "blip");
    blip.attrs.push(Attr {
        ns: Some(ns::R.to_owned()),
        local: "embed".to_owned(),
        value: token.to_owned(),
    });
    let mut stretch = el(ns::A, "stretch");
    stretch.children.push(Node::Element(el(ns::A, "fillRect")));
    let mut blip_fill = el(ns::PIC, "blipFill");
    blip_fill.children.push(Node::Element(blip));
    blip_fill.children.push(Node::Element(stretch));

    let mut off = el(ns::A, "off");
    set_bare(&mut off, "x", "0");
    set_bare(&mut off, "y", "0");
    let mut ext = el(ns::A, "ext");
    set_bare(&mut ext, "cx", &cx.to_string());
    set_bare(&mut ext, "cy", &cy.to_string());
    let mut xfrm = el(ns::A, "xfrm");
    xfrm.children.push(Node::Element(off));
    xfrm.children.push(Node::Element(ext));
    let mut geom = el(ns::A, "prstGeom");
    set_bare(&mut geom, "prst", "rect");
    geom.children.push(Node::Element(el(ns::A, "avLst")));
    let mut sp_pr = el(ns::PIC, "spPr");
    sp_pr.children.push(Node::Element(xfrm));
    sp_pr.children.push(Node::Element(geom));

    let mut c_nv_pr = el(ns::PIC, "cNvPr");
    set_bare(&mut c_nv_pr, "id", "0");
    set_bare(&mut c_nv_pr, "name", "Picture");
    let mut nv = el(ns::PIC, "nvPicPr");
    nv.children.push(Node::Element(c_nv_pr));
    nv.children.push(Node::Element(el(ns::PIC, "cNvPicPr")));

    let mut pic = el(ns::PIC, "pic");
    pic.children.push(Node::Element(nv));
    pic.children.push(Node::Element(blip_fill));
    pic.children.push(Node::Element(sp_pr));

    let mut data = el(ns::A, "graphicData");
    set_bare(&mut data, "uri", PICTURE_URI);
    data.children.push(Node::Element(pic));
    let mut graphic = el(ns::A, "graphic");
    graphic.children.push(Node::Element(data));

    let mut extent = el(ns::WP, "extent");
    set_bare(&mut extent, "cx", &cx.to_string());
    set_bare(&mut extent, "cy", &cy.to_string());
    let mut doc_pr = el(ns::WP, "docPr");
    set_bare(&mut doc_pr, "id", &doc_id.to_string());
    set_bare(&mut doc_pr, "name", &format!("Picture {doc_id}"));
    let mut inline = el(ns::WP, "inline");
    inline.children.push(Node::Element(extent));
    inline.children.push(Node::Element(doc_pr));
    inline.children.push(Node::Element(graphic));

    let mut drawing = el(ns::W, "drawing");
    drawing.children.push(Node::Element(inline));
    drawing
}

fn el(uri: &str, local: &str) -> Element {
    Element {
        ns: Some(uri.to_owned()),
        local: local.to_owned(),
        attrs: Vec::new(),
        children: Vec::new(),
    }
}

fn set_bare(element: &mut Element, local: &str, value: &str) {
    element.attrs.push(Attr {
        ns: None,
        local: local.to_owned(),
        value: value.to_owned(),
    });
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

fn xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}
