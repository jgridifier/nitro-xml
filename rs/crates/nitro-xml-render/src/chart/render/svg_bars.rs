// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-svg-bars.ts

use super::bar_cluster_geometry::clustered_bar_geometry;
use super::chart_spacing::chart_bar_spacing;
use super::chart_view::ChartViewModel;
use super::summary::ChartSummary;
use super::svg_cartesian::Frame;
use super::svg_util::rect;

pub fn bars(
    model: &ChartViewModel,
    frame: &Frame,
    out: &mut Vec<String>,
    values: &[Vec<Option<f64>>],
) {
    let stacked = model.grouping == "stacked" || model.grouping == "percentStacked";
    let count = model.series.len().max(1);
    let summary = ChartSummary {
        chart_type: model.type_,
        grouping: Some(model.grouping.clone()),
        bar_gap_width: model.bar_gap_width,
        bar_overlap: model.bar_overlap,
        ..Default::default()
    };
    let (gap, overlap) = chart_bar_spacing(&summary);
    let geometry = clustered_bar_geometry(frame.band_size, count, gap, overlap);
    let bar_size = geometry.single_bar_width;
    let base = frame.value(0.0);
    let mut pos = vec![0.0; model.categories.len()];
    let mut neg = vec![0.0; model.categories.len()];
    for (si, s) in model.series.iter().enumerate() {
        let row = values.get(si);
        for i in 0..model.categories.len() {
            let v = match row.and_then(|r| r.get(i)).copied().flatten() {
                Some(x) => x,
                None => continue,
            };
            let (from, to) = if stacked {
                let acc = if v >= 0.0 { &mut pos } else { &mut neg };
                let from = acc[i];
                let to = from + v;
                acc[i] = to;
                (from, to)
            } else {
                (0.0, v)
            };
            let a = if stacked { frame.value(from) } else { base };
            let b = frame.value(to);
            let offset = if model.horizontal {
                geometry.cluster_width / 2.0 - bar_size - si as f64 * geometry.step
            } else {
                -geometry.cluster_width / 2.0 + si as f64 * geometry.step
            };
            let center = frame.band(i);
            let _width = if model.horizontal {
                (b - a).abs()
            } else {
                bar_size
            };
            let _height = if model.horizontal {
                bar_size
            } else {
                (b - a).abs()
            };
            let color = s
                .point_colors
                .as_ref()
                .and_then(|c| c.get(i))
                .unwrap_or(&s.color);
            out.push(format!(
                "<g data-chart-series=\"{}\" data-chart-point=\"{}\">",
                si, i
            ));
            if model.horizontal {
                out.push(rect(
                    a.min(b),
                    center + offset,
                    (b - a).abs(),
                    bar_size,
                    color,
                    s.shadow_filter.as_deref(),
                ));
            } else {
                out.push(rect(
                    center + offset,
                    a.min(b),
                    bar_size,
                    (b - a).abs(),
                    color,
                    s.shadow_filter.as_deref(),
                ));
            }
            out.push("</g>".to_string());
        }
    }
}
