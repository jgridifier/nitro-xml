// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-svg-cartesian.ts

use super::chart_scale::format_axis_value;
use super::chart_view::{category_totals, ChartViewModel};
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

fn axes(model: &ChartViewModel, area: Rect, out: &mut Vec<String>) -> Option<Frame> {
    let axis = model.value_axis.as_ref()?;
    let scale = &axis.scale;
    let percent = axis.percent;
    let labels: Vec<String> = scale
        .ticks
        .iter()
        .map(|t| format_axis_value(*t, percent))
        .collect();
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
        labels
            .iter()
            .map(|l| text_width(l, value_size))
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
    } else {
        Rect {
            x: area.x + label_w,
            y: area.y + 4.0,
            w: (area.w - label_w - 4.0).max(1.0),
            h: (area.h - (category_size * 1.5).max(20.0)).max(1.0),
        }
    };
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
                out.push(text(plot.x - 4.0, p + value_size * 0.3, label, &a));
            }
        }
    }
    let zero_v = scale.min.max(0.0).min(scale.max);
    let zero = if model.horizontal {
        plot.x + ((zero_v - min) / span) * plot.w
    } else {
        plot.y + plot.h - ((zero_v - min) / span) * plot.h
    };
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
            out.push(text(
                p,
                plot.y + plot.h + category_size + 3.0,
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

pub fn cartesian_svg(model: &ChartViewModel, area: Rect, picture_defs: &mut Vec<String>) -> String {
    let mut out: Vec<String> = Vec::new();
    if model.type_ == super::summary::ChartSummaryType::Scatter {
        return super::svg_scatter::scatter(model, area, &mut out);
    }
    let frame = axes(model, area, &mut out);
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
