// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/render-space.ts

use nitro_xml_opc::chart::ChartSpace;

use super::chart_view::chart_summary_view;
use super::summary::ChartSummaryType;
use super::summary_from_space::chart_summary_from_space;
use super::svg::render_chart_svg;
use super::svg_text_metrics::ChartSvgMeasureOptions;
use super::theme_palette::chart_theme_palette;

pub const DRAWN_CHART_TYPES: &[ChartSummaryType] = &[
    ChartSummaryType::Bar,
    ChartSummaryType::Column,
    ChartSummaryType::Line,
    ChartSummaryType::Area,
    ChartSummaryType::Pie,
    ChartSummaryType::Doughnut,
    ChartSummaryType::Scatter,
    ChartSummaryType::Radar,
];

pub struct ChartThemeFonts {
    pub major: Option<String>,
    pub minor: Option<String>,
}

pub struct RenderChartSpaceOptions {
    pub width: u32,
    pub height: u32,
    pub measure: ChartSvgMeasureOptions,
}

pub fn chart_space_kind(space: &ChartSpace) -> (Option<ChartSummaryType>, bool) {
    if space.plot_area.groups.is_empty() {
        return (None, false);
    }
    let summary = chart_summary_from_space(space);
    let drawn = DRAWN_CHART_TYPES.contains(&summary.chart_type);
    (Some(summary.chart_type), drawn)
}

pub fn render_chart_space_svg(space: &ChartSpace, options: &RenderChartSpaceOptions) -> String {
    let summary = chart_summary_from_space(space);
    let theme = chart_theme_palette(None, None, None);
    let view = chart_summary_view(&summary, &theme);
    let mut measure = options.measure.clone();
    render_chart_svg(&view, options.width, options.height, &mut measure)
}
