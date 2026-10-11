// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/render-space.ts (S1); changes listed in PROVENANCE.md.

#[cfg(feature = "chart-rust-s1")]
mod render;

#[cfg(not(target_arch = "wasm32"))]
mod oracle_svg;

use nitro_xml_opc::chart::ChartSpace;

use crate::command_buffer::StringTable;
use crate::measure::MeasureBatch;
use crate::CommandBuffer;

#[cfg(feature = "chart-rust-s1")]
pub use render::{chart_space_kind, ChartSvgMeasureOptions, ChartThemeFonts, DRAWN_CHART_TYPES};

pub struct ChartSvgOptions {
    /// Prior-frame `measureText` widths consumed in document order by the painter.
    pub measure_widths: Vec<f64>,
}

pub struct RenderChartSpaceOptions {
    pub width: u32,
    pub height: u32,
    pub svg: ChartSvgOptions,
}

/// Standalone SVG for a parsed chart part (S1).
pub fn render_chart_space_svg(space: &ChartSpace, options: &RenderChartSpaceOptions) -> String {
    #[cfg(feature = "chart-rust-s1")]
    {
        let mut measure =
            render::ChartSvgMeasureOptions::from_widths(options.svg.measure_widths.clone());
        return render::render_chart_space_svg(
            space,
            &render::RenderChartSpaceOptions {
                width: options.width,
                height: options.height,
                measure,
            },
        );
    }
    #[cfg(not(feature = "chart-rust-s1"))]
    {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(svg) = oracle_svg::render_chart_space_svg_oracle(space, options) {
            return svg;
        }
        let w = options.width.max(1);
        let h = options.height.max(1);
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" role=\"img\"><text x=\"{tx}\" y=\"{ty}\" text-anchor=\"middle\">chart (S1)</text></svg>",
            w = w,
            h = h,
            tx = w / 2,
            ty = h / 2,
        )
    }
}

/// Encodes chart SVG into `buffer` as one `SetInnerMarkup` handoff; interns markup into `strings`.
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
            },
        },
    );
    let markup_id = strings.intern(&svg);
    buffer.begin();
    buffer.set_inner_markup(parent_id, markup_id);
    markup_id
}
