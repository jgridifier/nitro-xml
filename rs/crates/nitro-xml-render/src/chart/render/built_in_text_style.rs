// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/built-in-text-style.ts (subset for S1)

use nitro_xml_opc::drawingml::DrawingColor;
use nitro_xml_opc::drawingml::DrawingColorKind;

#[derive(Clone, Debug, Default)]
pub struct ChartStyleEntry {
    pub font_size: Option<f64>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub font_ref_color: Option<DrawingColor>,
}

pub fn effective_built_in_chart_style() -> i32 {
    102
}

pub fn built_in_chart_text_style(id: i32) -> std::collections::HashMap<String, ChartStyleEntry> {
    if !((1..=48).contains(&id) || (101..=148).contains(&id)) {
        return std::collections::HashMap::new();
    }
    let legacy = if id > 100 { id - 100 } else { id };
    let common = ChartStyleEntry {
        font_size: Some(10.0),
        bold: Some(false),
        italic: Some(false),
        font_ref_color: Some(DrawingColor {
            kind: DrawingColorKind::Scheme,
            value: if legacy >= 41 { "lt1" } else { "tx1" }.to_string(),
            transforms: vec![],
            fallback: None,
        }),
    };
    std::collections::HashMap::from([
        ("title".to_string(), {
            let mut t = common.clone();
            t.font_size = Some(if id > 100 { 18.0 } else { 10.0 });
            t.bold = Some(true);
            t
        }),
        ("categoryAxis".to_string(), common.clone()),
        ("valueAxis".to_string(), common.clone()),
        ("legend".to_string(), common),
    ])
}
