// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/color-palettes.ts

use nitro_xml_opc::drawingml::DrawingColorKind;
use nitro_xml_opc::drawingml::{resolve_drawing_color, DrawingColor, TransformOrder};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
pub(crate) struct PaletteJson {
    id: i32,
    #[serde(rename = "meth")]
    meth: String,
    base: Vec<String>,
    variations: Vec<HashMap<String, i64>>,
}

static PALETTES: OnceLock<Vec<PaletteJson>> = OnceLock::new();

fn palettes() -> &'static [PaletteJson] {
    PALETTES.get_or_init(|| {
        let raw = include_str!("chart_palettes.json");
        serde_json::from_str(raw).expect("chart_palettes.json")
    })
}

pub fn find_chart_color_palette(id: i32) -> Option<&'static PaletteJson> {
    palettes().iter().find(|p| p.id == id)
}

pub fn within_linear_transform(i: usize, n: usize) -> HashMap<String, i64> {
    if n <= 1 {
        return HashMap::new();
    }
    let step = 140.0 / (n as f64 + 1.0);
    let centre = (n as f64 - 1.0) / 2.0;
    let value = 100.0 - step * (i as f64 - centre).abs();
    if (i as f64) < centre {
        let shade = (value.floor() * 1000.0) as i64;
        HashMap::from([("shade".to_string(), shade)])
    } else if i as f64 > centre {
        let tint = (value.ceil() * 1000.0) as i64;
        HashMap::from([("tint".to_string(), tint)])
    } else {
        HashMap::new()
    }
}

fn chart_palette_series_color_choice(palette: &PaletteJson, i: usize, n: usize) -> DrawingColor {
    let (base, transform) = if palette.meth == "cycle" {
        let base = palette.base[i % palette.base.len()].clone();
        let round = i / palette.base.len();
        let transform = if palette.variations.is_empty() {
            HashMap::new()
        } else {
            palette.variations[round % palette.variations.len()].clone()
        };
        (base, transform)
    } else {
        let index = if palette.meth == "withinLinearReversed" {
            n - 1 - i
        } else {
            i
        };
        (palette.base[0].clone(), within_linear_transform(index, n))
    };
    DrawingColor {
        kind: DrawingColorKind::Scheme,
        value: base,
        transforms: transform
            .into_iter()
            .map(|(name, value)| nitro_xml_opc::drawingml::ColorTransform {
                name,
                value: value.to_string(),
            })
            .collect(),
        fallback: None,
    }
}

pub fn chart_palette_series_color(
    palette: &PaletteJson,
    i: usize,
    n: usize,
    scheme: &HashMap<String, String>,
) -> String {
    let choice = chart_palette_series_color_choice(palette, i, n);
    resolve_drawing_color(
        &choice,
        Some(scheme),
        Some(nitro_xml_opc::drawingml::ResolveOptions {
            transform_order: TransformOrder::Document,
            hsl_rounding: nitro_xml_opc::drawingml::HslRounding::Nearest,
        }),
    )
    .map(|c| c.hex.to_uppercase())
    .unwrap_or_else(|| {
        scheme
            .get(&choice.value)
            .cloned()
            .unwrap_or_else(|| "#000000".to_string())
            .to_uppercase()
    })
}
