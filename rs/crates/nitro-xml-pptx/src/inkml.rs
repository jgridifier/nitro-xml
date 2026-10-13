// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/utils/inkml-content-part.ts,
// inkml-trace-decode.ts, and inkml-ink-space.ts; changes listed in PROVENANCE.md.

use nitro_xml_opc::xml::{Element, Node};

use crate::inheritance::attr;
use crate::ink::{InkData, InkStroke};

const PX_PER_INCH: f64 = 96.0;

fn px_per_unit(units: &str) -> Option<f64> {
    match units.trim().to_lowercase().as_str() {
        "in" => Some(PX_PER_INCH),
        "cm" => Some(PX_PER_INCH / 2.54),
        "mm" => Some(PX_PER_INCH / 25.4),
        "pt" => Some(PX_PER_INCH / 72.0),
        "pc" => Some(PX_PER_INCH / 6.0),
        "himetric" => Some(PX_PER_INCH / 2540.0),
        "px" | "dev" | "" => Some(1.0),
        _ => None,
    }
}

fn ink_length_to_px(value: f64, units: Option<&str>) -> f64 {
    if !value.is_finite() {
        return f64::NAN;
    }
    let units = units.map(str::trim).filter(|u| !u.is_empty());
    match units {
        Some(u) => px_per_unit(u).map(|scale| value * scale).unwrap_or(value),
        None => value,
    }
}

#[derive(Clone, Copy)]
struct InkBounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

fn ink_bounds(traces: &[Vec<Vec<f64>>]) -> Option<InkBounds> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for points in traces {
        for point in points {
            if point.len() < 2 {
                continue;
            }
            let x = point[0];
            let y = point[1];
            if x.is_finite() && y.is_finite() {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    if min_x.is_finite() && min_y.is_finite() {
        Some(InkBounds {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    } else {
        None
    }
}

fn ink_point_mapper(bounds: InkBounds, width: f64, height: f64) -> impl Fn(f64, f64) -> (f64, f64) {
    let span_x = bounds.max_x - bounds.min_x;
    let span_y = bounds.max_y - bounds.min_y;
    let w = width.max(1.0);
    let h = height.max(1.0);
    move |x, y| {
        let nx = if span_x > 0.0 {
            ((x - bounds.min_x) / span_x) * w
        } else {
            w / 2.0
        };
        let ny = if span_y > 0.0 {
            ((y - bounds.min_y) / span_y) * h
        } else {
            h / 2.0
        };
        (nx, ny)
    }
}

fn find_first_local<'a>(root: &'a Element, local: &str) -> Option<&'a Element> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.local == local {
            return Some(node);
        }
        for child in node.children.iter().rev() {
            if let Node::Element(c) = child {
                stack.push(c);
            }
        }
    }
    None
}

fn collect_local<'a>(root: &'a Element, local: &str) -> Vec<&'a Element> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.local == local {
            out.push(node);
        }
        for child in node.children.iter().rev() {
            if let Node::Element(c) = child {
                stack.push(c);
            }
        }
    }
    out
}

fn element_id_attr(el: &Element) -> String {
    el.attrs
        .iter()
        .find(|a| a.local == "id")
        .map(|a| a.value.trim().trim_start_matches('#').to_owned())
        .unwrap_or_default()
}

fn element_text(el: &Element) -> String {
    el.children
        .iter()
        .filter_map(|n| match n {
            Node::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn resolve_channel_order(root: &Element) -> Vec<String> {
    let trace_format = find_first_local(root, "traceFormat");
    let Some(tf) = trace_format else {
        return vec!["X".to_owned(), "Y".to_owned()];
    };
    let channels = collect_local(tf, "channel");
    let names: Vec<String> = channels
        .iter()
        .filter_map(|ch| attr(ch, "name"))
        .map(|n| n.trim().to_uppercase())
        .filter(|n| !n.is_empty())
        .collect();
    if names.is_empty() {
        vec!["X".to_owned(), "Y".to_owned()]
    } else {
        names
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiffMode {
    Explicit,
    Single,
    Double,
}

fn scan_value_tokens(raw_point: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let bytes = raw_point.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let prefix = match bytes[i] {
            b'!' | b'\'' | b'"' => {
                let p = bytes[i];
                i += 1;
                Some(p)
            }
            _ => None,
        };
        let start = i;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        while i < bytes.len() {
            let c = bytes[i];
            if c.is_ascii_digit() || c == b'.' {
                i += 1;
                continue;
            }
            if (c == b'e' || c == b'E')
                && i + 1 < bytes.len()
                && (bytes[i + 1] == b'+' || bytes[i + 1] == b'-')
            {
                i += 2;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                break;
            }
            break;
        }
        if i == start {
            i += 1;
            continue;
        }
        let body = raw_point.get(start..i).unwrap_or("");
        let token = match prefix {
            Some(p) => format!("{}{}", p as char, body),
            None => body.to_owned(),
        };
        if !token.is_empty() {
            tokens.push(token);
        }
    }
    tokens
}

fn parse_value_token(token: &str, current: DiffMode) -> Option<(f64, DiffMode)> {
    let mut mode = current;
    let mut body = token;
    if let Some(prefix) = token.chars().next() {
        if prefix == '!' {
            mode = DiffMode::Explicit;
            body = token.get(1..).unwrap_or("");
        } else if prefix == '\'' {
            mode = DiffMode::Single;
            body = token.get(1..).unwrap_or("");
        } else if prefix == '"' {
            mode = DiffMode::Double;
            body = token.get(1..).unwrap_or("");
        }
    }
    if body.is_empty() {
        return None;
    }
    let value = body.parse::<f64>().ok()?;
    if value.is_finite() {
        Some((value, mode))
    } else {
        None
    }
}

fn apply_diff_mode(
    value: f64,
    mode: DiffMode,
    idx: usize,
    last_value: &mut [f64],
    last_velocity: &mut [f64],
) -> f64 {
    match mode {
        DiffMode::Single => {
            last_velocity[idx] = value;
            last_value[idx] += value;
        }
        DiffMode::Double => {
            last_velocity[idx] += value;
            last_value[idx] += last_velocity[idx];
        }
        DiffMode::Explicit => {
            last_value[idx] = value;
            last_velocity[idx] = 0.0;
        }
    }
    last_value[idx]
}

pub fn decode_trace_points(text: &str, channel_order: &[String]) -> Vec<Vec<f64>> {
    let mut points = Vec::new();
    let mut modes: Vec<DiffMode> = channel_order.iter().map(|_| DiffMode::Explicit).collect();
    let mut last_value: Vec<f64> = channel_order.iter().map(|_| 0.0).collect();
    let mut last_velocity: Vec<f64> = channel_order.iter().map(|_| 0.0).collect();
    for raw_point in text.split(',') {
        let tokens = scan_value_tokens(raw_point);
        if tokens.is_empty() {
            continue;
        }
        let mut decoded = Vec::new();
        for (i, token) in tokens.iter().enumerate() {
            if i >= channel_order.len() {
                break;
            }
            let Some((value, mode)) = parse_value_token(token, modes[i]) else {
                decoded.push(last_value[i]);
                continue;
            };
            modes[i] = mode;
            decoded.push(apply_diff_mode(
                value,
                mode,
                i,
                &mut last_value,
                &mut last_velocity,
            ));
        }
        if decoded.len() >= 2 {
            points.push(decoded);
        }
    }
    points
}

fn points_to_svg_path(points: &[Vec<f64>], channel_order: &[String]) -> String {
    let xi = channel_order.iter().position(|c| c == "X").unwrap_or(0);
    let yi = channel_order.iter().position(|c| c == "Y").unwrap_or(1);
    let mut segments = Vec::new();
    for point in points {
        if point.len() <= yi.max(xi) {
            continue;
        }
        let x = point[xi];
        let y = point[yi];
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        let cmd = if segments.is_empty() { 'M' } else { 'L' };
        segments.push(format!("{cmd} {x} {y}"));
    }
    segments.join(" ")
}

struct BrushStyle {
    color: String,
    width: f64,
    opacity: f64,
}

fn brush_opacity(properties: &[(String, String, Option<String>)]) -> f64 {
    for (name, value, _units) in properties {
        if name == "opacity" {
            if let Ok(v) = value.parse::<f64>() {
                if v.is_finite() {
                    return v.clamp(0.0, 1.0);
                }
            }
        }
        if name == "transparency" {
            if let Ok(v) = value.parse::<f64>() {
                if v.is_finite() {
                    return 1.0 - (v.clamp(0.0, 255.0) / 255.0);
                }
            }
        }
    }
    1.0
}

fn collect_brushes(root: &Element) -> std::collections::BTreeMap<String, BrushStyle> {
    let mut brushes = std::collections::BTreeMap::new();
    for brush in collect_local(root, "brush") {
        let mut properties = Vec::new();
        for prop in collect_local(brush, "brushProperty") {
            let name = attr(prop, "name").unwrap_or("").to_owned();
            let value = attr(prop, "value").unwrap_or("").to_owned();
            let units = attr(prop, "units");
            properties.push((name, value, units.map(str::to_owned)));
        }
        let size = properties
            .iter()
            .find(|(n, _, _)| *n == "width" || *n == "height");
        let width = size
            .and_then(|(_, v, u)| {
                let n = v.parse::<f64>().ok()?;
                let px = ink_length_to_px(n, u.as_deref());
                if px.is_finite() && px > 0.0 {
                    Some(px)
                } else {
                    None
                }
            })
            .unwrap_or(1.0);
        let color = properties
            .iter()
            .find(|(n, _, _)| *n == "color")
            .map(|(_, v, _)| v.clone())
            .unwrap_or_else(|| "#000000".to_owned());
        let id = element_id_attr(brush);
        brushes.insert(
            id,
            BrushStyle {
                color,
                width,
                opacity: brush_opacity(&properties),
            },
        );
    }
    brushes
}

/// Parse an InkML package part into paint strokes normalised to `box_w` × `box_h` CSS px.
pub fn parse_ink_ml_content(root: &Element, box_w: f64, box_h: f64) -> Option<InkData> {
    let ink_root = if root.local == "ink" {
        root
    } else {
        find_first_local(root, "ink")?
    };
    let channel_order = resolve_channel_order(ink_root);
    let brushes = collect_brushes(ink_root);
    let traces = collect_local(ink_root, "trace");
    let mut decoded_points = Vec::new();
    let mut brush_refs = Vec::new();
    for trace in traces {
        let text = element_text(trace).trim().to_owned();
        let brush_ref = attr(trace, "brushRef")
            .map(|s| s.trim().trim_start_matches('#').to_owned())
            .unwrap_or_default();
        let points = decode_trace_points(&text, &channel_order);
        decoded_points.push(points);
        brush_refs.push(brush_ref);
    }
    if decoded_points.is_empty() {
        return None;
    }
    let bounds = ink_bounds(&decoded_points);
    let mapper = bounds.map(|b| ink_point_mapper(b, box_w, box_h));
    let default_brush = BrushStyle {
        color: "#000000".to_owned(),
        width: 1.0,
        opacity: 1.0,
    };
    let mut strokes = Vec::new();
    for (points, brush_ref) in decoded_points.iter().zip(brush_refs.iter()) {
        let path = if let Some(map) = &mapper {
            let mapped: Vec<Vec<f64>> = points
                .iter()
                .map(|p| {
                    let x = p.get(0).copied().unwrap_or(0.0);
                    let y = p.get(1).copied().unwrap_or(0.0);
                    let (mx, my) = map(x, y);
                    vec![mx, my]
                })
                .collect();
            points_to_svg_path(&mapped, &["X".to_owned(), "Y".to_owned()])
        } else {
            points_to_svg_path(points, &channel_order)
        };
        if path.is_empty() {
            continue;
        }
        let brush = brushes.get(brush_ref).unwrap_or(&default_brush);
        strokes.push(InkStroke {
            path_d: path,
            color: brush.color.clone(),
            width: brush.width,
            opacity: brush.opacity,
        });
    }
    if strokes.is_empty() {
        return None;
    }
    Some(InkData { strokes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nitro_xml_opc::xml::parse_xml_str;

    // Catches: PowerPoint compact difference-encoded InkML traces decode to one point.
    #[test]
    fn fixture_trace_bounds_span_full_width() {
        let order = vec!["X".to_owned(), "Y".to_owned()];
        let traces = [
            "100 200,'40'46,\"0\"-5,0-10,0-13,0-15,0-15,0-14,0-11,0-7,0-3,0 4,0 7,0 12,0 14,0 16,0 14,0 13,0 9,0 4,0-1,0-5,0-11,0-12,0-16,0-16,0-13,0-10,0-7,0-2,0 5",
            "1360 200,'60'120,\"80\"-380",
            "1650 80,'130'240",
            "1780 80,'-130'240",
        ];
        let decoded: Vec<Vec<Vec<f64>>> = traces
            .iter()
            .map(|t| decode_trace_points(t, &order))
            .collect();
        let b = ink_bounds(&decoded).unwrap();
        assert!((b.max_x - 1780.0).abs() < 1.0, "max_x={}", b.max_x);
    }

    #[test]
    fn decode_checkmark_trace_has_three_points() {
        let order = vec!["X".to_owned(), "Y".to_owned()];
        let points = decode_trace_points("1360 200,'60'120,\"80\"-380", &order);
        assert_eq!(points.len(), 3, "points={:?}", points);
    }

    #[test]
    fn decode_sine_trace_has_thirty_line_segments() {
        let text = "100 200,'40'46,\"0\"-5,0-10,0-13,0-15,0-15,0-14,0-11,0-7,0-3,0 4,0 7,0 12,0 14,0 16,0 14,0 13,0 9,0 4,0-1,0-5,0-11,0-12,0-16,0-16,0-13,0-10,0-7,0-2,0 5";
        let order = vec!["X".to_owned(), "Y".to_owned()];
        let points = decode_trace_points(text, &order);
        assert_eq!(points.len(), 31);
        let path = points_to_svg_path(&points, &order);
        assert_eq!(path.matches('L').count(), 30);
    }

    // Catches: brushes nested under definitions with cm widths collapse to ~1.89px / ~3.78px.
    #[test]
    fn parse_fixture_inkml_brush_widths_and_colors() {
        let bytes =
            std::fs::read("/workspace/fixtures/upstream/pptx/73913b502a2e_ink-contentpart.pptx")
                .unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut file = zip.by_name("ppt/ink/ink1.xml").unwrap();
        let mut ink_xml = String::new();
        std::io::Read::read_to_string(&mut file, &mut ink_xml).unwrap();
        let doc = parse_xml_str(&ink_xml).unwrap();
        let w = 3240000f64 / 9525f64;
        let h = 1224000f64 / 9525f64;
        let data = parse_ink_ml_content(&doc.root, w, h).unwrap();
        assert_eq!(data.strokes.len(), 4);
        assert!(
            data.strokes[0].path_d.contains('L'),
            "sine path too short: {}",
            data.strokes[0].path_d
        );
        assert_eq!(
            data.strokes
                .iter()
                .map(|s| s.color.as_str())
                .collect::<Vec<_>>(),
            ["#E81123", "#0078D7", "#0078D7", "#0078D7"]
        );
        assert!((data.strokes[0].width - (0.05 * 96.0 / 2.54)).abs() < 0.001);
        assert!((data.strokes[1].width - (0.1 * 96.0 / 2.54)).abs() < 0.001);
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for stroke in &data.strokes {
            let mut parts = stroke.path_d.split_whitespace();
            while let Some(token) = parts.next() {
                if token == "M" || token == "L" {
                    let x = parts.next().and_then(|t| t.parse::<f64>().ok());
                    let y = parts.next().and_then(|t| t.parse::<f64>().ok());
                    if let (Some(x), Some(y)) = (x, y) {
                        xs.push(x);
                        ys.push(y);
                    }
                }
            }
        }
        assert!((xs.iter().copied().fold(f64::INFINITY, f64::min) - 0.0).abs() < 0.01);
        assert!((ys.iter().copied().fold(f64::INFINITY, f64::min) - 0.0).abs() < 0.01);
        let max_x = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let max_y = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(
            (max_x - w).abs() < 0.01,
            "max_x={max_x} expected w={w}"
        );
        assert!(
            (max_y - h).abs() < 0.01,
            "max_y={max_y} expected h={h}"
        );
    }
}
