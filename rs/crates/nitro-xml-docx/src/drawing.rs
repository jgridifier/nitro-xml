// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/drawing.ts and diagram.ts (host side);
// changes listed in PROVENANCE.md.

use std::collections::HashSet;

use indexmap::IndexMap;
use serde_json::{json, Value};

use nitro_xml_opc::drawingml::emu_to_pixels;
use nitro_xml_opc::opc::relationship_types;
use nitro_xml_opc::opc::{
    content_type_for_part, resolve_part_path, ContentTypes, RelMode, Relationship,
};
use nitro_xml_opc::xml::Element;

use crate::chart_host::{parse_chart_graphic, CHART_GRAPHIC_URI};
use crate::xml_util::{descendant_ns, first_ns, get_r, ElementExt};

const WP_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const VML_NS: &str = "urn:schemas-microsoft-com:vml";
const DGM_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
const PICTURE_GRAPHIC_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
const SVG_NS: &str = "http://schemas.microsoft.com/office/drawing/2016/SVG/main";

#[derive(Clone)]
pub struct DrawingContext {
    pub rels: IndexMap<String, Relationship>,
    pub content_types: ContentTypes,
    pub media_parts: HashSet<String>,
}

fn emu_to_px_attr(value: Option<&str>) -> i64 {
    let n = value.and_then(|v| v.parse::<i64>().ok());
    n.map(|v| emu_to_pixels(v).round() as i64).unwrap_or(0)
}

fn guess_content_type(part_name: &str) -> &'static str {
    match part_name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "tif" | "tiff" => "image/tiff",
        "emf" => "image/x-emf",
        "wmf" => "image/x-wmf",
        _ => "application/octet-stream",
    }
}

fn resolve_picture(rel_id: &str, context: &DrawingContext) -> Option<Value> {
    let rel = context.rels.get(rel_id)?;
    if rel.mode == RelMode::External {
        return None;
    }
    let part_name = resolve_part_path("word/document.xml", &rel.target);
    if !context.media_parts.contains(&part_name) {
        return None;
    }
    let content_type = content_type_for_part(&context.content_types, &part_name)
        .unwrap_or_else(|| guess_content_type(&part_name));
    Some(json!({
        "relId": rel_id,
        "partName": part_name,
        "contentType": content_type,
    }))
}

fn svg_blip_rel_id(blip: &Element) -> Option<String> {
    let svg_blip = descendant_ns(Some(blip), SVG_NS, "svgBlip")?;
    get_r(Some(svg_blip), "embed")
}

fn unsupported_kind_label(uri: &str) -> &'static str {
    if uri.contains("wordprocessingShape") {
        "Shape"
    } else if uri.contains("/chart") {
        "Chart"
    } else if uri.contains("/diagram") {
        "SmartArt"
    } else if uri.contains("/oleObject") || uri.contains("/ole") {
        "Embedded object"
    } else {
        "Drawing object"
    }
}

fn parse_modern_drawing(node: &Element, context: &DrawingContext) -> Value {
    let inline = first_ns(Some(node), WP_NS, "inline");
    let anchor_only = first_ns(Some(node), WP_NS, "anchor");
    let anchor = inline.or(anchor_only).unwrap_or(node);
    let extent = first_ns(Some(anchor), WP_NS, "extent");
    let doc_pr = first_ns(Some(anchor), WP_NS, "docPr");
    let width_px = emu_to_px_attr(extent.and_then(|e| {
        e.attrs
            .iter()
            .find(|a| a.local == "cx")
            .map(|a| a.value.as_str())
    }));
    let height_px = emu_to_px_attr(extent.and_then(|e| {
        e.attrs
            .iter()
            .find(|a| a.local == "cy")
            .map(|a| a.value.as_str())
    }));
    let alt_text = doc_pr
        .and_then(|d| {
            d.attrs
                .iter()
                .find(|a| a.local == "descr")
                .map(|a| a.value.clone())
        })
        .filter(|s| !s.is_empty());
    let title = doc_pr
        .and_then(|d| {
            d.attrs
                .iter()
                .find(|a| a.local == "title")
                .map(|a| a.value.clone())
        })
        .filter(|s| !s.is_empty());
    let anchored = inline.is_none() && anchor_only.is_some();
    let graphic_data = descendant_ns(Some(anchor), A_NS, "graphicData");
    let uri = graphic_data
        .and_then(|g| {
            g.attrs
                .iter()
                .find(|a| a.local == "uri")
                .map(|a| a.value.clone())
        })
        .unwrap_or_default();
    let blip = graphic_data.and_then(|g| descendant_ns(Some(g), A_NS, "blip"));
    let rel_id = blip.as_ref().and_then(|b| get_r(Some(b), "embed"));
    let picture = if uri == PICTURE_GRAPHIC_URI {
        rel_id.as_ref().and_then(|r| resolve_picture(r, context))
    } else {
        None
    };
    let svg = picture.as_ref().and_then(|_| {
        blip.as_ref()
            .and_then(|b| svg_blip_rel_id(b))
            .and_then(|r| resolve_picture(&r, context))
    });
    if let Some(mut pic) = picture {
        if let Some(obj) = pic.as_object_mut() {
            obj.insert("widthPx".into(), json!(width_px));
            obj.insert("heightPx".into(), json!(height_px));
            if let Some(a) = alt_text {
                obj.insert("altText".into(), json!(a));
            }
            if let Some(t) = title {
                obj.insert("title".into(), json!(t));
            }
            if anchored {
                obj.insert("anchored".into(), json!(true));
            }
            if let Some(svg_part) = svg.and_then(|s| s.get("partName").cloned()) {
                obj.insert("svgPartName".into(), svg_part);
            }
        }
        return pic;
    }
    let diagram = if uri == DGM_NS {
        Some(parse_diagram_graphic(
            graphic_data.unwrap(),
            extent,
            doc_pr,
            context,
            anchored,
        ))
    } else {
        None
    };
    let chart = if uri == CHART_GRAPHIC_URI {
        Some(parse_chart_graphic(
            graphic_data.unwrap(),
            doc_pr,
            &context.rels,
            anchored,
        ))
    } else {
        None
    };
    let unsupported = unsupported_kind_label(&uri);
    let mut base = json!({
        "relId": rel_id.unwrap_or_default(),
        "partName": "",
        "contentType": "application/octet-stream",
        "widthPx": width_px,
        "heightPx": height_px,
        "unsupported": unsupported,
    });
    if let Some(obj) = base.as_object_mut() {
        if let Some(a) = alt_text {
            obj.insert("altText".into(), json!(a));
        }
        if let Some(t) = title {
            obj.insert("title".into(), json!(t));
        }
        if anchored {
            obj.insert("anchored".into(), json!(true));
        }
        if let Some(d) = diagram {
            obj.insert("diagram".into(), d);
        }
        if let Some(c) = chart {
            obj.insert("chart".into(), c);
        }
    }
    base
}

fn parse_diagram_graphic(
    graphic_data: &Element,
    extent: Option<&Element>,
    doc_pr: Option<&Element>,
    context: &DrawingContext,
    is_anchor: bool,
) -> Value {
    let rel_ids_el = graphic_data
        .element_children()
        .into_iter()
        .find(|c| c.ns.as_deref() == Some(DGM_NS) && c.local == "relIds");
    let mut relationship_ids = serde_json::Map::new();
    if let Some(r) = rel_ids_el {
        for attr in &r.attrs {
            if attr.local.ends_with("Id") || attr.local.contains("Rel") {
                relationship_ids.insert(attr.local.clone(), json!(attr.value));
            }
        }
    }
    let mut issues = Vec::new();
    let mut parts = serde_json::Map::new();
    let mut host_relationships = serde_json::Map::new();
    let slots = [
        ("data", "dataRelId", relationship_types::DIAGRAM_DATA),
        ("layout", "layoutRelId", relationship_types::DIAGRAM_LAYOUT),
        (
            "quickStyle",
            "styleRelId",
            relationship_types::DIAGRAM_QUICK_STYLE,
        ),
        ("colors", "colorsRelId", relationship_types::DIAGRAM_COLORS),
    ];
    for (name, key, _rel_type) in slots {
        let rel_id = relationship_ids
            .get(key)
            .or_else(|| relationship_ids.get(&key.replace("RelId", "")))
            .and_then(|v| v.as_str());
        if let Some(rid) = rel_id {
            if let Some(rel) = context.rels.get(rid) {
                if rel.mode != RelMode::External {
                    let part_name = resolve_part_path("word/document.xml", &rel.target);
                    host_relationships.insert(rid.into(), json!(part_name));
                    parts.insert(name.into(), json!({ "relId": rid, "partName": part_name }));
                } else {
                    issues.push(json!({
                        "code": "DIAGRAM_RELATIONSHIP_UNRESOLVED",
                        "message": format!("Diagram relationship {} does not point at a part in this package.", rid),
                    }));
                }
            }
        }
    }
    if rel_ids_el.is_none() {
        issues.push(json!({
            "code": "DIAGRAM_RELIDS_MISSING",
            "message": "The graphic has no dgm:relIds, so no diagram part can be located.",
        }));
    }
    for rel in context.rels.values() {
        if rel.type_ == relationship_types::DIAGRAM_DRAWING && rel.mode != RelMode::External {
            host_relationships.insert(
                rel.id.clone(),
                json!(resolve_part_path("word/document.xml", &rel.target)),
            );
        }
    }
    let width = extent
        .and_then(|e| e.attrs.iter().find(|a| a.local == "cx"))
        .and_then(|a| a.value.parse::<i64>().ok())
        .unwrap_or(0);
    let height = extent
        .and_then(|e| e.attrs.iter().find(|a| a.local == "cy"))
        .and_then(|a| a.value.parse::<i64>().ok())
        .unwrap_or(0);
    let mut obj = serde_json::Map::new();
    obj.insert(
        "placement".into(),
        json!(if is_anchor { "anchor" } else { "inline" }),
    );
    obj.insert(
        "extentEmu".into(),
        json!({ "width": width, "height": height }),
    );
    if let Some(dp) = doc_pr {
        if let Some(name) = dp
            .attrs
            .iter()
            .find(|a| a.local == "name")
            .map(|a| a.value.clone())
        {
            obj.insert("name".into(), json!(name));
        }
        if let Some(id) = dp
            .attrs
            .iter()
            .find(|a| a.local == "id")
            .map(|a| a.value.clone())
        {
            obj.insert("id".into(), json!(id));
        }
    }
    obj.insert("relationshipIds".into(), Value::Object(relationship_ids));
    obj.insert("parts".into(), Value::Object(parts));
    obj.insert(
        "hostRelationships".into(),
        Value::Object(host_relationships),
    );
    obj.insert("nodes".into(), json!([]));
    obj.insert("rendering".into(), json!("placeholder"));
    obj.insert(
        "notice".into(),
        json!("SmartArt is shown from the layout cached in the file when available; Word layout is not recomputed."),
    );
    obj.insert("issues".into(), json!(issues));
    Value::Object(obj)
}

fn parse_vml_picture(node: &Element, context: &DrawingContext) -> Value {
    let imagedata = descendant_ns(Some(node), VML_NS, "imagedata");
    let shape = descendant_ns(Some(node), VML_NS, "shape");
    let style = shape
        .and_then(|s| {
            s.attrs
                .iter()
                .find(|a| a.local == "style")
                .map(|a| a.value.as_str())
        })
        .unwrap_or("");
    let width = regex_pt(style, "width");
    let height = regex_pt(style, "height");
    let width_px = width.map(|v| (v * 96.0 / 72.0).round() as i64).unwrap_or(0);
    let height_px = height
        .map(|v| (v * 96.0 / 72.0).round() as i64)
        .unwrap_or(0);
    let rel_id = imagedata.as_ref().and_then(|i| get_r(Some(i), "id"));
    let picture = rel_id.as_ref().and_then(|r| resolve_picture(r, context));
    if let Some(mut pic) = picture {
        if let Some(obj) = pic.as_object_mut() {
            obj.insert("widthPx".into(), json!(width_px));
            obj.insert("heightPx".into(), json!(height_px));
        }
        return pic;
    }
    json!({
        "relId": rel_id.unwrap_or_default(),
        "partName": "",
        "contentType": "application/octet-stream",
        "widthPx": width_px,
        "heightPx": height_px,
        "unsupported": "Legacy VML drawing",
    })
}

fn regex_pt(style: &str, prop: &str) -> Option<f64> {
    let _pattern = format!(r"{0}:\s*([\d.]+)pt", prop);
    let re = prop;
    let lower = style.to_lowercase();
    let key = format!("{}:", re);
    let pos = lower.find(&key)?;
    let rest = &style[pos + key.len()..];
    let num: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    num.parse().ok()
}

pub fn parse_drawing(node: &Element, context: &DrawingContext) -> Value {
    if node.local == "pict" {
        parse_vml_picture(node, context)
    } else {
        parse_modern_drawing(node, context)
    }
}
