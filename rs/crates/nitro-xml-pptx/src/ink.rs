// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/builders/PptxGraphicFrameParser.ts
// (`parseAinkTraceText`, `decodeAinkInk`); changes listed in PROVENANCE.md.

use nitro_xml_opc::xml::{Element, Node};

use crate::inheritance::{attr, children_local};
use nitro_xml_opc::xml::ns;

#[derive(Debug, Clone, PartialEq)]
pub struct InkStroke {
    pub path_d: String,
    pub color: String,
    pub width: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InkData {
    pub strokes: Vec<InkStroke>,
}

/// Decode `<aink:ink>` under a `graphicData` node (after `mc:AlternateContent` unwrap).
pub fn decode_aink_from_graphic_data(graphic_data: &Element) -> Option<InkData> {
    let ink_root = find_aink_ink_root(graphic_data)?;
    decode_aink_ink(ink_root)
}

fn find_aink_ink_root(graphic_data: &Element) -> Option<&Element> {
    if let Some(ink) = find_local_deep(graphic_data, "ink") {
        return Some(ink);
    }
    let alt = child_local_any_ns(graphic_data, "AlternateContent")?;
    for choice in children_local_any_ns(alt, "Choice") {
        let requires = attr(choice, "Requires")
            .map(str::to_lowercase)
            .unwrap_or_default();
        if requires.contains("aink") {
            if let Some(ink) = find_local_deep(choice, "ink") {
                return Some(ink);
            }
        }
    }
    None
}

fn decode_aink_ink(ink_root: &Element) -> Option<InkData> {
    let brush = children_local_any_ns(ink_root, "inkBrush")
        .into_iter()
        .next();
    let default_color = brush_color(brush, "#000000");
    let default_width = brush_width(brush, 2.0);

    let traces = children_local_any_ns(ink_root, "trace");
    let mut strokes = Vec::new();
    for trace in traces {
        let text = element_text_content(trace);
        let path_d = parse_aink_trace_text(&text)?;
        let color = attr(trace, "brushColor")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|raw| hex_color(raw, &default_color))
            .unwrap_or_else(|| default_color.clone());
        let width = attr(trace, "brushSize")
            .and_then(|raw| raw.trim().parse::<f64>().ok())
            .filter(|w| *w > 0.0)
            .unwrap_or(default_width);
        strokes.push(InkStroke {
            path_d,
            color,
            width,
        });
    }
    if strokes.is_empty() {
        return None;
    }
    Some(InkData { strokes })
}

fn brush_color(brush: Option<&Element>, fallback: &str) -> String {
    let raw = brush
        .and_then(|b| attr(b, "brushColor"))
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match raw {
        Some(r) => hex_color(r, fallback),
        None => fallback.to_owned(),
    }
}

fn brush_width(brush: Option<&Element>, fallback: f64) -> f64 {
    brush
        .and_then(|b| attr(b, "brushSize"))
        .and_then(|raw| raw.trim().parse::<f64>().ok())
        .filter(|w| *w > 0.0)
        .unwrap_or(fallback)
}

fn hex_color(raw: &str, fallback: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return fallback.to_owned();
    }
    if trimmed.starts_with('#') {
        trimmed.to_owned()
    } else {
        format!("#{}", trimmed)
    }
}

/// `aink:trace` text → SVG `d` (M/L only).
pub fn parse_aink_trace_text(raw: &str) -> Option<String> {
    let cleaned = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return None;
    }
    let mut coords = Vec::new();
    for token in cleaned.split(' ') {
        let cleaned_token = token.trim_start_matches(['M', 'm', 'L', 'l']);
        let parts: Vec<&str> = cleaned_token.split(',').collect();
        if parts.len() < 2 {
            continue;
        }
        let x = parts[0].parse::<f64>();
        let y = parts[1].parse::<f64>();
        if let (Ok(x), Ok(y)) = (x, y) {
            coords.push((x, y));
        }
    }
    if coords.is_empty() {
        return None;
    }
    let mut segments = Vec::with_capacity(coords.len());
    for (i, (x, y)) in coords.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        segments.push(format!("{cmd}{x},{y}"));
    }
    Some(segments.join(" "))
}

fn element_text_content(el: &Element) -> String {
    el.children
        .iter()
        .filter_map(|n| match n {
            Node::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn find_local_deep<'a>(el: &'a Element, local: &str) -> Option<&'a Element> {
    let mut stack = vec![el];
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

fn child_local_any_ns<'a>(parent: &'a Element, local: &str) -> Option<&'a Element> {
    parent.children.iter().find_map(|n| match n {
        Node::Element(e) if e.local == local => Some(e),
        _ => None,
    })
}

fn children_local_any_ns<'a>(parent: &'a Element, local: &str) -> Vec<&'a Element> {
    parent
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(e) if e.local == local => Some(e),
            _ => None,
        })
        .collect()
}

pub fn graphic_data_is_ink(g: &Element) -> bool {
    let uri = attr(g, "uri").map(str::to_lowercase).unwrap_or_default();
    uri.contains("/2010/ink")
        || uri.contains("drawing/2010/ink")
        || find_local_deep(g, "ink").is_some()
        || graphic_data_has_ink_choice(g)
}

fn graphic_data_has_ink_choice(g: &Element) -> bool {
    for child in &g.children {
        let Node::Element(el) = child else {
            continue;
        };
        if el.ns.as_deref() == Some(ns::MC) && el.local == "AlternateContent" {
            for choice in children_local(el, "Choice", ns::MC) {
                let requires = attr(choice, "Requires")
                    .map(str::to_lowercase)
                    .unwrap_or_default();
                if requires.contains("aink") || find_local_deep(choice, "ink").is_some() {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use nitro_xml_opc::xml::parse_xml_str;

    // Catches: aink trace point pairs never become SVG path data so ink strokes stay unpainted.
    #[test]
    fn parse_aink_trace_text_builds_moveto_lineto_path() {
        let path = parse_aink_trace_text("10,60 22,71 33,81").expect("path");
        assert_eq!(path, "M10,60 L22,71 L33,81");
    }

    // Catches: ink graphicFrame stays in other_kinds instead of a typed ink element with strokes.
    #[test]
    fn decode_ink_fixture_graphic_data() {
        let xml = r#"<a:graphicData xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:aink="http://schemas.microsoft.com/office/drawing/2010/ink" uri="http://schemas.microsoft.com/office/drawing/2010/ink">
          <aink:ink>
            <aink:inkBrush brushColor="E91E63" brushSize="3"/>
            <aink:trace>10,60 22,71</aink:trace>
            <aink:trace>40,96 48,88</aink:trace>
          </aink:ink>
        </a:graphicData>"#;
        let doc = parse_xml_str(xml).unwrap();
        let data = decode_aink_from_graphic_data(&doc.root).expect("ink");
        assert_eq!(data.strokes.len(), 2);
        assert_eq!(data.strokes[0].color, "#E91E63");
        assert_eq!(data.strokes[0].width, 3.0);
        assert_eq!(data.strokes[0].path_d, "M10,60 L22,71");
    }
}
