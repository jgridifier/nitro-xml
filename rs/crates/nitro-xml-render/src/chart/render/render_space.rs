// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/render-space.ts

use std::collections::HashMap;

use nitro_xml_opc::chart::ChartSpace;

use super::chart_appearance::overlay_authored_tx_pr_appearance;
use super::chart_picture_fill::resolve_chart_picture_fills;
use super::chart_title_text::chart_title_text;
use super::chart_view::chart_summary_view;
use super::style_resolve::ResolvedChartStyleEntry;
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

#[derive(Clone, Default)]
pub struct ChartThemeFonts {
    pub major: Option<String>,
    pub minor: Option<String>,
}

pub struct RenderChartSpaceOptions<'a> {
    pub width: u32,
    pub height: u32,
    pub measure: ChartSvgMeasureOptions<'a>,
    pub theme_colors: Option<Vec<String>>,
    pub theme_fonts: ChartThemeFonts,
    pub chart_images: HashMap<String, String>,
    pub picture_id_prefix: String,
    /// UI-layer bar3D extrusion; off for S1 oracle parity.
    pub bar3d_extrusion: bool,
    /// Picture pattern fills from chart blips; off for S1 oracle parity.
    pub picture_fills: bool,
}

pub fn chart_space_kind(space: &ChartSpace) -> (Option<ChartSummaryType>, bool) {
    if space.plot_area.groups.is_empty() {
        return (None, false);
    }
    let summary = chart_summary_from_space(space);
    let drawn = DRAWN_CHART_TYPES.contains(&summary.chart_type);
    (Some(summary.chart_type), drawn)
}

pub fn render_chart_space_svg(
    space: &ChartSpace,
    options: &mut RenderChartSpaceOptions<'_>,
) -> String {
    let summary = chart_summary_from_space(space);
    let theme = chart_theme_palette(
        options.theme_colors.clone(),
        options.theme_fonts.major.as_deref(),
        options.theme_fonts.minor.as_deref(),
    );
    let mut view = chart_summary_view(&summary, &theme);
    view.view3_d = space.view3_d.clone();
    view.bar3d_extrusion = options.bar3d_extrusion;
    if options.bar3d_extrusion {
        if let Some(appearance) = view.appearance.as_mut() {
            overlay_authored_tx_pr_appearance(appearance, space, &theme);
        }
    }
    view.picture_fills_enabled = options.picture_fills;
    view.picture_fills = if options.picture_fills {
        resolve_chart_picture_fills(space, &options.chart_images)
    } else {
        super::chart_picture_fill::ChartPictureFills::default()
    };
    view.picture_prefix = options.picture_id_prefix.clone();
    let title_appearance = view
        .appearance
        .as_ref()
        .and_then(|a| a.get("title"))
        .map(|e| ResolvedChartStyleEntry {
            font_size: e.font_size,
            bold: e.bold,
            italic: e.italic,
            underline: e.underline,
            color: e.color.clone(),
            line_color: e.line_color.clone(),
            line_width: e.line_width,
            fill_color: e.fill_color.clone(),
            labels_visible: e.labels_visible,
        });
    if options.bar3d_extrusion {
        view.category_label_offset = space
            .plot_area
            .axes
            .iter()
            .find(|a| a.kind == "catAx" || a.kind == "dateAx")
            .and_then(|a| a.label_offset)
            .unwrap_or(100.0);
    }
    if let Some(title) = &space.title {
        view.title_overlay = title.overlay.unwrap_or(false);
        // Title manual layout comes from chart formatting entries (upstream chart-view), not `c:title/layout`.
        view.title_text = chart_title_text(&summary, title, title_appearance.as_ref());
    }
    render_chart_svg(&view, options.width, options.height, &mut options.measure)
}
