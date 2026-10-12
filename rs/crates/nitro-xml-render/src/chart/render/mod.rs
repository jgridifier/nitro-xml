// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/ (S1); changes listed in PROVENANCE.md.

mod bar_cluster_geometry;
mod built_in_text_style;
mod chart_3d_depth;
mod chart_appearance;
mod chart_colors;
mod chart_palette_shade;
mod chart_picture_fill;
mod chart_scale;
mod chart_spacing;
mod chart_title_text;
mod chart_value_axis;
mod chart_view;
mod color_palettes;
mod css_font_family;
mod drawing_color_css;
mod drawing_shadow;
mod gradient_coincident_stops;
mod gradient_definition;
mod gradient_markup;
mod gradient_stop_edit;
mod manual_layout_resolve;
mod render_space;
mod sigma_gradient_stops;
mod style_resolve;
mod summary;
mod summary_from_space;
mod svg;
mod svg_appearance;
mod svg_bars;
mod svg_cartesian;
mod svg_legend;
mod svg_lines;
mod svg_radial;
mod svg_scatter;
mod svg_text_metrics;
mod svg_title_text;
mod svg_util;
pub mod theme_palette;
mod wrap_styled_runs;

pub use chart_picture_fill::chart_blip_rel_ids;
pub use render_space::{
    chart_space_kind, render_chart_space_svg, ChartThemeFonts, RenderChartSpaceOptions,
    DRAWN_CHART_TYPES,
};
pub use svg_text_metrics::ChartSvgMeasureOptions;
