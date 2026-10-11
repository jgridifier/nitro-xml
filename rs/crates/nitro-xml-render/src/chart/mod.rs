// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/render-space.ts (S1); changes listed in PROVENANCE.md.

#[allow(dead_code)]
mod render;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
mod oracle_svg;

use nitro_xml_opc::chart::ChartSpace;

use crate::command_buffer::StringTable;
use crate::measure::MeasureBatch;
use crate::CommandBuffer;

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
    let measure = render::ChartSvgMeasureOptions::from_widths(options.svg.measure_widths.clone());
    render::render_chart_space_svg(
        space,
        &render::RenderChartSpaceOptions {
            width: options.width,
            height: options.height,
            measure,
        },
    )
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
