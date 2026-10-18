// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-svg-legend.ts

use super::chart_plot_layout::{chart_title_reserve_px, CHART_PX_PER_PT};
use super::chart_view::ChartViewModel;
use super::summary::ChartSummaryType;
use super::svg_appearance::{chart_area_rect, chart_text_attributes};
use super::svg_text_metrics::{chart_text_width, fit_chart_text, ChartSvgMeasureOptions};
use super::svg_util::{rect, text, Rect, TextAttrs};

/// P9 `computeChartLegendLayout` defaults (`chart-legend-layout.ts`).
const P9_LEGEND_FONT_SIZE: f64 = 9.0;
const P9_LEGEND_FILL: &str = "#475569";
const P9_LEGEND_ITEM_WIDTH: f64 = 80.0;
const P9_LEGEND_ITEM_HEIGHT: f64 = 14.0;

fn is_p9_bar3d_legend(model: &ChartViewModel) -> bool {
    model.legend_overlay
        && model.bar3d_extrusion
        && model.is3_d
        && matches!(
            model.type_,
            ChartSummaryType::Bar | ChartSummaryType::Column
        )
}

pub struct LegendResult {
    pub plot: Rect,
    pub markup: String,
    pub foreground: bool,
}

pub fn chart_legend_svg(
    model: &ChartViewModel,
    area: Rect,
    frame_w: f64,
    frame_h: f64,
    measure: &mut ChartSvgMeasureOptions<'_>,
) -> LegendResult {
    let mut automatic: Vec<String> = Vec::new();
    let mut bounds_box: Option<Rect> = None;
    let reserved = if is_p9_bar3d_legend(model) {
        p9_bar3d_automatic_legend(
            model,
            area,
            frame_w,
            frame_h,
            &mut automatic,
            &mut bounds_box,
        )
    } else {
        automatic_legend(model, area, &mut automatic, measure, &mut bounds_box)
    };
    let plot = if model.legend_overlay { area } else { reserved };
    if bounds_box.is_none() {
        return LegendResult {
            plot,
            markup: String::new(),
            foreground: false,
        };
    }
    LegendResult {
        plot,
        markup: automatic.join(""),
        foreground: model.legend_overlay,
    }
}

fn legend_entries(model: &ChartViewModel) -> Vec<(String, String)> {
    if matches!(
        model.type_,
        ChartSummaryType::Pie | ChartSummaryType::Doughnut
    ) {
        let first = model.series.first();
        return model
            .categories
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let color = first
                    .and_then(|s| s.point_colors.as_ref().and_then(|c| c.get(i).cloned()))
                    .unwrap_or_else(|| {
                        first
                            .map(|s| s.color.clone())
                            .unwrap_or_else(|| "#4472C4".to_string())
                    });
                (label.clone(), color)
            })
            .collect();
    }
    model
        .series
        .iter()
        .map(|s| (s.name.clone(), s.color.clone()))
        .collect()
}

fn p9_legend_text_attrs() -> TextAttrs {
    TextAttrs {
        size: Some(P9_LEGEND_FONT_SIZE),
        fill: Some(P9_LEGEND_FILL.to_string()),
        ..Default::default()
    }
}

fn p9_plot_top(model: &ChartViewModel) -> f64 {
    // Match svg.rs auto-title defaults (12px), not built-in style 18pt.
    let title_px = if model.auto_title {
        12.0
    } else {
        chart_text_attributes(model, "title", 14.0)
            .size
            .unwrap_or(14.0 * CHART_PX_PER_PT)
    };
    let has_title = model.title.is_some() || model.title_placeholder;
    8.0 + chart_title_reserve_px(has_title, title_px)
}

/// Legend placement matching P9 `computeChartLegendLayout` + `resolveLegendAnchorPosition`.
fn p9_bar3d_automatic_legend(
    model: &ChartViewModel,
    area: Rect,
    frame_w: f64,
    frame_h: f64,
    out: &mut Vec<String>,
    bounds: &mut Option<Rect>,
) -> Rect {
    let entries = legend_entries(model);
    if !model.show_legend || entries.is_empty() {
        return area;
    }
    let pos = model.legend_position.as_str();
    let attrs = p9_legend_text_attrs();
    let n = entries.len() as f64;
    let w = frame_w.max(area.w);
    let h = frame_h.max(area.h);
    let plot_top = p9_plot_top(model);

    let (mut min_x, mut min_y, mut max_x, mut max_y) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    let mut track = |x: f64, y: f64, bw: f64, bh: f64| {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x + bw);
        max_y = max_y.max(y + bh);
    };

    if pos == "t" || pos == "b" {
        let legend_x = w / 2.0;
        let legend_y = if pos == "t" { 28.0 } else { h - 8.0 };
        for (i, (label, color)) in entries.iter().enumerate() {
            let x = legend_x - (n * P9_LEGEND_ITEM_WIDTH) / 2.0 + i as f64 * P9_LEGEND_ITEM_WIDTH;
            let y = legend_y;
            out.push(rect(x, y - 7.0, 10.0, 10.0, color, None));
            out.push(text(x + 13.0, y + 3.0, label, &attrs));
            track(x, y - 7.0, 10.0 + P9_LEGEND_ITEM_WIDTH - 13.0, 10.0);
        }
    } else {
        let (legend_x, legend_y) = match pos {
            "l" => (4.0, plot_top),
            _ => (w - 75.0, if pos == "tr" { 4.0 } else { plot_top }),
        };
        for (i, (label, color)) in entries.iter().enumerate() {
            let x = legend_x;
            let y = legend_y + i as f64 * P9_LEGEND_ITEM_HEIGHT;
            out.push(rect(x, y - 7.0, 10.0, 10.0, color, None));
            out.push(text(x + 13.0, y + 3.0, label, &attrs));
            track(x, y - 7.0, 80.0, 10.0);
        }
    }

    if min_x.is_finite() {
        *bounds = Some(Rect {
            x: min_x - 4.0,
            y: min_y - 4.0,
            w: (max_x - min_x) + 8.0,
            h: (max_y - min_y) + 8.0,
        });
        out.insert(0, chart_area_rect(model, "legend", bounds.unwrap()));
    }
    area
}

fn automatic_legend(
    model: &ChartViewModel,
    area: Rect,
    out: &mut Vec<String>,
    measure: &mut ChartSvgMeasureOptions<'_>,
    bounds: &mut Option<Rect>,
) -> Rect {
    let entries = legend_entries(model);
    if !model.show_legend || entries.is_empty() {
        return area;
    }
    let pos = model.legend_position.as_str();
    let attrs = chart_text_attributes(model, "legend", 10.0);
    let size = attrs.size.unwrap_or(10.0);
    let row_h = (size * 1.4).max(16.0);
    if pos == "t" || pos == "b" {
        // Upstream: Math.min(chartTextWidth(label) + 20, area.w / 2)
        let widths: Vec<f64> = entries
            .iter()
            .map(|(label, _)| (chart_text_width(label, &attrs, measure) + 20.0).min(area.w / 2.0))
            .collect();
        let total = widths.iter().sum::<f64>();
        let mut x = area.x + (area.w - total).max(0.0) / 2.0;
        let y = if pos == "t" {
            area.y + 4.0
        } else {
            area.y + area.h - row_h + 4.0
        };
        *bounds = Some(Rect {
            x: x - 4.0,
            y: y - 4.0,
            w: total.min(area.w) + 8.0,
            h: row_h + 4.0,
        });
        out.push(chart_area_rect(model, "legend", bounds.unwrap()));
        for (i, (label, color)) in entries.iter().enumerate() {
            let w = widths.get(i).copied().unwrap_or(0.0);
            out.push(rect(x, y, 8.0, 8.0, color, None));
            out.push(text(
                x + 12.0,
                y + 8.0,
                &fit_chart_text(label, w - 16.0, &attrs, measure),
                &attrs,
            ));
            x += w;
        }
        return if pos == "t" {
            Rect {
                x: area.x,
                y: area.y + row_h + 4.0,
                w: area.w,
                h: area.h - row_h - 4.0,
            }
        } else {
            Rect {
                x: area.x,
                y: area.y,
                w: area.w,
                h: area.h - row_h - 4.0,
            }
        };
    }
    let width = (area.w * 0.35).min(
        entries
            .iter()
            .map(|(l, _)| chart_text_width(l, &attrs, measure))
            .fold(0.0, f64::max)
            + 22.0,
    );
    let height = entries.len() as f64 * row_h;
    let x = if pos == "l" {
        area.x + 4.0
    } else {
        area.x + area.w - width
    };
    let y0 = if pos == "tr" {
        area.y + 4.0
    } else {
        area.y + (area.h - height).max(0.0) / 2.0
    };
    *bounds = Some(Rect {
        x: x - 4.0,
        y: y0,
        w: width + 4.0,
        h: height.min(area.y + area.h - y0),
    });
    out.push(chart_area_rect(model, "legend", bounds.unwrap()));
    for (i, (label, color)) in entries.iter().enumerate() {
        let y = y0 + i as f64 * row_h;
        if y + row_h > area.y + area.h + 2.0 {
            continue;
        }
        out.push(rect(x, y + 3.0, 8.0, 8.0, color, None));
        out.push(text(
            x + 12.0,
            y + 11.0,
            &fit_chart_text(label, width - 16.0, &attrs, measure),
            &attrs,
        ));
    }
    if pos == "l" {
        Rect {
            x: area.x + width + 8.0,
            y: area.y,
            w: area.w - width - 8.0,
            h: area.h,
        }
    } else {
        Rect {
            x: area.x,
            y: area.y,
            w: area.w - width - 8.0,
            h: area.h,
        }
    }
}

#[cfg(test)]
mod tests {
    use nitro_xml_opc::chart::parse_chart_space_xml;

    use crate::chart::{render_chart_space_svg, ChartSvgOptions, RenderChartSpaceOptions};

    fn chart1_svg(bar3d_extrusion: bool) -> String {
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
        render_chart_space_svg(
            &parsed.chart_space,
            &RenderChartSpaceOptions {
                width: 480,
                height: 320,
                svg: ChartSvgOptions {
                    bar3d_extrusion,
                    ..Default::default()
                },
            },
        )
    }

    fn legend_series_tag(svg: &str, label: &str) -> Option<String> {
        let needle = format!(">{label}</text>");
        svg.split(&needle)
            .next()
            .and_then(|prefix| prefix.rsplit("<text ").next())
            .map(|tag| tag.to_string())
    }

    // Catches: P9 bar3d path painting txPr legend (~15.96px / near-black) instead of layout defaults (9px / #475569).
    #[test]
    fn p9_bar3d_legend_uses_compute_chart_legend_layout_defaults() {
        let svg = chart1_svg(true);
        let tag = legend_series_tag(&svg, "Series 1").expect("legend label");
        assert!(
            tag.contains("font-size=\"9\"") && tag.contains("fill=\"#475569\""),
            "P9 legend tag should be 9px #475569, got: <text {tag}"
        );
        assert!(
            svg.contains("<rect x=\"120\" y=\"305\" width=\"10\" height=\"10\""),
            "P9 legend should use 80px horizontal spacing from computeChartLegendLayout: {svg}"
        );
    }

    // Catches: S1 chart SVG dump (bar3d_extrusion=false) must not pick up P9 legend defaults.
    #[test]
    fn s1_oracle_legend_keeps_s1_automatic_layout_without_bar3d_extrusion() {
        let svg = chart1_svg(false);
        let tag = legend_series_tag(&svg, "Series 1").expect("legend label");
        assert!(
            tag.contains("font-size=\"13.33") && tag.contains("fill=\"#000000\""),
            "S1 legend should stay on automatic_legend styling, got: <text {tag}"
        );
        assert!(
            !svg.contains("fill=\"#475569\""),
            "S1 legend must not use P9-only #475569 default: {svg}"
        );
    }
}
