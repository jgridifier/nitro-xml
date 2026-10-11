// SPDX-License-Identifier: MIT
//! X9 chart adapter → `nitro-xml-render::chart` (S1).

use nitro_xml_opc::chart::{parse_chart_space_xml, ChartSpace};
use nitro_xml_render::chart::{paint_chart_space, ChartSvgOptions, RenderChartSpaceOptions};
use nitro_xml_render::measure::MeasureBatch;
use nitro_xml_render::CommandBuffer;

pub fn paint_chart_from_xml(
    xml: &str,
    width: u32,
    height: u32,
    buffer: &mut CommandBuffer,
    measure: &mut MeasureBatch,
    parent_id: u32,
    markup_id: u32,
) -> ChartSpace {
    let parsed = parse_chart_space_xml(xml);
    let options = RenderChartSpaceOptions {
        width,
        height,
        svg: ChartSvgOptions {
            measure_widths: measure.widths.clone(),
        },
    };
    paint_chart_space(
        &parsed.chart_space,
        &options,
        buffer,
        measure,
        parent_id,
        markup_id,
    );
    parsed.chart_space
}
