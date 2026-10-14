// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/drawing.ts; changes listed in
// PROVENANCE.md.
//! Spreadsheet drawing part (`xdr:wsDr`) → sheet drawing objects for the live grid.

use nitro_xml_opc::opc::relationship_types;
use nitro_xml_opc::opc::Package;
use nitro_xml_opc::xml::{ns, parse_xml_str, Element};

use crate::model::{DrawingAnchor, DrawingMarker, DrawingObject};
use crate::xml_util::{a_first, att, children_ns, elements, first_ns, rel_attr};

const ANCHOR_PARTS: &[&str] = &["from", "to", "ext", "pos", "clientData"];

fn xdr_first<'a>(parent: Option<&'a Element>, local: &str) -> Option<&'a Element> {
    parent.and_then(|p| first_ns(p, local, ns::XDR))
}

fn int_child(parent: Option<&Element>, local: &str) -> i64 {
    xdr_first(parent, local)
        .map(|el| el.text_content())
        .and_then(|t| t.trim().parse::<f64>().ok())
        .map(|n| n.round() as i64)
        .unwrap_or(0)
}

fn marker(node: Option<&Element>) -> DrawingMarker {
    DrawingMarker {
        col: int_child(node, "col").max(0) as u32,
        col_offset: int_child(node, "colOff"),
        row: int_child(node, "row").max(0) as u32,
        row_offset: int_child(node, "rowOff"),
    }
}

fn read_anchor(anchor: &Element) -> DrawingAnchor {
    if anchor.local == "absoluteAnchor" {
        let pos = xdr_first(Some(anchor), "pos");
        let mut result = DrawingAnchor {
            from: DrawingMarker {
                row: 0,
                col: 0,
                col_offset: att(pos, "x").and_then(|v| v.parse().ok()).unwrap_or(0),
                row_offset: att(pos, "y").and_then(|v| v.parse().ok()).unwrap_or(0),
            },
            to: None,
            ext: None,
        };
        if let Some(ext) = xdr_first(Some(anchor), "ext") {
            result.ext = Some((
                att(Some(ext), "cx")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0),
                att(Some(ext), "cy")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0),
            ));
        }
        return result;
    }
    let mut result = DrawingAnchor {
        from: marker(xdr_first(Some(anchor), "from")),
        to: None,
        ext: None,
    };
    if let Some(to) = xdr_first(Some(anchor), "to") {
        result.to = Some(marker(Some(to)));
    }
    if let Some(ext) = xdr_first(Some(anchor), "ext") {
        result.ext = Some((
            att(Some(ext), "cx")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            att(Some(ext), "cy")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        ));
    }
    result
}

fn non_visual(content: &Element) -> Option<&Element> {
    let nv = elements(content).find(|node| node.local.starts_with("nv"));
    xdr_first(nv, "cNvPr")
}

fn describe(content: &Element) -> String {
    match content.local.as_str() {
        "sp" => {
            let nv = xdr_first(Some(content), "nvSpPr");
            let c_nv = xdr_first(nv, "cNvSpPr");
            if att(c_nv, "txBox") == Some("1") {
                "text box".into()
            } else {
                "shape".into()
            }
        }
        "grpSp" => "group".into(),
        "cxnSp" => "connector".into(),
        "contentPart" => "ink".into(),
        "AlternateContent" => "form control or extended object".into(),
        other => other.to_string(),
    }
}

fn unsupported(anchor: DrawingAnchor, description: impl Into<String>) -> DrawingObject {
    DrawingObject::Unsupported {
        anchor,
        description: description.into(),
    }
}

/// Parse `xdr:wsDr` into drawing objects (pictures/charts modelled; else unsupported).
pub fn parse_drawing(
    pkg: &mut Package,
    part_name: &str,
    warn: &mut impl FnMut(String),
) -> Vec<DrawingObject> {
    let Some(xml) = pkg.text(part_name).ok().flatten().map(|s| s.to_owned()) else {
        return Vec::new();
    };
    let Ok(doc) = parse_xml_str(&xml) else {
        return Vec::new();
    };
    let root = &doc.root;
    let rels = pkg.rels(part_name).ok().cloned();
    let mut out = Vec::new();
    for anchor_el in elements(root) {
        if !matches!(
            anchor_el.local.as_str(),
            "twoCellAnchor" | "oneCellAnchor" | "absoluteAnchor"
        ) {
            continue;
        }
        let anchor = read_anchor(anchor_el);
        let content = elements(anchor_el).find(|node| !ANCHOR_PARTS.contains(&node.local.as_str()));
        let Some(content) = content else {
            continue;
        };
        let c_nv = non_visual(content);
        let name = att(c_nv, "name").map(|s| s.to_owned());
        if content.local == "pic" {
            let blip_fill = xdr_first(Some(content), "blipFill");
            let blip = a_first(blip_fill, "blip");
            let rid = rel_attr(blip, "embed").unwrap_or("");
            let target = rels
                .as_ref()
                .and_then(|map| map.get(rid).and_then(|rel| pkg.target(part_name, rel)));
            if let Some(target) = target.filter(|t| pkg.has(t)) {
                let content_type = pkg
                    .content_type(&target)
                    .unwrap_or("application/octet-stream")
                    .to_owned();
                out.push(DrawingObject::Image {
                    anchor,
                    part_name: target,
                    content_type,
                    name,
                    description: att(c_nv, "descr").map(|s| s.to_owned()),
                });
            } else {
                out.push(unsupported(anchor, "linked picture"));
            }
            continue;
        }
        if content.local == "graphicFrame" {
            let graphic = a_first(Some(content), "graphic");
            let data = a_first(graphic, "graphicData");
            let chart_ref = data.and_then(|d| first_ns(d, "chart", ns::C));
            let rid = rel_attr(chart_ref, "id").unwrap_or("");
            let target = rels
                .as_ref()
                .and_then(|map| map.get(rid).and_then(|rel| pkg.target(part_name, rel)));
            if let Some(target) = target {
                match pkg.text(&target).map(|t| t.map(|s| s.to_owned())) {
                    Ok(Some(chart_xml)) => {
                        out.push(DrawingObject::Chart {
                            anchor,
                            part_name: target,
                            name,
                            chart_xml,
                        });
                    }
                    _ => {
                        warn(format!(
                            "Chart {target} could not be read; it is kept but not shown."
                        ));
                        out.push(unsupported(anchor, "chart"));
                    }
                }
                continue;
            }
            let uri = att(data, "uri").unwrap_or("");
            let desc = if uri.contains("diagram") {
                "smartArt"
            } else if uri.contains("chartex") {
                "chart (newer type)"
            } else {
                "graphic frame"
            };
            out.push(unsupported(anchor, desc));
            continue;
        }
        out.push(unsupported(anchor, describe(content)));
    }
    out
}

/// Load drawings for a worksheet part via its `drawing` relationship (or `<drawing r:id>`).
pub fn load_sheet_drawings(
    pkg: &mut Package,
    worksheet_part: &str,
    sheet_xml_root: Option<&Element>,
    warn: &mut impl FnMut(String),
) -> Vec<DrawingObject> {
    let mut target = None;
    if let Some(root) = sheet_xml_root {
        if let Some(drawing) = children_ns(root, "drawing", ns::X)
            .into_iter()
            .next()
            .or_else(|| elements(root).find(|e| e.local == "drawing"))
        {
            if let Some(rid) = rel_attr(Some(drawing), "id") {
                let rel = pkg
                    .rels(worksheet_part)
                    .ok()
                    .and_then(|rels| rels.get(rid).cloned());
                if let Some(rel) = rel {
                    target = pkg.target(worksheet_part, &rel);
                }
            }
        }
    }
    if target.is_none() {
        target = pkg
            .target_of_type(worksheet_part, relationship_types::DRAWING)
            .ok()
            .flatten();
    }
    let Some(target) = target else {
        return Vec::new();
    };
    parse_drawing(pkg, &target, warn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_excel_with_attachments_missing_unsupported_placeholders() {
        // Catches: ExcelWithAttachments shapes/groups never become .xg-obj-unsupported drawings.
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/real/xlsx/poi-ExcelWithAttachments.xlsm"
        ))
        .expect("fixture");
        let mut pkg = Package::open(&bytes).expect("pkg");
        let sheet = "xl/worksheets/sheet1.xml";
        let mut warns = Vec::new();
        let mut warn = |m: String| warns.push(m);
        let drawings = load_sheet_drawings(&mut pkg, sheet, None, &mut warn);
        assert!(
            drawings
                .iter()
                .any(|d| matches!(d, DrawingObject::Unsupported { .. })),
            "expected unsupported drawing placeholders, got {drawings:?}"
        );
    }
}
