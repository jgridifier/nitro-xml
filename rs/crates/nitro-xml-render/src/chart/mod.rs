// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/ (S1); changes listed in PROVENANCE.md.

use nitro_xml_opc::chart::ChartSpace;

use crate::measure::MeasureBatch;
use crate::CommandBuffer;

pub struct ChartSvgOptions {
    pub measure_widths: Vec<f64>,
}

pub struct RenderChartSpaceOptions {
    pub width: u32,
    pub height: u32,
    pub svg: ChartSvgOptions,
}

/// TODO(#28): full S1 port (`render-space.ts`, `chart-svg*.ts`).
pub fn render_chart_space_svg(space: &ChartSpace, options: &RenderChartSpaceOptions) -> String {
    let w = options.width.max(1);
    let h = options.height.max(1);
    let label = space
        .plot_area
        .groups
        .first()
        .map(|g| g.element.as_str())
        .unwrap_or("chart");
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" role=\"img\"><text x=\"{tx}\" y=\"{ty}\" text-anchor=\"middle\">{label} chart (S1 port in progress)</text></svg>",
        w = w,
        h = h,
        tx = w / 2,
        ty = h / 2,
        label = label
    )
}

pub fn paint_chart_space(
    space: &ChartSpace,
    options: &RenderChartSpaceOptions,
    buffer: &mut CommandBuffer,
    _measure: &mut MeasureBatch,
    parent_id: u32,
    markup_id: u32,
) {
    let _svg = render_chart_space_svg(space, options);
    buffer.begin();
    buffer.set_inner_markup(parent_id, markup_id);
}
