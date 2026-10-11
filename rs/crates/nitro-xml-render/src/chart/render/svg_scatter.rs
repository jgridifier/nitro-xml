// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-svg-cartesian.ts (scatter)

use super::chart_scale::{format_axis_value, AxisScale};
use super::chart_view::ChartViewModel;
use super::svg_appearance::{chart_area_rect, chart_stroke, chart_text_attributes};
use super::svg_util::{line, n, text, text_width, Rect};

pub fn scatter(model: &ChartViewModel, area: Rect, out: &mut Vec<String>) -> String {
    let ys = model.value_axis.as_ref().map(|v| &v.scale);
    let xs = model.x_axis.as_ref();
    if ys.is_none() || xs.is_none() {
        return out.join("");
    }
    let ys = ys.unwrap();
    let xs = xs.unwrap();
    let y_labels: Vec<String> = ys
        .ticks
        .iter()
        .map(|t| format_axis_value(*t, false))
        .collect();
    let value_attrs = chart_text_attributes(model, "valueAxis", 10.0);
    let category_attrs = chart_text_attributes(model, "categoryAxis", 10.0);
    let value_size = value_attrs.size.unwrap_or(10.0);
    let category_size = category_attrs.size.unwrap_or(10.0);
    let grid = chart_stroke(model, "gridlineMajor");
    let x_axis = chart_stroke(model, "categoryAxis");
    let y_axis = chart_stroke(model, "valueAxis");
    let value_labels = true;
    let category_labels = true;
    let label_w = y_labels
        .iter()
        .map(|l| text_width(l, value_size))
        .fold(8.0, f64::max)
        + 6.0;
    let plot = Rect {
        x: area.x + label_w,
        y: area.y + 4.0,
        w: (area.w - label_w - 8.0).max(1.0),
        h: (area.h - (category_size * 1.5).max(20.0)).max(1.0),
    };
    let px = |v: f64, s: &AxisScale| plot.x + ((v - s.min) / (s.max - s.min).max(1e-9)) * plot.w;
    let py = |v: f64, s: &AxisScale| {
        plot.y + plot.h - ((v - s.min) / (s.max - s.min).max(1e-9)) * plot.h
    };
    out.push(chart_area_rect(model, "plotArea", plot));
    for (i, t) in ys.ticks.iter().enumerate() {
        out.push(line(
            plot.x,
            py(*t, ys),
            plot.x + plot.w,
            py(*t, ys),
            &grid.0,
            grid.1,
        ));
        if value_labels {
            let mut a = value_attrs.clone();
            a.anchor = Some("end");
            out.push(text(
                plot.x - 4.0,
                py(*t, ys) + value_size * 0.3,
                y_labels.get(i).map(|s| s.as_str()).unwrap_or(""),
                &a,
            ));
        }
    }
    for t in &xs.ticks {
        out.push(line(
            px(*t, xs),
            plot.y + plot.h,
            px(*t, xs),
            plot.y + plot.h + 3.0,
            &x_axis.0,
            x_axis.1,
        ));
        if category_labels {
            let mut a = category_attrs.clone();
            a.anchor = Some("middle");
            out.push(text(
                px(*t, xs),
                plot.y + plot.h + category_size + 3.0,
                &format_axis_value(*t, false),
                &a,
            ));
        }
    }
    out.push(line(
        plot.x,
        plot.y + plot.h,
        plot.x + plot.w,
        plot.y + plot.h,
        &x_axis.0,
        x_axis.1,
    ));
    out.push(line(
        plot.x,
        plot.y,
        plot.x,
        plot.y + plot.h,
        &y_axis.0,
        y_axis.1,
    ));
    for s in &model.series {
        for (i, v) in s.values.iter().enumerate() {
            let x = s
                .x_values
                .as_ref()
                .and_then(|xv| xv.get(i))
                .copied()
                .flatten();
            let v = v.copied().flatten();
            if v.is_none() || x.is_none() {
                continue;
            }
            let color = s
                .point_colors
                .as_ref()
                .and_then(|c| c.get(i))
                .unwrap_or(&s.color);
            let filter = s
                .shadow_filter
                .as_ref()
                .map(|f| format!(" filter=\"{}\"", f))
                .unwrap_or_default();
            out.push(format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"3\" fill=\"{}\"{} />",
                n(px(x.unwrap(), xs)),
                n(py(v.unwrap(), ys)),
                color,
                filter
            ));
        }
    }
    out.join("")
}
