// SPDX-License-Identifier: MIT
//! P9: S1 chart SVG when the chart part XML is loaded; labelled stub otherwise.
//!
//! Chartex and other unsupported roots stay a labelled frame (full P9 view-model
//! is out of scope). `paint_chart_space` clears the command buffer, so this
//! adapter renders S1 markup and appends `SetInnerMarkup` without `begin`.

use nitro_xml_opc::chart::{parse_chart_space_xml, ChartSpace};
use nitro_xml_render::chart::{render_chart_space_svg, ChartSvgOptions, RenderChartSpaceOptions};
use nitro_xml_render::measure::MeasureBatch;
use nitro_xml_render::CommandBuffer;

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_inner_markup_stub};
use crate::slide::DumpElement;

/// Paint S1 chart SVG into `parent_id` (does not clear `buffer`).
pub fn paint_pptx_chart_xml(
    xml: &str,
    width: u32,
    height: u32,
    buffer: &mut CommandBuffer,
    measure: &mut MeasureBatch,
    parent_id: u32,
    mut intern: impl FnMut(&str) -> u32,
) -> ChartSpace {
    let parsed = parse_chart_space_xml(xml);
    let svg = if parsed
        .issues
        .iter()
        .any(|i| i.code == "CHART_ROOT_UNEXPECTED")
    {
        // cx:/chartex and other unsupported roots: labelled frame (full P9 is out of scope).
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" role=\"img\"><text>chartex (TODO #28)</text></svg>"
        )
    } else {
        render_chart_space_svg(
            &parsed.chart_space,
            &RenderChartSpaceOptions {
                width,
                height,
                svg: ChartSvgOptions {
                    measure_widths: measure.widths.clone(),
                },
            },
        )
    };
    let id = intern(&svg);
    buffer.set_inner_markup(parent_id, id);
    parsed.chart_space
}

/// Chart frame at the element box. Uses S1 when `chart_xml` is loaded.
pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "chart");
    let w = el.width.max(1.0).round() as u32;
    let h = el.height.max(1.0).round() as u32;
    if let Some(xml) = el.chart_xml.clone() {
        let buffer = &mut *ctx.buffer;
        let measure = &mut *ctx.measure;
        let intern = &mut *ctx.intern;
        paint_pptx_chart_xml(&xml, w, h, buffer, measure, id, |s| intern.intern(s));
    } else {
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" role=\"img\"><rect width=\"100%\" height=\"100%\" fill=\"#f5f5f5\" stroke=\"#999\"/><text x=\"8\" y=\"20\" font-size=\"14\" fill=\"#333\">Chart</text></svg>"
        );
        set_inner_markup_stub(ctx, id, &svg);
    }
    ctx.view_ir.push_painted("chart", el, None);
}
