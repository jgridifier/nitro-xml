// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/docx/print-picture.ts; changes listed in PROVENANCE.md.

use std::collections::HashMap;

use nitro_xml_opc::chart::ChartSpace;

use super::chart_cache::chart_svg_key;
use super::dom::{Dom, StringIntern};
use crate::layout::LayoutChart;
use crate::render::chart::docx_chart_svg;

pub struct PicturePaint<'a> {
    pub part_name: &'a str,
    pub content_type: &'a str,
    pub width_px: f64,
    pub height_px: f64,
    pub chart: Option<&'a LayoutChart>,
    pub alt: &'a str,
}

pub fn paint_picture<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    parent: u32,
    picture: &PicturePaint<'_>,
    urls: &HashMap<String, String>,
    charts: &HashMap<String, ChartSpace>,
    chart_svg_cache: &HashMap<String, String>,
) -> u32 {
    if let Some(chart) = picture.chart {
        return paint_chart(dom, parent, picture, chart, charts, chart_svg_cache);
    }
    let url = if picture.part_name.is_empty() {
        None
    } else {
        urls.get(picture.part_name)
    };
    let tag = if url.is_some() { "img" } else { "div" };
    let el = dom.el(parent, tag);
    if let Some(url) = url {
        dom.attr(el, "src", url);
        dom.attr(el, "alt", picture.alt);
    } else {
        dom.class(el, "dve-print-picture-missing dve-print-picture");
    }
    if url.is_some() {
        dom.class(el, "dve-print-picture");
    }
    size_picture(dom, el, picture.width_px, picture.height_px);
    let _ = picture.content_type;
    el
}

fn paint_chart<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    parent: u32,
    picture: &PicturePaint<'_>,
    chart: &LayoutChart,
    charts: &HashMap<String, ChartSpace>,
    chart_svg_cache: &HashMap<String, String>,
) -> u32 {
    let el = dom.el(parent, "div");
    let space = charts.get(picture.part_name);
    if let Some(space) = space {
        dom.class(el, "dve-print-picture dve-print-chart");
        dom.attr(el, "data-docx-chart", "1");
        dom.attr(el, "title", &chart.label);
        let w = picture.width_px.round().max(1.0) as u32;
        let h = picture.height_px.round().max(1.0) as u32;
        let key = chart_svg_key(picture.part_name, w, h);
        let svg = chart_svg_cache
            .get(&key)
            .cloned()
            .unwrap_or_else(|| docx_chart_svg(space, w, h, &[]));
        dom.markup(el, &svg);
    } else if let Some(svg) = chart.svg.as_deref().filter(|s| !s.is_empty()) {
        dom.class(el, "dve-print-picture dve-print-chart");
        dom.attr(el, "data-docx-chart", "1");
        dom.attr(el, "title", &chart.label);
        dom.markup(el, svg);
    } else {
        dom.class(
            el,
            "dve-print-picture dve-print-picture-missing dve-print-chart-frame",
        );
        dom.attr(el, "data-docx-chart", "frame");
        dom.text(el, &chart.label);
    }
    size_picture(dom, el, picture.width_px, picture.height_px);
    el
}

fn size_picture<I: StringIntern>(dom: &mut Dom<'_, I>, el: u32, width: f64, height: f64) {
    dom.style(el, "position", "absolute");
    dom.px(el, "width", width);
    dom.px(el, "height", height);
}
