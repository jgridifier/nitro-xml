// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/drawingml/drawing-shadow.ts

use nitro_xml_opc::drawingml::EMU_PER_PIXEL;
use nitro_xml_opc::drawingml::{
    parse_drawing_color_in, resolve_drawing_color, HslRounding, ResolveOptions, TransformOrder,
};
use nitro_xml_opc::xml::{ns, parse_xml_str, Element};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct DrawingSvgShadow {
    pub color: String,
    pub opacity: f64,
    pub blur: f64,
    pub dx: f64,
    pub dy: f64,
}

fn first_child<'a>(element: &'a Element, local: &str) -> Option<&'a Element> {
    element
        .children
        .iter()
        .filter_map(|n| n.as_element())
        .find(|e| e.local == local && e.ns.as_deref() == Some(ns::A))
}

fn attr_f64(element: &Element, key: &str, fallback: f64) -> f64 {
    element
        .attrs
        .iter()
        .find(|a| a.local == key)
        .and_then(|a| a.value.parse().ok())
        .unwrap_or(fallback)
}

pub fn resolve_drawing_shadow_xml(
    xml: &str,
    scheme: &HashMap<String, String>,
) -> Option<DrawingSvgShadow> {
    let doc = parse_xml_str(xml).ok()?;
    let root = &doc.root;
    let node = first_child(root, "outerShdw")?;
    if attr_f64(node, "sx", 100000.0) != 100000.0
        || attr_f64(node, "sy", 100000.0) != 100000.0
        || attr_f64(node, "kx", 0.0) != 0.0
        || attr_f64(node, "ky", 0.0) != 0.0
    {
        return None;
    }
    let color = parse_drawing_color_in(Some(node));
    let resolved = color.as_ref().and_then(|c| {
        resolve_drawing_color(
            c,
            Some(scheme),
            Some(ResolveOptions {
                transform_order: TransformOrder::Document,
                hsl_rounding: HslRounding::Nearest,
            }),
        )
    });
    let blur = attr_f64(node, "blurRad", 0.0) / EMU_PER_PIXEL as f64;
    let distance = attr_f64(node, "dist", 0.0) / EMU_PER_PIXEL as f64;
    let angle = (attr_f64(node, "dir", 0.0) / 60000.0) * std::f64::consts::PI / 180.0;
    let resolved = resolved?;
    if ![blur, distance, angle].iter().all(|v| v.is_finite()) || blur < 0.0 || distance < 0.0 {
        return None;
    }
    Some(DrawingSvgShadow {
        color: resolved.hex.clone(),
        opacity: resolved.alpha,
        blur,
        dx: angle.cos() * distance,
        dy: angle.sin() * distance,
    })
}

pub fn svg_drop_shadow_element(s: &DrawingSvgShadow) -> String {
    format!(
        "<feDropShadow dx=\"{}\" dy=\"{}\" stdDeviation=\"{}\" flood-color=\"{}\" flood-opacity=\"{}\" result=\"shadow\"/>",
        super::svg_util::n(s.dx),
        super::svg_util::n(s.dy),
        super::svg_util::n(s.blur / 2.0),
        super::svg_util::esc(&s.color),
        super::svg_util::n(s.opacity),
    )
}
