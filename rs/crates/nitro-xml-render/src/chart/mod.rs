// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/ (S1); changes listed in PROVENANCE.md.

mod oracle_svg;

use nitro_xml_opc::chart::ChartSpace;

use crate::command_buffer::StringTable;
use crate::measure::MeasureBatch;
use crate::CommandBuffer;

pub struct ChartSvgOptions {
    /// Prior-frame `measureText` widths consumed in document order by the painter.
    pub measure_widths: Vec<f64>,
}

pub struct RenderChartSpaceOptions {
    pub width: u32,
    pub height: u32,
    pub svg: ChartSvgOptions,
}

/// Standalone SVG for a parsed chart part (S1). Native builds delegate to the upstream oracle
/// until the in-crate painter reaches byte parity (#28); WASM returns a labelled stub.
pub fn render_chart_space_svg(space: &ChartSpace, options: &RenderChartSpaceOptions) -> String {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(svg) = oracle_svg::render_chart_space_svg_oracle(space, options) {
        return svg;
    }
    let w = options.width.max(1);
    let h = options.height.max(1);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" role=\"img\"><text x=\"{tx}\" y=\"{ty}\" text-anchor=\"middle\">chart (S1 Rust painter #28)</text></svg>",
        w = w,
        h = h,
        tx = w / 2,
        ty = h / 2,
    )
}

/// Encodes chart SVG into `buffer` as one `SetInnerMarkup` handoff; interns markup into `strings`.
pub fn paint_chart_space(
    space: &ChartSpace,
    options: &RenderChartSpaceOptions,
    buffer: &mut CommandBuffer,
    _measure: &mut MeasureBatch,
    parent_id: u32,
    strings: &mut StringTable,
) -> u32 {
    let svg = render_chart_space_svg(space, options);
    let markup_id = strings.intern(&svg);
    buffer.begin();
    buffer.set_inner_markup(parent_id, markup_id);
    markup_id
}
