// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec
// src/core/pptx/core/utils/smartart-effect-intensity.ts; changes listed in PROVENANCE.md.

use nitro_xml_opc::xml::{parse_xml_str, Element, Node};

/// Resolved effect-intensity levels from a quick-style part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartArtEffectIntensity {
    Intense,
    Moderate,
    Subtle,
}

/// Derive effect intensity from `ppt/diagrams/quickStyle*.xml` content.
pub fn resolve_smart_art_effect_intensity_from_qs_xml(
    qs_xml: &str,
) -> Option<SmartArtEffectIntensity> {
    let doc = parse_xml_str(qs_xml).ok()?;
    let style_lbls = collect_style_lbls(&doc.root);
    resolve_smart_art_effect_intensity(&style_lbls)
}

fn collect_style_lbls(root: &Element) -> Vec<StyleLbl> {
    let mut out = Vec::new();
    let mut stack: Vec<&Element> = vec![root];
    while let Some(node) = stack.pop() {
        if node.local == "styleLbl" {
            out.push(parse_style_lbl(node));
        }
        for child in node.children.iter().rev() {
            if let Node::Element(child) = child {
                stack.push(child);
            }
        }
    }
    out
}

struct StyleLbl {
    name: String,
    has_bevel_geometry: bool,
    effect_idx: u32,
    fill_idx: u32,
}

fn parse_style_lbl(el: &Element) -> StyleLbl {
    let name = attr_str(el, "name").unwrap_or_default();
    let sp3d = child_any(el, "sp3d");
    let has_bevel_geometry = sp3d.map(has_bevel_geometry_in_sp3d).unwrap_or(false);
    let style = child_any(el, "style");
    let effect_idx = style
        .and_then(|s| child_any(s, "effectRef"))
        .and_then(|r| attr_str(r, "idx"))
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let fill_idx = style
        .and_then(|s| child_any(s, "fillRef"))
        .and_then(|r| attr_str(r, "idx"))
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    StyleLbl {
        name,
        has_bevel_geometry,
        effect_idx,
        fill_idx,
    }
}

fn has_bevel_geometry_in_sp3d(sp3d: &Element) -> bool {
    sp3d.children.iter().any(|c| matches!(c, Node::Element(_)))
}

fn primary_node_style_lbl(style_lbls: &[StyleLbl]) -> Option<&StyleLbl> {
    style_lbls
        .iter()
        .find(|lbl| lbl.name == "node1")
        .or_else(|| style_lbls.iter().find(|lbl| is_node_style_label(&lbl.name)))
}

fn is_node_style_label(name: &str) -> bool {
    match name.strip_prefix("node") {
        Some(rest) => !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
}

fn resolve_smart_art_effect_intensity(style_lbls: &[StyleLbl]) -> Option<SmartArtEffectIntensity> {
    let primary = primary_node_style_lbl(style_lbls)?;
    if primary.has_bevel_geometry {
        return Some(SmartArtEffectIntensity::Intense);
    }
    let effect_idx = primary.effect_idx;
    let fill_idx = primary.fill_idx;
    if effect_idx >= 2 || fill_idx >= 3 {
        return Some(SmartArtEffectIntensity::Intense);
    }
    if effect_idx >= 1 || fill_idx >= 2 {
        return Some(SmartArtEffectIntensity::Moderate);
    }
    Some(SmartArtEffectIntensity::Subtle)
}

/// Runtime `data.style` on `PptxSmartArtData`: moderate/intense only (TS `PptxHandlerRuntimeSmartArt`).
pub fn data_style_from_effect_intensity(
    intensity: Option<SmartArtEffectIntensity>,
) -> Option<super::style_helpers::DiagramStyleIntensity> {
    use super::style_helpers::DiagramStyleIntensity;
    match intensity {
        Some(SmartArtEffectIntensity::Moderate) => Some(DiagramStyleIntensity::Moderate),
        Some(SmartArtEffectIntensity::Intense) => Some(DiagramStyleIntensity::Intense),
        Some(SmartArtEffectIntensity::Subtle) | None => None,
    }
}

fn child_any<'a>(el: &'a Element, local: &str) -> Option<&'a Element> {
    el.children.iter().find_map(|c| match c {
        Node::Element(e) if e.local == local => Some(e),
        _ => None,
    })
}

fn attr_str(el: &Element, name: &str) -> Option<String> {
    el.attrs
        .iter()
        .find(|a| a.local == name)
        .map(|a| a.value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    const QS_INTENSE_EFFECT_REF: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<dgm:styleDef xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <dgm:styleLbl name="node1">
    <dgm:sp3d/>
    <dgm:style>
      <a:lnRef idx="2"><a:schemeClr val="accent1"/></a:lnRef>
      <a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef>
      <a:effectRef idx="2"><a:schemeClr val="accent1"/></a:effectRef>
      <a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef>
    </dgm:style>
  </dgm:styleLbl>
</dgm:styleDef>"#;

    #[test]
    fn quick_style_node1_effect_ref_idx_2_is_intense() {
        // Catches: quickStyle node1 effectRef idx=2 never becomes drop-shadow so SmartArt nodes float without TS intense shadow.
        let intensity = resolve_smart_art_effect_intensity_from_qs_xml(QS_INTENSE_EFFECT_REF);
        assert_eq!(intensity, Some(SmartArtEffectIntensity::Intense));
        assert_eq!(
            data_style_from_effect_intensity(intensity),
            Some(super::super::style_helpers::DiagramStyleIntensity::Intense)
        );
    }
}
