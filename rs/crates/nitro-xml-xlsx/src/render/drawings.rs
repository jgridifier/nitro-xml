// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/drawings.ts (unsupported + chart
// placement); changes listed in PROVENANCE.md.
//! Sheet drawings layer (`.xg-drawings` / `.xg-obj`) on the main scroll plane.

use nitro_xml_opc::chart::parse_chart_space_xml;
use nitro_xml_render::chart::{render_chart_space_svg, ChartSvgOptions, RenderChartSpaceOptions};
use nitro_xml_render::command_buffer::CommandBuffer;
use nitro_xml_render::measure::MeasureBatch;

use crate::intern::StringIntern;
use crate::model::{ActiveWorksheet, DrawingObject};
use crate::render::grid::{DRAWINGS_ID, PLANE_ID};
use crate::view::anchors::anchor_to_pixel_box;
use crate::view::metrics::GridMetrics;

const OBJ_BASE: u32 = 80_000;

fn set_style<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    id: u32,
    prop: &str,
    val: &str,
) {
    let p = intern.intern(prop);
    let v = intern.intern(val);
    buf.set_style(id, p, v);
}

fn set_attr<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    id: u32,
    key: &str,
    val: &str,
) {
    let k = intern.intern(key);
    let v = intern.intern(val);
    buf.set_attr(id, k, v);
}

fn place<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    id: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    set_style(buf, intern, id, "left", &format!("{x}px"));
    set_style(buf, intern, id, "top", &format!("{y}px"));
    set_style(buf, intern, id, "width", &format!("{w}px"));
    set_style(buf, intern, id, "height", &format!("{h}px"));
}

/// Paint `.xg-drawings` children for the active sheet (main plane only for now).
pub fn paint<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    measures: &mut MeasureBatch,
    sheet: &ActiveWorksheet,
    metrics: &GridMetrics,
) {
    if sheet.drawings.is_empty() {
        return;
    }
    let div = intern.intern("div");
    buf.create_element(DRAWINGS_ID, div, PLANE_ID);
    set_attr(buf, intern, DRAWINGS_ID, "class", "xg-drawings");

    for (index, drawing) in sheet.drawings.iter().enumerate() {
        let box_ = anchor_to_pixel_box(drawing.anchor(), metrics);
        if box_.w <= 0 || box_.h <= 0 {
            continue;
        }
        let id = OBJ_BASE + index as u32;
        buf.create_element(id, div, DRAWINGS_ID);
        place(buf, intern, id, box_.x, box_.y, box_.w, box_.h);
        match drawing {
            DrawingObject::Unsupported { description, .. } => {
                set_attr(buf, intern, id, "class", "xg-obj xg-obj-unsupported");
                let label = format!("{description} (not shown)");
                buf.set_text(id, intern.intern(&label));
            }
            DrawingObject::Image { description, name, .. } => {
                // Image bytes → object URL is loader-side; show honest placeholder for now.
                set_attr(buf, intern, id, "class", "xg-obj xg-obj-unsupported");
                let label = description
                    .clone()
                    .or_else(|| name.clone())
                    .unwrap_or_else(|| "picture".into());
                buf.set_text(id, intern.intern(&format!("{label} (not shown)")));
            }
            DrawingObject::Chart {
                chart_xml, name, ..
            } => {
                set_attr(buf, intern, id, "class", "xg-obj xg-chart");
                set_attr(buf, intern, id, "role", "img");
                let aria = name.clone().unwrap_or_else(|| "Chart".into());
                set_attr(buf, intern, id, "aria-label", &aria);
                let parsed = parse_chart_space_xml(chart_xml);
                let svg = render_chart_space_svg(
                    &parsed.chart_space,
                    &RenderChartSpaceOptions {
                        width: box_.w.max(1) as u32,
                        height: box_.h.max(1) as u32,
                        svg: ChartSvgOptions {
                            measure_widths: measures.widths.clone(),
                            ..Default::default()
                        },
                    },
                );
                buf.set_inner_markup(id, intern.intern(&svg));
            }
        }
    }
}
