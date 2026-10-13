// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/geometry/connector-geometry.ts,
// connector-preset-geometry.ts; changes listed in PROVENANCE.md.

use nitro_xml_opc::drawingml::evaluate_preset_shape;

/// Whether the connector preset needs an SVG `<path>` rather than a `<line>`.
pub fn connector_needs_path(shape_type: Option<&str>) -> bool {
    connector_kind(shape_type) != ConnectorKind::Straight
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConnectorKind {
    Straight,
    Bent,
    Curved,
}

fn connector_kind(shape_type: Option<&str>) -> ConnectorKind {
    let t = shape_type.unwrap_or("").to_ascii_lowercase();
    if t.contains("bentconnector") {
        ConnectorKind::Bent
    } else if t.contains("curvedconnector") {
        ConnectorKind::Curved
    } else {
        ConnectorKind::Straight
    }
}

fn is_bent_or_curved_preset(shape_type: &str) -> bool {
    let t = shape_type.to_ascii_lowercase();
    if !t.starts_with("bentconnector") && !t.starts_with("curvedconnector") {
        return false;
    }
    let n = t.chars().last().and_then(|c| c.to_digit(10));
    matches!(n, Some(2) | Some(3) | Some(4) | Some(5))
}

fn format_coord(value: f64) -> String {
    format!("{}", (value * 1e12).round() / 1e12)
}

/// Flip preset path coordinates for `flipH` / `flipV` (mirrors upstream `flipPresetPath`).
fn flip_preset_path(
    path_data: &str,
    width: f64,
    height: f64,
    flip_h: bool,
    flip_v: bool,
) -> Option<String> {
    if !flip_h && !flip_v {
        return Some(path_data.to_owned());
    }
    let tokens = tokenize_path(path_data);
    if tokens.is_empty() {
        return None;
    }
    let mut output = Vec::new();
    let mut command = String::new();
    let mut coordinate_index = 0usize;
    for token in tokens {
        if token.len() == 1 && token.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
            command = token.clone();
            coordinate_index = 0;
            let arity = command_arity(&command)?;
            output.push(token);
            if arity == 0 {
                continue;
            }
            continue;
        }
        let arity = command_arity(&command)?;
        if coordinate_index >= arity {
            return None;
        }
        let value = token.parse::<f64>().ok()?;
        let is_x = coordinate_index.is_multiple_of(2);
        let flipped = if is_x {
            if flip_h {
                width - value
            } else {
                value
            }
        } else if flip_v {
            height - value
        } else {
            value
        };
        output.push(format_coord(flipped));
        coordinate_index += 1;
    }
    Some(output.join(" "))
}

fn command_arity(command: &str) -> Option<usize> {
    match command {
        "M" | "L" => Some(2),
        "Q" => Some(4),
        "C" => Some(6),
        "Z" => Some(0),
        _ => None,
    }
}

fn tokenize_path(path_data: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    for ch in path_data.chars() {
        if ch.is_ascii_uppercase() {
            if !cur.is_empty() {
                tokens.push(cur.clone());
                cur.clear();
            }
            tokens.push(ch.to_string());
        } else if ch.is_ascii_whitespace() || ch == ',' {
            if !cur.is_empty() {
                tokens.push(cur.clone());
                cur.clear();
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

pub struct ConnectorStrokeGeometry {
    /// SVG fragment: either `<line .../>` or `<path d="..." .../>`.
    pub markup: String,
}

/// Build connector stroke geometry from element box and xfrm flips (upstream `getConnectorPathGeometry`).
pub fn connector_stroke_geometry(
    shape_type: Option<&str>,
    width: f64,
    height: f64,
    flip_h: bool,
    flip_v: bool,
    stroke: &str,
    stroke_width: f64,
) -> ConnectorStrokeGeometry {
    let w = if width.is_finite() && width > 0.0 {
        width
    } else {
        0.0
    };
    let h = if height.is_finite() && height > 0.0 {
        height
    } else {
        0.0
    };
    let start_x = if flip_h { w } else { 0.0 };
    let start_y = if flip_v { h } else { 0.0 };
    let end_x = if flip_h { 0.0 } else { w };
    let end_y = if flip_v { 0.0 } else { h };

    let stroke_attrs = format!(
        "fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"",
        xml_escape_attr(stroke),
        stroke_width
    );

    if let Some(st) = shape_type {
        if is_bent_or_curved_preset(st) {
            if let Some(eval) = evaluate_preset_shape(st, w, h, None) {
                let path_d = if flip_h || flip_v {
                    flip_preset_path(&eval.svg_path, w, h, flip_h, flip_v).unwrap_or(eval.svg_path)
                } else {
                    eval.svg_path
                };
                let markup = format!(
                    "<path d=\"{}\" {}/>",
                    xml_escape_attr(&path_d),
                    stroke_attrs
                );
                return ConnectorStrokeGeometry { markup };
            }
        }
    }

    if connector_needs_path(shape_type) {
        let path_d = format!(
            "M {} {} L {} {}",
            format_coord(start_x),
            format_coord(start_y),
            format_coord(end_x),
            format_coord(end_y)
        );
        let markup = format!(
            "<path d=\"{}\" {}/>",
            xml_escape_attr(&path_d),
            stroke_attrs
        );
        return ConnectorStrokeGeometry { markup };
    }

    let markup = format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" {}/>",
        format_coord(start_x),
        format_coord(start_y),
        format_coord(end_x),
        format_coord(end_y),
        stroke_attrs
    );
    ConnectorStrokeGeometry { markup }
}

fn xml_escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}
