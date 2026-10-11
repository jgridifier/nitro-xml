// SPDX-License-Identifier: MIT
//! D4 chart adapter → `nitro-xml-render::chart` (S1).

use nitro_xml_opc::chart::ChartSpace;
use nitro_xml_render::chart::{
    paint_chart_space as s1_paint_chart_space, ChartSvgOptions, RenderChartSpaceOptions,
};
use nitro_xml_render::measure::MeasureBatch;
use nitro_xml_render::CommandBuffer;

pub fn paint_docx_chart(
    space: &ChartSpace,
    width: u32,
    height: u32,
    buffer: &mut CommandBuffer,
    measure: &mut MeasureBatch,
    parent_id: u32,
    markup_id: u32,
) {
    s1_paint_chart_space(
        space,
        &RenderChartSpaceOptions {
            width,
            height,
            svg: ChartSvgOptions {
                measure_widths: Vec::new(),
            },
        },
        buffer,
        measure,
        parent_id,
        markup_id,
    );
}
