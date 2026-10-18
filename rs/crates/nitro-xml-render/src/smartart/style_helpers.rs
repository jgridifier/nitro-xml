// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec
// src/core/diagram/layout/smartart-layout-style-helpers.ts (`styleShadow`, `styleStroke`);
// changes listed in PROVENANCE.md.

/// Effect intensity used when painting SmartArt (matches upstream `DiagramStyleIntensity`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramStyleIntensity {
    Flat,
    Subtle,
    Moderate,
    Intense,
}

/// Drop-shadow filter string for the given style (CSS `filter` value).
pub fn style_shadow(style: DiagramStyleIntensity) -> Option<&'static str> {
    match style {
        DiagramStyleIntensity::Intense => Some("drop-shadow(0 2px 6px rgba(0,0,0,0.35))"),
        DiagramStyleIntensity::Moderate => Some("drop-shadow(0 1px 3px rgba(0,0,0,0.2))"),
        DiagramStyleIntensity::Flat | DiagramStyleIntensity::Subtle => None,
    }
}

/// Stroke width for node outlines when no authored width is present.
pub fn style_stroke(style: DiagramStyleIntensity) -> f64 {
    match style {
        DiagramStyleIntensity::Intense => 2.0,
        DiagramStyleIntensity::Moderate => 1.5,
        DiagramStyleIntensity::Flat | DiagramStyleIntensity::Subtle => 0.0,
    }
}

/// Map runtime `data.style` (moderate/intense only) to the intensity used by helpers.
pub fn diagram_style_from_data_style(
    data_style: Option<DiagramStyleIntensity>,
) -> DiagramStyleIntensity {
    data_style.unwrap_or(DiagramStyleIntensity::Flat)
}
