// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-svg-cartesian.ts

use super::chart_axis_nice::{
    axis_scale_from_nice_bounds, axis_target_intervals, nice_value_axis_bounds, VALUE_AXIS_HEADROOM,
};
use super::chart_plot_layout::{category_axis_band_px, left_value_axis_band_px, PlotLayout};
use super::chart_scale::format_axis_value;
use super::chart_view::{category_totals, ChartViewModel};
use super::summary::ChartSummaryType;
use super::svg_appearance::{chart_area_rect, chart_stroke, chart_text_attributes};
use super::svg_bars::bars;
use super::svg_util::{fit, line, text, text_width, Rect};

pub struct Frame {
    pub plot: Rect,
    pub band_size: f64,
    min: f64,
    span: f64,
    horizontal: bool,
}

impl Frame {
    pub fn value(&self, v: f64) -> f64 {
        if self.horizontal {
            self.plot.x + ((v - self.min) / self.span) * self.plot.w
        } else {
            self.plot.y + self.plot.h - ((v - self.min) / self.span) * self.plot.h
        }
    }

    pub fn band(&self, i: usize) -> f64 {
        if self.horizontal {
            self.plot.y + self.plot.h - self.band_size * (i as f64 + 0.5)
        } else {
            self.plot.x + self.band_size * (i as f64 + 0.5)
        }
    }
}

fn value_data_bounds(model: &ChartViewModel) -> (f64, f64) {
    let stackable = matches!(
        model.type_,
        ChartSummaryType::Bar
            | ChartSummaryType::Column
            | ChartSummaryType::Line
            | ChartSummaryType::Area
    );
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    if stackable && model.grouping == "stacked" {
        for i in 0..model.categories.len() {
            let mut pos = 0.0;
            let mut neg = 0.0;
            for s in &model.series {
                let v = s.values.get(i).copied().flatten().unwrap_or(0.0);
                if v >= 0.0 {
                    pos += v;
                } else {
                    neg += v;
                }
            }
            min = min.min(neg);
            max = max.max(pos);
        }
    } else {
        for s in &model.series {
            for v in s.values.iter().flatten() {
                min = min.min(*v);
                max = max.max(*v);
            }
        }
    }
    if min == f64::INFINITY {
        (0.0, 1.0)
    } else {
        (min, max)
    }
}

fn axes(
    model: &ChartViewModel,
    area: Rect,
    out: &mut Vec<String>,
    plot_layout: Option<PlotLayout>,
) -> Option<Frame> {
    let axis = model.value_axis.as_ref()?;
    let p9_bar3d = model.is3_d
        && model.bar3d_extrusion
        && matches!(
            model.type_,
            ChartSummaryType::Bar | ChartSummaryType::Column
        );
    let percent = axis.percent;
    let mut scale = axis.scale.clone();
    let value_attrs = chart_text_attributes(model, "valueAxis", 10.0);
    let category_attrs = chart_text_attributes(model, "categoryAxis", 10.0);
    let value_size = value_attrs.size.unwrap_or(10.0);
    let category_size = category_attrs.size.unwrap_or(10.0);
    let grid = chart_stroke(model, "gridlineMajor");
    let axis_stroke = chart_stroke(model, "categoryAxis");
    let value_labels = model
        .appearance
        .as_ref()
        .and_then(|a| a.get("valueAxis"))
        .and_then(|e| e.labels_visible)
        .unwrap_or(true);
    let category_labels = model
        .appearance
        .as_ref()
        .and_then(|a| a.get("categoryAxis"))
        .and_then(|e| e.labels_visible)
        .unwrap_or(true);
    let label_w = if value_labels {
        scale
            .ticks
            .iter()
            .map(|t| text_width(&format_axis_value(*t, percent), value_size))
            .fold(8.0, f64::max)
            + 6.0
    } else {
        4.0
    };
    let cats = model.categories.len().max(1);
    let cat_w = (area.w * 0.3).min(
        model
            .categories
            .iter()
            .map(|c| text_width(c, category_size))
            .fold(8.0, f64::max)
            + 6.0,
    );
    let plot = if model.horizontal {
        Rect {
            x: area.x + cat_w,
            y: area.y,
            w: (area.w - cat_w - 4.0).max(1.0),
            h: (area.h - (value_size * 1.5).max(16.0)).max(1.0),
        }
    } else if p9_bar3d {
        if let Some(layout) = plot_layout {
            Rect {
                x: layout.plot_left,
                y: layout.plot_top,
                w: layout.plot_width,
                h: layout.plot_height,
            }
        } else {
            let left = left_value_axis_band_px(label_w);
            let bottom = category_axis_band_px(model.category_label_offset, category_size);
            Rect {
                x: area.x + left,
                y: area.y + 4.0,
                w: (area.w - left - 4.0).max(1.0),
                h: (area.h - bottom - 4.0).max(1.0),
            }
        }
    } else {
        Rect {
            x: area.x + label_w,
            y: area.y + 4.0,
            w: (area.w - label_w - 4.0).max(1.0),
            h: (area.h - (category_size * 1.5).max(20.0)).max(1.0),
        }
    };
    if p9_bar3d {
        let (data_min, data_max) = value_data_bounds(model);
        let bounds = nice_value_axis_bounds(
            data_min,
            data_max,
            axis_target_intervals(plot.h),
            VALUE_AXIS_HEADROOM,
        );
        scale = axis_scale_from_nice_bounds(bounds);
    }
    let labels: Vec<String> = scale
        .ticks
        .iter()
        .map(|t| format_axis_value(*t, percent))
        .collect();
    let span = scale.max - scale.min;
    let span = if span == 0.0 { 1.0 } else { span };
    let min = scale.min;
    let band_size = if model.horizontal { plot.h } else { plot.w } / cats as f64;
    out.push(chart_area_rect(model, "plotArea", plot));
    for (i, t) in scale.ticks.iter().enumerate() {
        let p = if model.horizontal {
            plot.x + ((t - min) / span) * plot.w
        } else {
            plot.y + plot.h - ((t - min) / span) * plot.h
        };
        let label = labels.get(i).map(|s| s.as_str()).unwrap_or("");
        if model.horizontal {
            out.push(line(p, plot.y, p, plot.y + plot.h, &grid.0, grid.1));
            if value_labels {
                let mut a = value_attrs.clone();
                a.anchor = Some("middle");
                out.push(text(p, plot.y + plot.h + value_size + 2.0, label, &a));
            }
        } else {
            out.push(line(plot.x, p, plot.x + plot.w, p, &grid.0, grid.1));
            if value_labels {
                let mut a = value_attrs.clone();
                a.anchor = Some("end");
                if p9_bar3d {
                    a.baseline = Some("central");
                    out.push(text(plot.x - 4.0, p, label, &a));
                } else {
                    out.push(text(plot.x - 4.0, p + value_size * 0.3, label, &a));
                }
            }
        }
    }
    let zero_v = scale.min.max(0.0).min(scale.max);
    let zero = if model.horizontal {
        plot.x + ((zero_v - min) / span) * plot.w
    } else {
        plot.y + plot.h - ((zero_v - min) / span) * plot.h
    };
    let draw_zero_line = if p9_bar3d {
        scale.min < 0.0 && scale.max > 0.0
    } else {
        true
    };
    if draw_zero_line {
        if model.horizontal {
            out.push(line(
                zero,
                plot.y,
                zero,
                plot.y + plot.h,
                &axis_stroke.0,
                axis_stroke.1,
            ));
        } else {
            out.push(line(
                plot.x,
                zero,
                plot.x + plot.w,
                zero,
                &axis_stroke.0,
                axis_stroke.1,
            ));
        }
    }
    let step = (cats as f64
        / (if model.horizontal { plot.h } else { plot.w } / 14.0)
            .floor()
            .max(1.0))
    .ceil()
    .max(1.0) as usize;
    for (i, c) in model.categories.iter().enumerate() {
        if !category_labels || i % step != 0 {
            continue;
        }
        let p = if model.horizontal {
            plot.y + plot.h - band_size * (i as f64 + 0.5)
        } else {
            plot.x + band_size * (i as f64 + 0.5)
        };
        if model.horizontal {
            let mut a = category_attrs.clone();
            a.anchor = Some("end");
            out.push(text(
                plot.x - 4.0,
                p + category_size * 0.3,
                &fit(c, cat_w - 6.0, category_size),
                &a,
            ));
        } else {
            let mut a = category_attrs.clone();
            a.anchor = Some("middle");
            let cat_y = if p9_bar3d {
                plot.y + plot.h + 12.0
            } else {
                plot.y + plot.h + category_size + 3.0
            };
            out.push(text(
                p,
                cat_y,
                &fit(c, band_size * step as f64, category_size),
                &a,
            ));
        }
    }
    Some(Frame {
        plot,
        band_size,
        min,
        span,
        horizontal: model.horizontal,
    })
}

fn plotted(model: &ChartViewModel) -> Vec<Vec<Option<f64>>> {
    if model.grouping != "percentStacked" {
        return model.series.iter().map(|s| s.values.clone()).collect();
    }
    let totals = category_totals(model);
    model
        .series
        .iter()
        .map(|s| {
            s.values
                .iter()
                .enumerate()
                .map(|(i, v)| v.map(|x| x / totals.get(i).copied().unwrap_or(1.0)))
                .collect()
        })
        .collect()
}

pub fn cartesian_svg(
    model: &ChartViewModel,
    area: Rect,
    picture_defs: &mut Vec<String>,
    plot_layout: Option<PlotLayout>,
) -> String {
    let mut out: Vec<String> = Vec::new();
    if model.type_ == super::summary::ChartSummaryType::Scatter {
        return super::svg_scatter::scatter(model, area, &mut out);
    }
    let frame = axes(model, area, &mut out, plot_layout);
    if frame.is_none() {
        return out.join("");
    }
    let frame = frame.unwrap();
    let values = plotted(model);
    if matches!(
        model.type_,
        super::summary::ChartSummaryType::Bar | super::summary::ChartSummaryType::Column
    ) {
        bars(model, &frame, &mut out, picture_defs, &values);
    } else {
        super::svg_lines::lines_and_areas(model, &frame, &mut out, &values);
    }
    out.join("")
}

#[cfg(test)]
mod tests {
    use nitro_xml_opc::chart::parse_chart_space_xml;

    use crate::chart::{render_chart_space_svg, ChartSvgOptions, RenderChartSpaceOptions};

    fn first_bar_rect(svg: &str) -> (f64, f64, f64, f64) {
        let marker = "<g data-chart-series=\"0\" data-chart-point=\"0\">";
        let start = svg.find(marker).expect("bar group");
        let rest = &svg[start..];
        let rect_start = rest.find("<rect x=\"").expect("front rect");
        let rest = &rest[rect_start + 9..];
        let parts = rest.split('"').collect::<Vec<_>>();
        let x = parts[0].parse().expect("x");
        let y = parts[2].parse().expect("y");
        let w = parts[4].parse().expect("w");
        let h = parts[6].parse().expect("h");
        (x, y, w, h)
    }

    // Catches: P9 bar3D plot layout must match TS oracle SVG at 480×320 (S1, no extrusion).
    #[test]
    fn bar3d_clustered_plot_layout_matches_ts_oracle_at_gate_size() {
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
        let svg = render_chart_space_svg(
            &parsed.chart_space,
            &RenderChartSpaceOptions {
                width: 480,
                height: 320,
                svg: ChartSvgOptions::default(),
            },
        );
        assert!(
            !svg.contains("Chart Title"),
            "auto title without c:tx must not paint synthetic placeholder (matches upstream summary)"
        );
        let (x, y, w, h) = first_bar_rect(&svg);
        // TS `renderChartSpaceSvg` at 480×320 (chart1 clustered column).
        const TS_X: f64 = 40.58;
        const TS_Y: f64 = 86.04;
        const TS_W: f64 = 24.78;
        const TS_H: f64 = 187.29;
        assert!(
            (x - TS_X).abs() < 5.0,
            "bar x drift: got {x}, expected ~{TS_X}"
        );
        assert!(
            (y - TS_Y).abs() < 5.0,
            "bar y drift: got {y}, expected ~{TS_Y}"
        );
        assert!(
            (h - TS_H).abs() < 5.0,
            "bar height drift: got {h}, expected ~{TS_H}"
        );
        assert!(
            (w - TS_W).abs() < 5.0,
            "bar width drift: got {w}, expected ~{TS_W}"
        );
        assert!(
            svg.contains(">6</text>"),
            "P9 value axis should top out at 6 for chart1 (global min 1.8 / max 5 + 5% headroom)"
        );
    }

    // Catches: P9 bar3d multi-series empty c:title must paint TS CHART_AUTO_TITLE at txPr title size.
    #[test]
    fn bar3d_multi_series_empty_title_paints_chart_auto_title() {
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

        let svg = render_chart_space_svg(
            &parsed.chart_space,
            &RenderChartSpaceOptions {
                width: 480,
                height: 320,
                svg: ChartSvgOptions {
                    bar3d_extrusion: true,
                    ..Default::default()
                },
            },
        );
        assert!(
            svg.contains(">Chart Title</text>"),
            "multi-series placeholder title must paint visible CHART_AUTO_TITLE on bar3d path: {svg}"
        );
        let title_tag = svg
            .split(">Chart Title</text>")
            .next()
            .and_then(|prefix| prefix.rsplit("<text ").next())
            .unwrap_or("");
        assert!(
            title_tag.contains("24.8") || title_tag.contains("24.9"),
            "title should use txPr sz=1862 (~18.62pt), got: <text {title_tag}"
        );
        let (_, y, _, _) = first_bar_rect(&svg);
        assert!(
            y > 86.04,
            "title band reserve should push clustered bars down (got y={y})"
        );
    }
}
