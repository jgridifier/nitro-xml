// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/drawingml/drawing-text.ts; changes listed in PROVENANCE.md.

use super::color::parse_drawing_color_in;
use super::dom::{attr, boolean_attribute, children, elements, first};
use super::types::{
    DrawingTextBody, DrawingTextParagraph, DrawingTextRun, DrawingTextRunProperties,
    DrawingTextSpacing, FrameInsets,
};
use crate::xml::{ns, Element};

fn parse_spacing(parent: &Element) -> Option<DrawingTextSpacing> {
    for (name, unit, divisor) in [("spcPts", "points", 100), ("spcPct", "percent", 100_000)] {
        let raw = first(parent, name, ns::A).and_then(|e| attr(e, "val"));
        if let Some(raw) = raw {
            if raw.chars().all(|c| c.is_ascii_digit()) {
                let value = raw.parse::<f64>().ok()? / divisor as f64;
                if value.is_finite() {
                    return Some(DrawingTextSpacing {
                        unit: unit.to_string(),
                        value,
                    });
                }
            }
        }
    }
    None
}

fn parse_run_properties(properties: Option<&Element>) -> DrawingTextRunProperties {
    let mut parsed = DrawingTextRunProperties::default();
    if let Some(properties) = properties {
        if let Some(sz) = attr(properties, "sz") {
            if let Ok(size) = sz.parse::<i64>() {
                parsed.size_pt = Some(size as f64 / 100.0);
            }
        }
        parsed.bold = boolean_attribute(properties, "b");
        parsed.italic = boolean_attribute(properties, "i");
        if let Some(u) = attr(properties, "u") {
            parsed.underline = Some(u != "none");
        }
        parsed.color = parse_drawing_color_in(first(properties, "solidFill", ns::A));
        parsed.typeface = first(properties, "latin", ns::A)
            .and_then(|e| attr(e, "typeface"))
            .map(|s| s.to_string());
    }
    parsed
}

fn run_properties_empty(props: &DrawingTextRunProperties) -> bool {
    props.size_pt.is_none()
        && props.bold.is_none()
        && props.italic.is_none()
        && props.underline.is_none()
        && props.color.is_none()
        && props.typeface.is_none()
}

fn parse_run(run: &Element) -> DrawingTextRun {
    let text = if run.local == "br" {
        "\n".to_string()
    } else {
        first(run, "t", ns::A)
            .map(|e| e.text_content())
            .unwrap_or_default()
    };
    let props = parse_run_properties(first(run, "rPr", ns::A));
    DrawingTextRun {
        text,
        size_pt: props.size_pt,
        bold: props.bold,
        italic: props.italic,
        underline: props.underline,
        color: props.color,
        typeface: props.typeface,
    }
}

fn parse_paragraph(paragraph: &Element) -> DrawingTextParagraph {
    let mut runs = Vec::new();
    for child in elements(paragraph) {
        if child.ns.as_deref() == Some(ns::A) && matches!(child.local.as_str(), "r" | "fld" | "br")
        {
            runs.push(parse_run(child));
        }
    }
    let properties = first(paragraph, "pPr", ns::A);
    let align = properties.and_then(|p| attr(p, "algn"));
    let def_r_pr = properties.and_then(|p| first(p, "defRPr", ns::A));
    let defaults = parse_run_properties(def_r_pr);
    let line_spacing = properties
        .and_then(|p| first(p, "lnSpc", ns::A))
        .and_then(parse_spacing);
    let space_before = properties
        .and_then(|p| first(p, "spcBef", ns::A))
        .and_then(parse_spacing);
    let space_after = properties
        .and_then(|p| first(p, "spcAft", ns::A))
        .and_then(parse_spacing);
    DrawingTextParagraph {
        align: align.map(|s| s.to_string()),
        default_properties: if !run_properties_empty(&defaults) {
            Some(defaults)
        } else {
            None
        },
        line_spacing,
        space_before,
        space_after,
        runs,
    }
}

/// Reads a text body: anchor, insets and the runs of every paragraph.
pub fn parse_drawing_text_body(body: Option<&Element>) -> Option<DrawingTextBody> {
    let body = body?;
    let properties = first(body, "bodyPr", ns::A);
    let paragraphs: Vec<DrawingTextParagraph> = children(body, "p", ns::A)
        .into_iter()
        .map(parse_paragraph)
        .collect();
    let text = paragraphs
        .iter()
        .map(|p| p.runs.iter().map(|r| r.text.as_str()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    let mut result = DrawingTextBody {
        anchor: properties
            .and_then(|p| attr(p, "anchor"))
            .map(|s| s.to_string()),
        insets_emu: None,
        paragraphs,
        text,
    };
    let mut insets = FrameInsets {
        left: None,
        top: None,
        right: None,
        bottom: None,
    };
    if let Some(properties) = properties {
        for (key, attribute) in [
            (&mut insets.left, "lIns"),
            (&mut insets.top, "tIns"),
            (&mut insets.right, "rIns"),
            (&mut insets.bottom, "bIns"),
        ] {
            if let Some(raw) = attr(properties, attribute) {
                if let Ok(value) = raw.parse::<i64>() {
                    *key = Some(value);
                }
            }
        }
    }
    if insets.left.is_some()
        || insets.top.is_some()
        || insets.right.is_some()
        || insets.bottom.is_some()
    {
        result.insets_emu = Some(insets);
    }
    Some(result)
}
