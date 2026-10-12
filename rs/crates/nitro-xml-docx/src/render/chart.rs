// SPDX-License-Identifier: MIT
//! D4 chart adapter → `nitro-xml-render::chart` (S1).

use nitro_xml_opc::chart::ChartSpace;
use nitro_xml_render::chart::{
    probe_chart_space_measures, ChartSvgOptions, ChartThemeFonts, DEFAULT_THEME_COLORS,
    RenderChartSpaceOptions, THEME_SLOTS,
};
use nitro_xml_render::measure::MeasureBatch;
use nitro_xml_render::{CommandBuffer, StringTable};
use serde_json::Value;

/// Build chart SVG options from the document theme catalog (upstream `themeCatalogColorScheme`).
pub fn chart_svg_options(theme: Option<&Value>, measure_widths: &[f64]) -> ChartSvgOptions {
    ChartSvgOptions {
        measure_widths: measure_widths.to_vec(),
        theme_colors: theme_color_vector(theme),
        theme_fonts: chart_theme_fonts(theme),
    }
}

fn theme_color_vector(theme: Option<&Value>) -> Option<Vec<String>> {
    let theme = theme?;
    let colors = theme.get("colors")?.as_object()?;
    let vec = THEME_SLOTS
        .iter()
        .enumerate()
        .map(|(i, slot)| {
            colors
                .get(*slot)
                .and_then(|v| v.as_str())
                .map(|s| s.trim_start_matches('#').to_uppercase())
                .or_else(|| DEFAULT_THEME_COLORS.get(i).map(|s| s.to_string()))
                .unwrap_or_else(|| "000000".to_string())
        })
        .collect();
    Some(vec)
}

fn chart_theme_fonts(theme: Option<&Value>) -> ChartThemeFonts {
    let latin = |role: &str| {
        theme
            .and_then(|t| t.get("fonts"))
            .and_then(|f| f.get(role))
            .and_then(|r| r.get("latin"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    ChartThemeFonts {
        major: latin("major"),
        minor: latin("minor"),
    }
}

/// SVG for one chart, without clearing the frame buffer (`paint_chart_space` calls `begin`).
pub fn docx_chart_svg(
    space: &ChartSpace,
    width: u32,
    height: u32,
    measure_widths: &[f64],
    theme: Option<&Value>,
) -> String {
    let svg = chart_svg_options(theme, measure_widths);
    nitro_xml_render::chart::render_chart_space_svg(
        space,
        &RenderChartSpaceOptions { width, height, svg },
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
    theme: Option<&Value>,
) {
    let widths = if measure.widths.is_empty() {
        Vec::new()
    } else {
        let w = measure.widths.clone();
        measure.widths.clear();
        w
    };
    let svg = docx_chart_svg(space, width, height, &widths, theme);
    let markup_id = strings.intern(&svg);
    // Do not `buffer.begin()` — Print Layout embeds the chart in an existing frame.
    buffer.set_inner_markup(parent_id, markup_id);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use nitro_xml_opc::chart::parse_chart_space_xml;
    use nitro_xml_render::measure::MeasureBatch;

    use super::{chart_svg_options, docx_chart_svg, probe_docx_chart_measures};
    use crate::body::read_package;

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
            !batch.is_empty(),
            "expected chart SVG paint to queue measureText probes"
        );
        let estimated = docx_chart_svg(&space, 480, 320, &[], None);
        let widths: Vec<f64> = vec![42.0; batch.len()];
        let measured = docx_chart_svg(&space, 480, 320, &widths, None);
        assert_ne!(
            estimated, measured,
            "canvas widths should change chart label layout vs estimates"
        );
    }

    #[test]
    fn poi_61745_chart_svg_uses_document_theme_accent1() {
        // Catches: print chart SVG using Office default accent1 (4472C4) instead of the document theme (5B9BD5).
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/real/docx/poi-61745.docx");
        let bytes = std::fs::read(&path).expect("fixture");
        let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let loaded = read_package(leaked).expect("open");
        let theme = loaded.model.theme.as_ref();
        let colors = chart_svg_options(theme, &[])
            .theme_colors
            .expect("theme colors");
        assert_eq!(colors[4], "5B9BD5", "accent1 slot");
        let listing = std::process::Command::new("unzip")
            .args(["-Z1", path.to_str().unwrap()])
            .output()
            .expect("unzip");
        let part = listing
            .stdout
            .split(|&b| b == b'\n')
            .map(|line| std::str::from_utf8(line).unwrap_or(""))
            .find(|p| p.contains("/charts/") && p.ends_with(".xml"))
            .expect("chart part");
        let xml = std::process::Command::new("unzip")
            .args(["-p", path.to_str().unwrap(), part])
            .output()
            .expect("unzip -p")
            .stdout;
        let xml = String::from_utf8_lossy(&xml);
        let space = parse_chart_space_xml(&xml).chart_space;
        let svg = docx_chart_svg(&space, 480, 320, &[], theme);
        assert!(
            svg.contains("5B9BD5") || svg.contains("#5B9BD5"),
            "expected document theme blue in chart SVG"
        );
        assert!(
            !svg.contains("4472C4") && !svg.contains("#4472C4"),
            "default Office accent1 should not appear when document theme is wired"
        );
    }
}
