// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-svg-radial.ts

use super::chart_scale::format_axis_value;
use super::chart_view::ChartViewModel;
use super::summary::ChartSummaryType;
use super::svg_appearance::{chart_area_rect, chart_stroke, chart_text_attributes};
use super::svg_util::{fit, line, n, text, Rect};

fn polar(cx: f64, cy: f64, r: f64, angle: f64) -> (f64, f64) {
    (cx + r * angle.sin(), cy - r * angle.cos())
}

fn slice_path(cx: f64, cy: f64, r: f64, inner: f64, a0: f64, a1: f64) -> String {
    let sweep = a1 - a0;
    if sweep >= std::f64::consts::PI * 2.0 - 1e-9 {
        let mid = a0 + std::f64::consts::PI;
        return slice_path(cx, cy, r, inner, a0, mid) + &slice_path(cx, cy, r, inner, mid, a1);
    }
    let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
    let (x0, y0) = polar(cx, cy, r, a0);
    let (x1, y1) = polar(cx, cy, r, a1);
    if inner <= 0.0 {
        return format!(
            "M{},{} L{},{} A{},{} 0 {} 1 {},{} Z",
            n(cx),
            n(cy),
            n(x0),
            n(y0),
            n(r),
            n(r),
            large,
            n(x1),
            n(y1)
        );
    }
    let (ix1, iy1) = polar(cx, cy, inner, a1);
    let (ix0, iy0) = polar(cx, cy, inner, a0);
    format!(
        "M{},{} A{},{} 0 {} 1 {},{} L{},{} A{},{} 0 {} 0 {},{} Z",
        n(x0),
        n(y0),
        n(r),
        n(r),
        large,
        n(x1),
        n(y1),
        n(ix1),
        n(iy1),
        n(inner),
        n(inner),
        large,
        n(ix0),
        n(iy0)
    )
}

pub fn pie_svg(model: &ChartViewModel, area: Rect) -> String {
    let mut out = vec![chart_area_rect(model, "plotArea", area)];
    let cx = area.x + area.w / 2.0;
    let cy = area.y + area.h / 2.0;
    let radius = (area.w.min(area.h) / 2.0 - 4.0).max(1.0);
    let rings: Vec<_> = if model.type_ == ChartSummaryType::Doughnut {
        model.series.clone()
    } else {
        model.series.first().cloned().into_iter().collect()
    };
    let hole = if model.type_ == ChartSummaryType::Doughnut {
        radius * 0.5
    } else {
        0.0
    };
    let ring_size = (radius - hole) / rings.len().max(1) as f64;
    for (ri, s) in rings.iter().enumerate() {
        let outer = radius - ri as f64 * ring_size;
        let inner = if model.type_ == ChartSummaryType::Doughnut {
            outer - ring_size
        } else {
            0.0
        };
        let total = s
            .values
            .iter()
            .filter_map(|v| v.filter(|x| *x > 0.0))
            .sum::<f64>();
        if total <= 0.0 {
            continue;
        }
        let mut angle = 0.0;
        for (i, v) in s.values.iter().enumerate() {
            let v = *v;
            if v.is_none() || v.unwrap() <= 0.0 {
                continue;
            }
            let v = v.unwrap();
            let sweep = (v / total) * std::f64::consts::PI * 2.0;
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
                "<path d=\"{}\" fill=\"{}\" stroke=\"#FFFFFF\" stroke-width=\"1\"{} />",
                slice_path(cx, cy, outer, inner, angle, angle + sweep),
                color,
                filter
            ));
            angle += sweep;
        }
    }
    out.join("")
}

pub fn radar_svg(model: &ChartViewModel, area: Rect) -> String {
    let mut out = vec![chart_area_rect(model, "plotArea", area)];
    let scale = model.value_axis.as_ref().map(|v| &v.scale);
    let count = model.categories.len();
    if scale.is_none() || count < 1 {
        return out.join("");
    }
    let scale = scale.unwrap();
    let value_attrs = chart_text_attributes(model, "valueAxis", 9.0);
    let category_attrs = chart_text_attributes(model, "categoryAxis", 10.0);
    let value_size = value_attrs.size.unwrap_or(9.0);
    let category_size = category_attrs.size.unwrap_or(10.0);
    let grid = chart_stroke(model, "gridlineMajor");
    let axis = chart_stroke(model, "categoryAxis");
    let cx = area.x + area.w / 2.0;
    let cy = area.y + area.h / 2.0 + 4.0;
    let radius = (area.w.min(area.h) / 2.0 - (category_size * 1.8).max(18.0)).max(1.0);
    let angle = |i: usize| (i as f64 / count as f64) * std::f64::consts::PI * 2.0;
    let r = |v: f64| ((v - scale.min) / (scale.max - scale.min).max(1e-9)) * radius;
    for t in scale.ticks.iter() {
        let ring = (0..count)
            .map(|i| {
                let (x, y) = polar(cx, cy, r(*t), angle(i));
                format!("{},{}", n(x), n(y))
            })
            .collect::<Vec<_>>()
            .join(" ");
        out.push(format!(
            "<polygon points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>",
            ring,
            grid.0,
            n(grid.1)
        ));
        if t.fract() == 0.0 {
            out.push(text(
                cx + 3.0,
                cy - r(*t) + value_size * 0.3,
                &format_axis_value(*t, false),
                &value_attrs,
            ));
        }
    }
    for (i, c) in model.categories.iter().enumerate() {
        let (x, y) = polar(cx, cy, radius, angle(i));
        out.push(line(cx, cy, x, y, &axis.0, axis.1));
        let (lx, ly) = polar(cx, cy, radius + 10.0, angle(i));
        let anchor = if (lx - cx).abs() < 2.0 {
            "middle"
        } else if lx > cx {
            "start"
        } else {
            "end"
        };
        let mut a = category_attrs.clone();
        a.anchor = Some(anchor);
        out.push(text(
            lx,
            ly + category_size * 0.3,
            &fit(c, area.w / 4.0, category_size),
            &a,
        ));
    }
    for s in &model.series {
        let points = s
            .values
            .iter()
            .take(count)
            .enumerate()
            .map(|(i, v)| {
                let (x, y) = polar(cx, cy, r((*v).unwrap_or(scale.min)), angle(i));
                format!("{},{}", n(x), n(y))
            })
            .collect::<Vec<_>>()
            .join(" ");
        let filter = s
            .shadow_filter
            .as_ref()
            .map(|f| format!(" filter=\"{}\"", f))
            .unwrap_or_default();
        out.push(format!(
            "<polygon points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"{} />",
            points, s.color, filter
        ));
    }
    out.join("")
}
