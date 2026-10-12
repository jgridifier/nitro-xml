// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-svg-bars.ts; changes listed in PROVENANCE.md.

use super::bar_cluster_geometry::clustered_bar_geometry;
use super::chart_3d_depth::{bar3d_extrusion_markup, compute_depth_vector, BarRect};
use super::chart_picture_fill::{fill_for_bar_face, ResolvedPointPicture};
use super::chart_spacing::chart_bar_spacing;
use super::chart_view::ChartViewModel;
use super::summary::ChartSummary;
use super::svg_cartesian::Frame;
use super::svg_util::{esc, n, rect, Rect};

pub fn bars(
    model: &ChartViewModel,
    frame: &Frame,
    out: &mut Vec<String>,
    picture_defs: &mut Vec<String>,
    values: &[Vec<Option<f64>>],
) {
    let stacked = model.grouping == "stacked" || model.grouping == "percentStacked";
    let count = model.series.len().max(1);
    let summary = ChartSummary {
        chart_type: model.type_,
        is3_d: model.is3_d,
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
    let mut bar_rects: Vec<BarRect> = Vec::new();
    let mut front_markup: Vec<String> = Vec::new();

    for (si, s) in model.series.iter().enumerate() {
        let row = values.get(si);
        let series_pictures = model.picture_fills.by_series.get(si);
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
            let color = s
                .point_colors
                .as_ref()
                .and_then(|c| c.get(i))
                .unwrap_or(&s.color);
            let (rx, ry, rw, rh) = if model.horizontal {
                (a.min(b), center + offset, (b - a).abs(), bar_size)
            } else {
                (center + offset, a.min(b), bar_size, (b - a).abs())
            };
            let geom = Rect {
                x: rx,
                y: ry,
                w: rw,
                h: rh,
            };
            let picture: Option<&ResolvedPointPicture> = series_pictures.and_then(|m| m.get(&i));
            let fill = fill_for_bar_face(
                picture,
                model.is3_d,
                "front",
                color,
                &model.picture_prefix,
                si,
                i,
                geom,
                picture_defs,
            );
            if model.is3_d {
                bar_rects.push(BarRect {
                    x: rx,
                    y: ry,
                    w: rw,
                    h: rh,
                    fill: color.clone(),
                    series_index: si,
                });
            }
            front_markup.push(format!(
                "<g data-chart-series=\"{}\" data-chart-point=\"{}\">",
                si, i
            ));
            let opacity = picture.and_then(|p| p.opacity);
            front_markup.push(bar_rect_markup(
                rx,
                ry,
                rw,
                rh,
                &fill,
                s.shadow_filter.as_deref(),
                opacity,
            ));
            front_markup.push("</g>".to_string());
        }
    }

    if model.is3_d {
        let depth = compute_depth_vector(model.view3_d.as_ref());
        let extrusion = bar3d_extrusion_markup(&bar_rects, depth, &model.grouping);
        if !extrusion.is_empty() {
            out.push(extrusion);
        }
    }
    out.extend(front_markup);
}

fn bar_rect_markup(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: &str,
    filter: Option<&str>,
    opacity: Option<f64>,
) -> String {
    if opacity.is_none() {
        return rect(x, y, w, h, fill, filter);
    }
    let filter_attr = filter
        .map(|f| format!(" filter=\"{}\"", esc(f)))
        .unwrap_or_default();
    let opacity_attr = format!(" opacity=\"{}\"", n(opacity.unwrap_or(1.0)));
    format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"{}{}/>",
        n(x),
        n(y),
        n(w.max(0.0)),
        n(h.max(0.0)),
        esc(fill),
        filter_attr,
        opacity_attr
    )
}
