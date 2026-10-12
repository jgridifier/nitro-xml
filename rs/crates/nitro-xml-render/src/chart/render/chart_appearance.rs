// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-appearance.ts (S1 subset)

use nitro_xml_opc::chart::ChartTextBody;
use nitro_xml_opc::chart::{ChartShapeProperties, ChartSpace};
use nitro_xml_opc::drawingml::DrawingColor;
use nitro_xml_opc::drawingml::{
    resolve_drawing_color, DrawingTextRunProperties, HslRounding, ResolveOptions, TransformOrder,
};
use std::collections::HashMap;

use super::built_in_text_style::built_in_chart_text_style;
use super::chart_colors::chart_color_scheme;
use super::drawing_color_css::drawing_color_css;
use super::style_resolve::fill_color_solid;
use super::style_resolve::{resolve_chart_style_entry, ResolvedChartStyleEntry};
use super::summary::{ChartSummary, ChartThemePalette};

pub const EMU_PER_POINT: f64 = 12700.0;
pub const EMU_PER_PIXEL: f64 = 9525.0;

pub fn chart_points_to_pixels(points: f64) -> f64 {
    (points * EMU_PER_POINT) / EMU_PER_PIXEL
}

pub type ChartAppearance = HashMap<String, ResolvedChartStyleEntry>;

pub fn chart_appearance(
    _chart: &ChartSummary,
    theme: &ChartThemePalette,
    built_in_style: i32,
) -> Option<ChartAppearance> {
    let id = built_in_style;
    let defaults = built_in_chart_text_style(id);
    if defaults.is_empty() {
        return None;
    }
    let scheme = chart_color_scheme(theme);
    let resolve = |color: &nitro_xml_opc::drawingml::DrawingColor| {
        let resolve_opts = ResolveOptions {
            transform_order: TransformOrder::Document,
            hsl_rounding: HslRounding::HalfDown,
        };
        drawing_color_css(resolve_drawing_color(color, Some(&scheme), Some(resolve_opts)).as_ref())
    };
    let mut out = HashMap::new();
    for (part, entry) in defaults {
        let mut resolved = resolve_chart_style_entry(&entry, &resolve);
        if part == "categoryAxis" || part == "valueAxis" {
            resolved.labels_visible = Some(true);
        }
        if resolved.font_size.is_some() || resolved.color.is_some() {
            out.insert(part, resolved);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn tx_pr_default_properties(tx_pr: &ChartTextBody) -> Option<&DrawingTextRunProperties> {
    tx_pr
        .body
        .paragraphs
        .first()
        .and_then(|p| p.default_properties.as_ref())
}

fn apply_tx_pr_defaults(
    entry: &mut ResolvedChartStyleEntry,
    props: &DrawingTextRunProperties,
    resolve: &dyn Fn(&DrawingColor) -> Option<String>,
) {
    if let Some(sz) = props.size_pt {
        entry.font_size = Some(sz);
    }
    if let Some(b) = props.bold {
        entry.bold = Some(b);
    }
    if let Some(i) = props.italic {
        entry.italic = Some(i);
    }
    if let Some(color) = props.color.as_ref() {
        if let Some(css) = resolve(color) {
            entry.color = Some(css);
        }
    }
}

fn overlay_part(
    appearance: &mut ChartAppearance,
    part: &str,
    tx_pr: Option<&ChartTextBody>,
    resolve: &dyn Fn(&DrawingColor) -> Option<String>,
) {
    let Some(props) = tx_pr.and_then(tx_pr_default_properties) else {
        return;
    };
    let entry = appearance.entry(part.to_string()).or_default();
    apply_tx_pr_defaults(entry, props, resolve);
}

/// Direct `c:txPr` / `defRPr` on the chart part (docx print + pptx bar3D).
pub fn overlay_authored_tx_pr_appearance(
    appearance: &mut ChartAppearance,
    space: &ChartSpace,
    resolve: &dyn Fn(&DrawingColor) -> Option<String>,
) {
    if let Some(title) = &space.title {
        overlay_part(appearance, "title", title.tx_pr.as_ref(), resolve);
    }
    if let Some(legend) = &space.legend {
        overlay_part(appearance, "legend", legend.tx_pr.as_ref(), resolve);
    }
    for axis in &space.plot_area.axes {
        let part = match axis.kind.as_str() {
            "cat" | "date" => "categoryAxis",
            "val" => "valueAxis",
            _ => continue,
        };
        overlay_part(appearance, part, axis.tx_pr.as_ref(), resolve);
    }
}

fn overlay_shape_part(
    appearance: &mut ChartAppearance,
    part: &str,
    sp_pr: Option<&ChartShapeProperties>,
    resolve: &dyn Fn(&DrawingColor) -> Option<String>,
) {
    let Some(sp) = sp_pr else {
        return;
    };
    let entry = appearance.entry(part.to_string()).or_default();
    if let Some(fill) = fill_color_solid(sp.fill.as_ref(), resolve) {
        entry.fill_color = Some(fill);
    }
    if let Some(line) = &sp.line {
        if let Some(color) = fill_color_solid(line.fill.as_ref(), resolve) {
            entry.line_color = Some(color);
        }
        if let Some(w) = line.width_emu {
            entry.line_width = Some(w as f64 / EMU_PER_POINT);
        }
    }
}

/// `c:chartSpace/c:spPr` chart-area chrome (fill + outline).
pub fn overlay_chart_space_sp_pr(
    appearance: &mut ChartAppearance,
    space: &ChartSpace,
    resolve: &dyn Fn(&DrawingColor) -> Option<String>,
) {
    overlay_shape_part(appearance, "chartArea", space.sp_pr.as_ref(), resolve);
}

#[cfg(test)]
mod tests {
    use super::*;
    use nitro_xml_opc::chart::parse_chart_space_xml;
    use nitro_xml_opc::drawingml::{
        resolve_drawing_color, HslRounding, ResolveOptions, TransformOrder,
    };

    use crate::chart::render::chart_colors::chart_color_scheme;
    use crate::chart::render::drawing_color_css::drawing_color_css;
    use crate::chart::render::summary::ChartThemePalette;

    // Catches: bar3d/pptx path uses legend/axis sz from txPr (~11.97pt) not bare built-in 10pt.
    #[test]
    fn overlay_tx_pr_replaces_axis_and_legend_font_sizes() {
        let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/upstream/pptx/2789ff8d6772_three-d-charts.pptx");
        let bytes = std::fs::read(&deck).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut chart_xml = String::new();
        {
            let mut file = archive.by_name("ppt/charts/chart1.xml").unwrap();
            std::io::Read::read_to_string(&mut file, &mut chart_xml).unwrap();
        }
        let parsed = parse_chart_space_xml(&chart_xml);
        let mut appearance = ChartAppearance::from([
            (
                "categoryAxis".to_string(),
                ResolvedChartStyleEntry {
                    font_size: Some(10.0),
                    bold: Some(false),
                    ..Default::default()
                },
            ),
            (
                "legend".to_string(),
                ResolvedChartStyleEntry {
                    font_size: Some(10.0),
                    bold: Some(false),
                    ..Default::default()
                },
            ),
        ]);
        let scheme = chart_color_scheme(&ChartThemePalette {
            colors: vec![],
            major_font: "Calibri Light".into(),
            minor_font: "Calibri".into(),
        });
        let resolve = |color: &DrawingColor| {
            let resolve_opts = ResolveOptions {
                transform_order: TransformOrder::Document,
                hsl_rounding: HslRounding::HalfDown,
            };
            drawing_color_css(
                resolve_drawing_color(color, Some(&scheme), Some(resolve_opts)).as_ref(),
            )
        };
        overlay_authored_tx_pr_appearance(&mut appearance, &parsed.chart_space, &resolve);

        let axis_sz = appearance
            .get("categoryAxis")
            .and_then(|e| e.font_size)
            .unwrap();
        let legend_sz = appearance.get("legend").and_then(|e| e.font_size).unwrap();
        let title_sz = appearance.get("title").and_then(|e| e.font_size).unwrap();
        assert!(
            (axis_sz - 11.97).abs() < 0.01,
            "category axis txPr sz: got {axis_sz}"
        );
        assert!(
            (legend_sz - 11.97).abs() < 0.01,
            "legend txPr sz: got {legend_sz}"
        );
        assert!(
            (title_sz - 18.62).abs() < 0.01,
            "title txPr sz: got {title_sz}"
        );
    }
}
