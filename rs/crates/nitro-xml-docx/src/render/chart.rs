// SPDX-License-Identifier: MIT
//! D4 chart adapter → `nitro-xml-render::chart` (S1).

use nitro_xml_opc::chart::ChartSpace;
use nitro_xml_render::chart::{ChartSvgOptions, RenderChartSpaceOptions};
use nitro_xml_render::measure::MeasureBatch;
use nitro_xml_render::{CommandBuffer, StringTable};

/// SVG for one chart, without clearing the frame buffer (`paint_chart_space` calls `begin`).
pub fn docx_chart_svg(
    space: &ChartSpace,
    width: u32,
    height: u32,
    measure_widths: &[f64],
) -> String {
    nitro_xml_render::chart::render_chart_space_svg(
        space,
        &RenderChartSpaceOptions {
            width,
            height,
            svg: ChartSvgOptions {
                measure_widths: measure_widths.to_vec(),
            },
        },
    )
}

pub fn paint_docx_chart(
    space: &ChartSpace,
    width: u32,
    height: u32,
    buffer: &mut CommandBuffer,
    measure: &mut MeasureBatch,
    parent_id: u32,
    strings: &mut StringTable,
) {
    let widths = if measure.widths.is_empty() {
        Vec::new()
    } else {
        let w = measure.widths.clone();
        measure.widths.clear();
        w
    };
    let svg = docx_chart_svg(space, width, height, &widths);
    let markup_id = strings.intern(&svg);
    // Do not `buffer.begin()` — Print Layout embeds the chart in an existing frame.
    buffer.set_inner_markup(parent_id, markup_id);
}
