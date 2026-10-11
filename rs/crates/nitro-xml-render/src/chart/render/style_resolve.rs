// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/style-definition.ts (resolve subset)

use nitro_xml_opc::drawingml::{DrawingColor, DrawingFill};
use std::collections::HashMap;

use super::built_in_text_style::ChartStyleEntry;

#[derive(Clone, Debug, Default)]
pub struct ResolvedChartStyleEntry {
    pub font_size: Option<f64>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub color: Option<String>,
    pub line_color: Option<String>,
    pub line_width: Option<f64>,
    pub fill_color: Option<String>,
    pub labels_visible: Option<bool>,
}

pub fn resolve_chart_style_entry(
    source: &ChartStyleEntry,
    resolve: &dyn Fn(&DrawingColor) -> Option<String>,
) -> ResolvedChartStyleEntry {
    let color = source.font_ref_color.as_ref().and_then(resolve);
    ResolvedChartStyleEntry {
        font_size: source.font_size,
        bold: source.bold,
        italic: source.italic,
        underline: None,
        color,
        line_color: None,
        line_width: None,
        fill_color: None,
        labels_visible: None,
    }
}

pub fn fill_color_solid(
    fill: Option<&DrawingFill>,
    resolve: &dyn Fn(&DrawingColor) -> Option<String>,
) -> Option<String> {
    match fill {
        Some(DrawingFill::None) => Some("none".to_string()),
        Some(DrawingFill::Solid { color }) => resolve(color),
        _ => None,
    }
}

pub fn resolve_chart_style_map(
    entries: &HashMap<String, ChartStyleEntry>,
    resolve: &dyn Fn(&DrawingColor) -> Option<String>,
) -> HashMap<String, ResolvedChartStyleEntry> {
    entries
        .iter()
        .map(|(k, v)| (k.clone(), resolve_chart_style_entry(v, resolve)))
        .filter(|(_, v)| {
            v.font_size.is_some() || v.bold.is_some() || v.color.is_some() || v.fill_color.is_some()
        })
        .collect()
}
