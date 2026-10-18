// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/render-space.ts (S1); changes listed in PROVENANCE.md.

#[allow(dead_code)]
mod render;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
mod oracle_svg;

use std::collections::HashMap;

use nitro_xml_opc::chart::{ChartSpace, ChartUserShape};

use crate::command_buffer::StringTable;
use crate::measure::MeasureBatch;
use crate::CommandBuffer;

pub use render::bar_cluster_geometry::clustered_bar_geometry;
pub use render::chart_axis_nice::{
    axis_target_intervals, nice_value_axis_bounds, VALUE_AXIS_HEADROOM,
};
pub use render::chart_blip_rel_ids;
pub use render::theme_palette::{DEFAULT_THEME_COLORS, THEME_SLOTS};
pub use render::{
    build_chart_user_shape_overlay_svg, chart_space_kind, ChartSvgMeasureOptions, ChartThemeFonts,
    DRAWN_CHART_TYPES,
};

pub struct ChartSvgOptions {
    /// Prior-frame `measureText` widths consumed in document order by the painter.
    pub measure_widths: Vec<f64>,
    /// Office color palette id from the chart part (docx `colorPalette`).
    pub color_palette: Option<i32>,
    /// Document theme colours in [`THEME_SLOTS`] order (hex without `#`); missing slots use Office defaults.
    pub theme_colors: Option<Vec<String>>,
    pub theme_fonts: ChartThemeFonts,
    /// Chart-part relationship ids → image URLs for `a:blipFill` picture fills.
    pub chart_images: HashMap<String, String>,
    /// Unique prefix for SVG pattern element ids (e.g. element id on slide).
    pub picture_id_prefix: String,
    /// UI-layer bar3D extrusion polygons (`applyChart3DDepth`); off for S1 oracle parity.
    pub bar3d_extrusion: bool,
    /// Picture pattern fills from chart blips; off for S1 oracle parity.
    pub picture_fills: bool,
    /// Apply `c:txPr` / chart-space `spPr` from the part (docx print); off for bare S1 oracle dumps.
    pub docx_chart_chrome: bool,
    /// When set, use `c14:style` / `c:style` from the chart space (xlsx live grid / docx print).
    pub use_space_style: bool,
    /// Override axis label size in points from the chart `style1.xml` part (xlsx).
    pub axis_font_size_pt: Option<f64>,
    /// Override legend label size in points from the chart style part.
    pub legend_font_size_pt: Option<f64>,
    /// Override title size in points from the chart style part.
    pub title_font_size_pt: Option<f64>,
    /// Parsed `c:userShapes` overlay (pptx); painted above the plot area.
    pub user_shapes: Option<Vec<ChartUserShape>>,
}

impl Default for ChartSvgOptions {
    fn default() -> Self {
        Self {
            measure_widths: Vec::new(),
            color_palette: None,
            theme_colors: None,
            theme_fonts: ChartThemeFonts {
                major: None,
                minor: None,
            },
            chart_images: HashMap::new(),
            picture_id_prefix: "chart".to_string(),
            bar3d_extrusion: false,
            picture_fills: false,
            docx_chart_chrome: false,
            use_space_style: false,
            axis_font_size_pt: None,
            legend_font_size_pt: None,
            title_font_size_pt: None,
            user_shapes: None,
        }
    }
}

pub struct RenderChartSpaceOptions {
    pub width: u32,
    pub height: u32,
    pub svg: ChartSvgOptions,
}

/// Standalone SVG for a parsed chart part (S1).
pub fn render_chart_space_svg(space: &ChartSpace, options: &RenderChartSpaceOptions) -> String {
    let measure = render::ChartSvgMeasureOptions::from_widths(options.svg.measure_widths.clone());
    render::render_chart_space_svg(
        space,
        &mut render::RenderChartSpaceOptions {
            width: options.width,
            height: options.height,
            measure,
            theme_colors: options.svg.theme_colors.clone(),
            theme_fonts: options.svg.theme_fonts.clone(),
            chart_images: options.svg.chart_images.clone(),
            picture_id_prefix: options.svg.picture_id_prefix.clone(),
            bar3d_extrusion: options.svg.bar3d_extrusion,
            picture_fills: options.svg.picture_fills,
            color_palette: options.svg.color_palette,
            docx_chart_chrome: options.svg.docx_chart_chrome,
            use_space_style: options.svg.use_space_style,
            axis_font_size_pt: options.svg.axis_font_size_pt,
            legend_font_size_pt: options.svg.legend_font_size_pt,
            title_font_size_pt: options.svg.title_font_size_pt,
            user_shapes: options.svg.user_shapes.clone(),
        },
    )
}

/// Queue canvas `measureText` probes for one chart (document order matches [`render_chart_space_svg`]).
pub fn probe_chart_space_measures(
    space: &ChartSpace,
    width: u32,
    height: u32,
    batch: &mut MeasureBatch,
    intern: &mut dyn FnMut(&str) -> u32,
) {
    let _ = render_chart_space_svg_probing(
        space,
        &RenderChartSpaceOptions {
            width,
            height,
            svg: ChartSvgOptions::default(),
        },
        batch,
        intern,
    );
}

/// Render chart SVG while probing `batch` (uses cached widths when present; queues misses).
///
/// Prefer this over [`render_chart_space_svg`] when the host shares a [`MeasureBatch`] with
/// other painters (xlsx cells): sequential `measure_widths` would be polluted by non-chart
/// requests, while the cache is keyed by `(font, text)`.
pub fn render_chart_space_svg_probing(
    space: &ChartSpace,
    options: &RenderChartSpaceOptions,
    batch: &mut MeasureBatch,
    intern: &mut dyn FnMut(&str) -> u32,
) -> String {
    let measure = render::ChartSvgMeasureOptions::probing(batch, intern);
    render::render_chart_space_svg(
        space,
        &mut render::RenderChartSpaceOptions {
            width: options.width,
            height: options.height,
            measure,
            theme_colors: options.svg.theme_colors.clone(),
            theme_fonts: options.svg.theme_fonts.clone(),
            chart_images: options.svg.chart_images.clone(),
            picture_id_prefix: options.svg.picture_id_prefix.clone(),
            bar3d_extrusion: options.svg.bar3d_extrusion,
            picture_fills: options.svg.picture_fills,
            color_palette: options.svg.color_palette,
            docx_chart_chrome: options.svg.docx_chart_chrome,
            use_space_style: options.svg.use_space_style,
            axis_font_size_pt: options.svg.axis_font_size_pt,
            legend_font_size_pt: options.svg.legend_font_size_pt,
            title_font_size_pt: options.svg.title_font_size_pt,
            user_shapes: options.svg.user_shapes.clone(),
        },
    )
}

/// Encodes chart SVG into `buffer` as one `SetInnerMarkup` handoff; interns markup into `strings`.
///
/// Markup is produced only by the Rust S1 SVG writer (document text/attrs escaped via `esc`).
/// Do not pass raw document strings as `SetInnerMarkup` — see `docs/boundary.md`.
pub fn paint_chart_space(
    space: &ChartSpace,
    options: &RenderChartSpaceOptions,
    buffer: &mut CommandBuffer,
    measure: &mut MeasureBatch,
    parent_id: u32,
    strings: &mut StringTable,
) -> u32 {
    let mut widths = options.svg.measure_widths.clone();
    if !measure.widths.is_empty() {
        widths = measure.widths.clone();
        measure.widths.clear();
    }
    let svg = render_chart_space_svg(
        space,
        &RenderChartSpaceOptions {
            width: options.width,
            height: options.height,
            svg: ChartSvgOptions {
                measure_widths: widths,
                theme_colors: options.svg.theme_colors.clone(),
                theme_fonts: options.svg.theme_fonts.clone(),
                chart_images: options.svg.chart_images.clone(),
                picture_id_prefix: options.svg.picture_id_prefix.clone(),
                bar3d_extrusion: options.svg.bar3d_extrusion,
                picture_fills: options.svg.picture_fills,
                color_palette: options.svg.color_palette,
                docx_chart_chrome: options.svg.docx_chart_chrome,
                use_space_style: options.svg.use_space_style,
                axis_font_size_pt: options.svg.axis_font_size_pt,
                legend_font_size_pt: options.svg.legend_font_size_pt,
                title_font_size_pt: options.svg.title_font_size_pt,
                user_shapes: options.svg.user_shapes.clone(),
            },
        },
    );
    let markup_id = strings.intern(&svg);
    buffer.begin();
    buffer.set_inner_markup(parent_id, markup_id);
    markup_id
}
