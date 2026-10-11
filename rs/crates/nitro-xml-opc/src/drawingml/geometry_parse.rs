// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/drawingml/drawing-geometry.ts; changes listed in PROVENANCE.md.

use std::collections::HashMap;

use serde::Serialize;

use super::dom::{boolean_attribute, children, first, integer_attribute};
use super::types::{DrawingFrame, DrawingPath, DrawingPathCommand};
use crate::xml::{ns, Element};

pub fn parse_frame(container: Option<&Element>) -> Option<DrawingFrame> {
    let container = container?;
    let off = first(container, "off", ns::A)?;
    let ext = first(container, "ext", ns::A)?;
    Some(DrawingFrame {
        x: integer_attribute(off, "x").unwrap_or(0),
        y: integer_attribute(off, "y").unwrap_or(0),
        width: integer_attribute(ext, "cx").unwrap_or(0),
        height: integer_attribute(ext, "cy").unwrap_or(0),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ParsedTransform {
    pub frame: DrawingFrame,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flip_horizontal: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flip_vertical: Option<bool>,
}

pub fn parse_transform(xfrm: Option<&Element>) -> Option<ParsedTransform> {
    let xfrm = xfrm?;
    let frame = parse_frame(Some(xfrm))?;
    let rot = integer_attribute(xfrm, "rot");
    Some(ParsedTransform {
        frame,
        rotation: rot.filter(|r| *r != 0).map(|r| r as f64 / 60_000.0),
        flip_horizontal: boolean_attribute(xfrm, "flipH").filter(|v| *v),
        flip_vertical: boolean_attribute(xfrm, "flipV").filter(|v| *v),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ParsedGeometry {
    pub geometry: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjustments: Option<HashMap<String, i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<DrawingPath>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
}

pub fn parse_drawing_geometry(sp_pr: Option<&Element>) -> ParsedGeometry {
    let Some(sp_pr) = sp_pr else {
        return ParsedGeometry {
            geometry: "rect".into(),
            adjustments: None,
            paths: None,
            issue: None,
        };
    };
    let preset = first(sp_pr, "prstGeom", ns::A);
    let custom = first(sp_pr, "custGeom", ns::A);
    if let Some(custom) = custom {
        let mut paths = Vec::new();
        let path_parent = first(custom, "pathLst", ns::A).unwrap_or(custom);
        for path in children(path_parent, "path", ns::A) {
            match parse_path_commands(path) {
                Some(commands) => {
                    let width = integer_attribute(path, "w").map(|v| v as f64);
                    let height = integer_attribute(path, "h").map(|v| v as f64);
                    let fill = super::dom::attr(path, "fill").map(|s| s.to_string());
                    let stroke = match super::dom::attr(path, "stroke") {
                        Some(s) if s == "0" || s.eq_ignore_ascii_case("false") => Some(false),
                        _ => None,
                    };
                    paths.push(DrawingPath {
                        width,
                        height,
                        fill,
                        stroke,
                        commands,
                    });
                }
                None => {
                    return ParsedGeometry {
                        geometry: "custom".into(),
                        adjustments: None,
                        paths: None,
                        issue: Some("CUSTOM_GEOMETRY_GUIDES_UNSUPPORTED".into()),
                    };
                }
            }
        }
        return ParsedGeometry {
            geometry: "custom".into(),
            adjustments: None,
            paths: Some(paths),
            issue: None,
        };
    }
    let geometry = preset
        .and_then(|p| super::dom::attr(p, "prst"))
        .unwrap_or("rect")
        .to_string();
    let mut adjustments = HashMap::new();
    if let Some(av_lst) = preset.and_then(|p| first(p, "avLst", ns::A)) {
        for guide in children(av_lst, "gd", ns::A) {
            let name = super::dom::attr(guide, "name");
            let fmla = super::dom::attr(guide, "fmla").unwrap_or("");
            if let (Some(name), Some(m)) = (name, fmla.strip_prefix("val ")) {
                if let Ok(val) = m.trim().parse::<i64>() {
                    adjustments.insert(name.to_string(), val);
                }
            }
        }
    }
    ParsedGeometry {
        geometry,
        adjustments: if adjustments.is_empty() {
            None
        } else {
            Some(adjustments)
        },
        paths: None,
        issue: None,
    }
}

fn parse_path_commands(path: &Element) -> Option<Vec<DrawingPathCommand>> {
    let mut commands = Vec::new();
    for child in &path.children {
        let node = match child {
            crate::xml::Node::Element(e) => e,
            _ => continue,
        };
        let points: Vec<Option<(f64, f64)>> = children(node, "pt", ns::A)
            .into_iter()
            .map(|pt| {
                let x = integer_attribute(pt, "x")? as f64;
                let y = integer_attribute(pt, "y")? as f64;
                Some((x, y))
            })
            .collect();
        if points.iter().any(|p| p.is_none()) {
            return None;
        }
        let pts: Vec<(f64, f64)> = points.into_iter().flatten().collect();
        match node.local.as_str() {
            "moveTo" | "lnTo" => {
                let (x, y) = pts.first().copied()?;
                commands.push(if node.local == "moveTo" {
                    DrawingPathCommand::M { x, y }
                } else {
                    DrawingPathCommand::L { x, y }
                });
            }
            "cubicBezTo" => {
                if pts.len() < 3 {
                    return None;
                }
                commands.push(DrawingPathCommand::C {
                    x1: pts[0].0,
                    y1: pts[0].1,
                    x2: pts[1].0,
                    y2: pts[1].1,
                    x: pts[2].0,
                    y: pts[2].1,
                });
            }
            "quadBezTo" => {
                if pts.len() < 2 {
                    return None;
                }
                commands.push(DrawingPathCommand::Q {
                    x1: pts[0].0,
                    y1: pts[0].1,
                    x: pts[1].0,
                    y: pts[1].1,
                });
            }
            "arcTo" => {
                let w_r = integer_attribute(node, "wR")? as f64;
                let h_r = integer_attribute(node, "hR")? as f64;
                let start = integer_attribute(node, "stAng")? as f64;
                let swing = integer_attribute(node, "swAng")? as f64;
                commands.push(DrawingPathCommand::A {
                    w_r,
                    h_r,
                    start_angle: start / 60_000.0,
                    swing_angle: swing / 60_000.0,
                });
            }
            "close" => commands.push(DrawingPathCommand::Z),
            _ => {}
        }
    }
    Some(commands)
}
