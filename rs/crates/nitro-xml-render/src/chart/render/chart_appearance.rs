// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-appearance.ts (S1 subset)

use nitro_xml_opc::drawingml::{
    resolve_drawing_color, HslRounding, ResolveOptions, TransformOrder,
};
use std::collections::HashMap;

use super::built_in_text_style::{built_in_chart_text_style, effective_built_in_chart_style};
use super::chart_colors::chart_color_scheme;
use super::drawing_color_css::drawing_color_css;
use super::style_resolve::{resolve_chart_style_entry, ResolvedChartStyleEntry};
use super::summary::{ChartSummary, ChartThemePalette};

pub const EMU_PER_POINT: f64 = 12700.0;
pub const EMU_PER_PIXEL: f64 = 9525.0;

pub fn chart_points_to_pixels(points: f64) -> f64 {
    (points * EMU_PER_POINT) / EMU_PER_PIXEL
}

pub type ChartAppearance = HashMap<String, ResolvedChartStyleEntry>;

pub fn chart_appearance(
    chart: &ChartSummary,
    theme: &ChartThemePalette,
) -> Option<ChartAppearance> {
    let id = effective_built_in_chart_style();
    let defaults = built_in_chart_text_style(id);
    if defaults.is_empty() {
        return None;
    }
    let scheme = chart_color_scheme(theme);
    let resolve_opts = ResolveOptions {
        transform_order: TransformOrder::Document,
        hsl_rounding: HslRounding::HalfDown,
    };
    let resolve = |color: &nitro_xml_opc::drawingml::DrawingColor| {
        drawing_color_css(resolve_drawing_color(color, Some(&scheme), Some(resolve_opts)).as_ref())
    };
    let mut out = HashMap::new();
    for (part, entry) in defaults {
        let resolved = resolve_chart_style_entry(&entry, &resolve);
        if part == "categoryAxis" || part == "valueAxis" {
            resolved.labels_visible = Some(true);
        }
        if resolved.font_size.is_some() || resolved.color.is_some() {
            out.insert(part, resolved);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}
