// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/drawingml/drawing-fill.ts; changes listed in PROVENANCE.md.

use super::color::parse_drawing_color_in;
use super::dom::{children, elements, first};
use super::serialize::serialize_element;
use super::types::{DrawingFill, DrawingLine, GradientStop, RectFractions};
use crate::xml::{ns, Element};

const FILL_LOCALS: &[&str] = &[
    "noFill",
    "solidFill",
    "gradFill",
    "pattFill",
    "blipFill",
    "grpFill",
];

pub fn fill_element_of(properties: Option<&Element>) -> Option<&Element> {
    let properties = properties?;
    elements(properties)
        .into_iter()
        .find(|element| FILL_LOCALS.contains(&element.local.as_str()))
}

pub fn parse_drawing_fill(properties: Option<&Element>) -> Option<DrawingFill> {
    parse_fill_element(fill_element_of(properties)?)
}

/// Parse one fill element (`a:solidFill`, `a:gradFill`, `a:blipFill`, …).
pub fn parse_fill_element(fill: &Element) -> Option<DrawingFill> {
    match fill.local.as_str() {
        "noFill" => Some(DrawingFill::None),
        "solidFill" => {
            let color = parse_drawing_color_in(Some(fill));
            if let Some(color) = color {
                Some(DrawingFill::Solid { color })
            } else {
                Some(DrawingFill::Unsupported {
                    element: "solidFill".into(),
                })
            }
        }
        "gradFill" => {
            let stops = children(first(fill, "gsLst", ns::A).unwrap_or(fill), "gs", ns::A)
                .into_iter()
                .filter_map(|stop| {
                    let color = parse_drawing_color_in(Some(stop))?;
                    let pos = super::dom::attr(stop, "pos").and_then(|v| v.parse::<i64>().ok())?;
                    Some(GradientStop {
                        position: pos as f64 / 1000.0,
                        color,
                    })
                })
                .collect();
            let lin = first(fill, "lin", ns::A);
            let angle = lin
                .and_then(|e| super::dom::attr(e, "ang"))
                .and_then(|v| v.parse::<i64>().ok())
                .map(|a| a as f64 / 60_000.0);
            let scaled = lin
                .and_then(|e| super::dom::attr(e, "scaled"))
                .map(|s| s == "1" || s.eq_ignore_ascii_case("true"));
            let path_el = first(fill, "path", ns::A);
            let path = path_el.and_then(|e| super::dom::attr(e, "path").map(|s| s.to_string()));
            let focus = path_el.and_then(|p| first(p, "fillToRect", ns::A));
            let fill_to_rect = focus.map(parse_rect_fractions);
            let tile = first(fill, "tileRect", ns::A);
            let tile_rect = tile.map(parse_rect_fractions);
            Some(DrawingFill::Gradient {
                source_xml: Some(serialize_element(fill)),
                stops,
                angle,
                scaled,
                path,
                fill_to_rect: fill_to_rect.filter(|r| {
                    r.l.is_finite() && r.t.is_finite() && r.r.is_finite() && r.b.is_finite()
                }),
                tile_rect: tile_rect.filter(|r| {
                    r.l.is_finite() && r.t.is_finite() && r.r.is_finite() && r.b.is_finite()
                }),
            })
        }
        "pattFill" => {
            let foreground = parse_drawing_color_in(first(fill, "fgClr", ns::A));
            let background = parse_drawing_color_in(first(fill, "bgClr", ns::A));
            Some(DrawingFill::Pattern {
                preset: super::dom::attr(fill, "prst").unwrap_or("").to_string(),
                foreground,
                background,
            })
        }
        "blipFill" => {
            let blip = first(fill, "blip", ns::A);
            let rel_id = blip
                .and_then(|b| super::dom::attr_ns(b, ns::R, "embed"))
                .or_else(|| blip.and_then(|b| super::dom::attr(b, "embed")))
                .map(|s| s.to_string());
            let opacity = blip
                .and_then(|b| first(b, "alphaModFix", ns::A))
                .and_then(|a| {
                    super::dom::attr(a, "amt")
                        .and_then(|v| v.parse::<f64>().ok())
                        .map(|amt| (amt / 100_000.0).clamp(0.0, 1.0))
                });
            Some(DrawingFill::Picture { rel_id, opacity })
        }
        _ => Some(DrawingFill::Unsupported {
            element: fill.local.clone(),
        }),
    }
}

fn parse_rect_fractions(element: &Element) -> RectFractions {
    let frac = |name: &str| {
        super::dom::attr(element, name)
            .and_then(|v| v.parse::<f64>().ok())
            .map(|n| n / 100_000.0)
            .unwrap_or(0.0)
    };
    RectFractions {
        l: frac("l"),
        t: frac("t"),
        r: frac("r"),
        b: frac("b"),
    }
}

pub fn parse_drawing_line(ln: Option<&Element>) -> Option<DrawingLine> {
    let ln = ln?;
    let mut line = DrawingLine {
        width_emu: None,
        fill: None,
        dash: None,
        cap: None,
    };
    if let Some(w) = super::dom::attr(ln, "w").and_then(|v| v.parse::<i64>().ok()) {
        line.width_emu = Some(w);
    }
    line.fill = parse_drawing_fill(Some(ln));
    line.dash = first(ln, "prstDash", ns::A)
        .and_then(|e| super::dom::attr(e, "val").map(|s| s.to_string()));
    line.cap = super::dom::attr(ln, "cap").map(|s| s.to_string());
    Some(line)
}
