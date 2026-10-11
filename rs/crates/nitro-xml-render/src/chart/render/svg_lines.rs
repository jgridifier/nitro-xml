// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-svg-cartesian.ts (lines/areas)

use super::chart_view::ChartViewModel;
use super::summary::ChartSummaryType;
use super::svg_cartesian::Frame;
use super::svg_util::n;

pub fn lines_and_areas(
    model: &ChartViewModel,
    frame: &Frame,
    out: &mut Vec<String>,
    values: &[Vec<Option<f64>>],
) {
    let stacked = model.grouping == "stacked" || model.grouping == "percentStacked";
    let area = model.type_ == ChartSummaryType::Area;
    let mut acc = vec![0.0; model.categories.len()];
    type LineLayer = (Vec<(f64, f64)>, Vec<(f64, f64)>, String, String);
    let mut layers: Vec<LineLayer> = Vec::new();
    for (si, s) in model.series.iter().enumerate() {
        let row = values.get(si);
        let mut points: Vec<(f64, f64)> = Vec::new();
        let mut lower: Vec<(f64, f64)> = Vec::new();
        for (i, slot) in acc.iter_mut().enumerate().take(model.categories.len()) {
            let v = row.and_then(|r| r.get(i)).copied().flatten();
            if v.is_none() && !area {
                points.push((f64::NAN, f64::NAN));
                continue;
            }
            let below = if stacked { *slot } else { 0.0 };
            let top = below + v.unwrap_or(0.0);
            if stacked {
                *slot = top;
            }
            points.push((frame.band(i), frame.value(top)));
            lower.push((frame.band(i), frame.value(below)));
        }
        layers.push((
            points,
            lower,
            s.color.clone(),
            s.shadow_filter
                .as_ref()
                .map(|f| format!(" filter=\"{}\"", f))
                .unwrap_or_default(),
        ));
    }
    let iter: Vec<_> = if area {
        layers.iter().rev().collect()
    } else {
        layers.iter().collect()
    };
    for (points, lower, color, filter) in iter {
        if area {
            let poly: Vec<String> = points
                .iter()
                .chain(lower.iter().rev())
                .map(|(x, y)| format!("{},{}", n(*x), n(*y)))
                .collect();
            out.push(format!(
                "<polygon points=\"{}\" fill=\"{}\" fill-opacity=\"0.85\"{} />",
                poly.join(" "),
                color,
                filter
            ));
            continue;
        }
        let mut segment: Vec<String> = Vec::new();
        let flush = |segment: &mut Vec<String>,
                     out: &mut Vec<String>,
                     color: &str,
                     filter: &str| {
            if segment.len() > 1 {
                out.push(format!(
                    "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2.25\" stroke-linejoin=\"round\"{} />",
                    segment.join(" "),
                    color,
                    filter
                ));
            }
            segment.clear();
        };
        for (x, y) in points {
            if x.is_nan() {
                flush(&mut segment, out, color, filter);
            } else {
                segment.push(format!("{},{}", n(*x), n(*y)));
            }
        }
        flush(&mut segment, out, color, filter);
        for (x, y) in points {
            if !x.is_nan() {
                out.push(format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"2.5\" fill=\"{}\"{} />",
                    n(*x),
                    n(*y),
                    color,
                    filter
                ));
            }
        }
    }
}
