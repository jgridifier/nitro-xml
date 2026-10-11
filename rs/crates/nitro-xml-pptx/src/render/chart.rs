// SPDX-License-Identifier: MIT
//! P9 scaffold: delegate S1-supported families to shared chart painter; chartex → TODO(#28).

use nitro_xml_opc::chart::{parse_chart_space_xml, ChartSpace};
use nitro_xml_render::chart::{
    paint_chart_space as s1_paint, ChartSvgOptions, RenderChartSpaceOptions,
};
use nitro_xml_render::measure::MeasureBatch;
use nitro_xml_render::CommandBuffer;

pub fn paint_pptx_chart_xml(
    xml: &str,
    width: u32,
    height: u32,
    buffer: &mut CommandBuffer,
    measure: &mut MeasureBatch,
    parent_id: u32,
    markup_id: u32,
) -> ChartSpace {
    let parsed = parse_chart_space_xml(xml);
    if parsed
        .issues
        .iter()
        .any(|i| i.code == "CHART_ROOT_UNEXPECTED")
    {
        // cx:/chartex and other unsupported roots: labelled frame parity (full P9 port is out of scope).
        buffer.begin();
        buffer.set_inner_markup(parent_id, markup_id);
        return parsed.chart_space;
    }
    s1_paint(
        &parsed.chart_space,
        &RenderChartSpaceOptions {
            width,
            height,
            svg: ChartSvgOptions {
                measure_widths: measure.widths.clone(),
            },
        },
        buffer,
        measure,
        parent_id,
        markup_id,
    );
    parsed.chart_space
}
