// SPDX-License-Identifier: MIT
//! P9 scaffold: delegate S1-supported families to shared chart painter; chartex → TODO(#28).

use nitro_xml_opc::chart::{parse_chart_space_xml, ChartSpace};
use nitro_xml_render::chart::{
    paint_chart_space as s1_paint, ChartSvgOptions, RenderChartSpaceOptions,
};
use nitro_xml_render::measure::MeasureBatch;
use nitro_xml_render::{CommandBuffer, StringTable};

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_inner_markup_stub};
use crate::slide::DumpElement;

#[allow(dead_code)] // wired when chart parts resolve from package (#28).
pub fn paint_pptx_chart_xml(
    xml: &str,
    width: u32,
    height: u32,
    buffer: &mut CommandBuffer,
    measure: &mut MeasureBatch,
    parent_id: u32,
    strings: &mut StringTable,
) -> ChartSpace {
    let parsed = parse_chart_space_xml(xml);
    if parsed
        .issues
        .iter()
        .any(|i| i.code == "CHART_ROOT_UNEXPECTED")
    {
        // cx:/chartex and other unsupported roots: labelled frame parity (full P9 port is out of scope).
        buffer.begin();
        let stub = "<svg xmlns=\"http://www.w3.org/2000/svg\" role=\"img\"><text>chartex (TODO #28)</text></svg>";
        let id = strings.intern(stub);
        buffer.set_inner_markup(parent_id, id);
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
        strings,
    );
    parsed.chart_space
}

/// P9 frame placeholder until chart parts are wired from the package (#28).
pub fn paint_element_stub<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    el: &DumpElement,
    parent: u32,
) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "chart");
    let w = el.width.max(1.0).round() as u32;
    let h = el.height.max(1.0).round() as u32;
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" role=\"img\"><rect width=\"100%\" height=\"100%\" fill=\"#f5f5f5\" stroke=\"#999\"/><text x=\"8\" y=\"20\" font-size=\"14\" fill=\"#333\">Chart</text></svg>"
    );
    set_inner_markup_stub(ctx, id, &svg);
    ctx.view_ir.push_painted("chart", el, None);
}
