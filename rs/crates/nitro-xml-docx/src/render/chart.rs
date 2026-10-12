// SPDX-License-Identifier: MIT
//! D4 chart adapter → `nitro-xml-render::chart` (S1).

use nitro_xml_opc::chart::ChartSpace;
use nitro_xml_render::chart::{
    probe_chart_space_measures, ChartSvgOptions, RenderChartSpaceOptions,
};
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

/// Queue measure probes for one embedded chart (same order as [`docx_chart_svg`] with widths).
pub fn probe_docx_chart_measures(
    space: &ChartSpace,
    width: u32,
    height: u32,
    batch: &mut MeasureBatch,
    intern: &mut dyn FnMut(&str) -> u32,
) {
    probe_chart_space_measures(space, width, height, batch, intern);
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use nitro_xml_opc::chart::parse_chart_space_xml;
    use nitro_xml_render::measure::MeasureBatch;

    use super::{docx_chart_svg, probe_docx_chart_measures};

    #[test]
    fn chart_probe_queues_canvas_measure_requests() {
        // Catches: docx print charts painted with empty measure_widths (estimated label metrics vs canvas).
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/real/docx/poi-61745.docx");
        let listing = std::process::Command::new("unzip")
            .args(["-Z1", root.to_str().unwrap()])
            .output()
            .expect("unzip");
        let part = listing
            .stdout
            .split(|&b| b == b'\n')
            .map(|line| std::str::from_utf8(line).unwrap_or(""))
            .find(|p| p.contains("/charts/") && p.ends_with(".xml"))
            .expect("chart part");
        let xml = std::process::Command::new("unzip")
            .args(["-p", root.to_str().unwrap(), part])
            .output()
            .expect("unzip -p")
            .stdout;
        let xml = String::from_utf8_lossy(&xml);
        let space = parse_chart_space_xml(&xml).chart_space;
        let mut batch = MeasureBatch::new();
        let mut strings = std::collections::HashMap::<String, u32>::new();
        let mut next = 1u32;
        let mut intern = |s: &str| -> u32 {
            if let Some(id) = strings.get(s) {
                return *id;
            }
            let id = next;
            next += 1;
            strings.insert(s.to_string(), id);
            id
        };
        probe_docx_chart_measures(&space, 480, 320, &mut batch, &mut intern);
        assert!(
            batch.len() > 0,
            "expected chart SVG paint to queue measureText probes"
        );
        let estimated = docx_chart_svg(&space, 480, 320, &[]);
        let widths: Vec<f64> = vec![42.0; batch.len()];
        let measured = docx_chart_svg(&space, 480, 320, &widths);
        assert_ne!(
            estimated, measured,
            "canvas widths should change chart label layout vs estimates"
        );
    }
}
