// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/chart-cartesian-line-area.ts,
// chart-marker-shape.ts, chart-legend-swatch.ts, chart-datapoint-style.ts,
// chart-series-line-style.ts and core/chart/stacked-series.ts (flat line / stacked line subset);
// changes listed in PROVENANCE.md.
//! Line-chart pieces of the pptx chart view-model engine. Anything not ported makes
//! `parse_series` return `None` so the caller falls back to the S1 SVG painter.

use std::collections::HashMap;
use std::fmt::Write as _;

use nitro_xml_opc::chart::ChartSpaceSeries;
use nitro_xml_opc::drawingml::{resolve_drawing_color, DrawingFill};

const PX_PER_PT: f64 = 4.0 / 3.0;
const DEFAULT_MARKER_RADIUS: f64 = 2.5;
const LEGEND_MARKER_RADIUS: f64 = 3.0;
const AUTOMATIC_MARKER_CYCLE: [&str; 9] = [
    "diamond", "square", "triangle", "x", "star", "circle", "plus", "dot", "dash",
];

pub(super) struct PointMarker {
    pub index: i64,
    pub symbol: Option<String>,
    pub size: Option<f64>,
    pub fill: Option<String>,
    pub point_fill: Option<String>,
}

pub(super) struct LineSeries {
    pub idx: i64,
    pub color: Option<String>,
    pub has_marker: bool,
    pub symbol: Option<String>,
    pub size: Option<f64>,
    pub marker_fill: Option<String>,
    pub stroke_px: f64,
    pub points: Vec<PointMarker>,
}

fn solid(fill: Option<&DrawingFill>, scheme: &HashMap<String, String>) -> Option<Option<String>> {
    match fill {
        None => Some(None),
        Some(DrawingFill::Solid { color }) => resolve_drawing_color(color, Some(scheme), None)
            .filter(|r| r.alpha == 1.0)
            .map(|r| Some(r.hex)),
        Some(_) => None,
    }
}

/// `None` = unsupported feature (fall back); `Some` = parsed line series.
pub(super) fn parse_series(
    s: &ChartSpaceSeries,
    position: usize,
    scheme: &HashMap<String, String>,
) -> Option<LineSeries> {
    if s.data_labels.is_some()
        || s.picture_options.is_some()
        || s.smooth == Some(true)
        || s.sp_pr.as_ref().is_some_and(|p| p.effects_xml.is_some())
    {
        return None;
    }
    let mut color = None;
    let mut stroke_px = 2.4;
    if let Some(p) = &s.sp_pr {
        color = solid(p.fill.as_ref(), scheme)?;
        if let Some(ln) = &p.line {
            if ln.dash.as_deref().is_some_and(|d| d != "solid") {
                return None;
            }
            match &ln.fill {
                Some(DrawingFill::None) => return None,
                other => {
                    if let Some(c) = solid(other.as_ref(), scheme)? {
                        color = Some(c);
                    }
                }
            }
            if let Some(w) = ln.width_emu {
                stroke_px = w as f64 / 12700.0 * PX_PER_PT;
            }
        }
    }
    let marker_of = |m: &nitro_xml_opc::chart::ChartMarker| -> Option<(Option<String>, Option<f64>, Option<String>)> {
        let sp = m.sp_pr.as_ref();
        if sp.is_some_and(|p| p.line.is_some() || p.effects_xml.is_some()) {
            return None;
        }
        let fill = solid(sp.and_then(|p| p.fill.as_ref()), scheme)?;
        Some((m.symbol.clone(), m.size, fill))
    };
    let (mut has_marker, mut symbol, mut size, mut marker_fill) = (false, None, None, None);
    if let Some(m) = &s.marker {
        has_marker = true;
        (symbol, size, marker_fill) = marker_of(m)?;
    }
    let mut points = Vec::new();
    for dp in &s.data_points {
        if dp.explosion.is_some() || dp.picture_options.is_some() {
            return None;
        }
        let point_fill = match &dp.sp_pr {
            Some(p) => {
                if p.line.is_some() || p.effects_xml.is_some() {
                    return None;
                }
                solid(p.fill.as_ref(), scheme)?
            }
            None => None,
        };
        let (psym, psize, pfill) = match &dp.marker {
            Some(m) => {
                has_marker = true;
                marker_of(m)?
            }
            None => (None, None, None),
        };
        points.push(PointMarker {
            index: dp.index.unwrap_or(-1),
            symbol: psym,
            size: psize,
            fill: pfill,
            point_fill,
        });
    }
    Some(LineSeries {
        idx: s.index.unwrap_or(position as i64),
        color,
        has_marker,
        symbol,
        size,
        marker_fill,
        stroke_px,
        points,
    })
}

struct Resolved {
    symbol: Option<String>,
    size: Option<f64>,
    fill: Option<String>,
}

/// `resolveDataPointMarker`: point override per field, automatic symbol cycle by `c:idx`.
fn resolve_marker(s: &LineSeries, point_index: i64) -> Resolved {
    let pt = s.points.iter().find(|p| p.index == point_index);
    let raw = pt
        .and_then(|p| p.symbol.clone())
        .or_else(|| s.symbol.clone());
    let symbol = if s.has_marker && raw.as_deref().is_none_or(|r| r == "auto") {
        Some(AUTOMATIC_MARKER_CYCLE[s.idx.rem_euclid(9) as usize].to_owned())
    } else {
        raw
    };
    Resolved {
        symbol,
        size: pt.and_then(|p| p.size).or(s.size),
        fill: pt
            .and_then(|p| p.fill.clone())
            .or_else(|| s.marker_fill.clone()),
    }
}

fn n(v: f64) -> String {
    let r = (v * 1e6).round() / 1e6;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

fn pts(v: &[(f64, f64)]) -> String {
    v.iter()
        .map(|(x, y)| format!("{x:.2},{y:.2}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[allow(clippy::too_many_arguments)]
/// `buildMarkerPrimitive` (no outline: outlined markers are gated out in `parse_series`).
fn marker_markup(
    symbol: Option<&str>,
    size: Option<f64>,
    cx: f64,
    cy: f64,
    fill: &str,
    default_r: f64,
    attrs: &str,
    title: &str,
) -> String {
    if symbol == Some("none") {
        return String::new();
    }
    let r = match size {
        Some(sz) if sz.is_finite() && sz > 0.0 => sz / 2.0,
        _ => default_r,
    };
    let end = if title.is_empty() {
        String::new()
    } else {
        format!("<title>{title}</title>")
    };
    let poly = |points: Vec<(f64, f64)>| {
        format!(
            "<polygon points=\"{}\" fill=\"{fill}\" stroke=\"{fill}\" stroke-width=\"0\" opacity=\"1\"{attrs}>{end}</polygon>",
            pts(&points)
        )
    };
    let path = |d: String| {
        format!(
            "<path d=\"{d}\" fill=\"none\" stroke=\"{fill}\" stroke-width=\"{}\" opacity=\"1\"{attrs}>{end}</path>",
            n((r * 0.4).max(1.0))
        )
    };
    match symbol {
        Some("square") | Some("dash") => {
            let h = if symbol == Some("dash") { r * 0.64 } else { r * 2.0 };
            format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{fill}\" rx=\"0\" opacity=\"1\"{attrs}>{end}</rect>",
                n(cx - r), n(cy - h / 2.0), n(r * 2.0), n(h)
            )
        }
        Some("diamond") => poly(vec![(cx, cy - r), (cx + r, cy), (cx, cy + r), (cx - r, cy)]),
        Some("triangle") => poly(vec![
            (cx, cy - r),
            (cx + r * 0.9, cy + r * 0.75),
            (cx - r * 0.9, cy + r * 0.75),
        ]),
        Some("star") => poly(
            (0..10)
                .map(|i| {
                    let rad = if i % 2 == 0 { r } else { r * 0.5 };
                    let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * i as f64 / 5.0;
                    (cx + rad * a.cos(), cy + rad * a.sin())
                })
                .collect(),
        ),
        Some("plus") => path(format!(
            "M{:.2},{:.2} L{:.2},{:.2} M{:.2},{:.2} L{:.2},{:.2}",
            cx - r, cy, cx + r, cy, cx, cy - r, cx, cy + r
        )),
        Some("x") => path(format!(
            "M{:.2},{:.2} L{:.2},{:.2} M{:.2},{:.2} L{:.2},{:.2}",
            cx - r, cy - r, cx + r, cy + r, cx + r, cy - r, cx - r, cy + r
        )),
        Some("dot") => format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{fill}\" opacity=\"1\"{attrs}>{end}</circle>",
            n(cx), n(cy), n((r * 0.6).max(1.0))
        ),
        _ => format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{fill}\" opacity=\"1\"{attrs}>{end}</circle>",
            n(cx), n(cy), n(r)
        ),
    }
}

/// One series: polyline then a marker per point (`buildLines`).
#[allow(clippy::too_many_arguments)]
pub(super) fn series_markup(
    o: &mut String,
    s: &LineSeries,
    si: usize,
    color: &str,
    name: &str,
    cats: &[String],
    own_values: &[f64],
    xs: &[f64],
    ys: &[f64],
    esc: &dyn Fn(&str) -> String,
    fmt_val: &dyn Fn(f64) -> String,
) {
    let points: Vec<(f64, f64)> = xs.iter().copied().zip(ys.iter().copied()).collect();
    let _ = write!(
        o,
        "<polyline points=\"{}\" stroke=\"{color}\" stroke-width=\"{}\" fill=\"none\" opacity=\"1\" data-chart-part=\"series\" data-chart-series=\"{si}\"></polyline>",
        pts(&points),
        n(s.stroke_px)
    );
    for (i, (x, y)) in points.iter().enumerate() {
        let m = resolve_marker(s, i as i64);
        let point_fill = s
            .points
            .iter()
            .find(|p| p.index == i as i64)
            .and_then(|p| p.point_fill.clone());
        let fill = m.fill.or(point_fill).unwrap_or_else(|| color.to_owned());
        let cat = cats.get(i).map(String::as_str).unwrap_or("");
        let label = [name, cat]
            .iter()
            .filter(|p| !p.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let val = own_values.get(i).copied().unwrap_or(0.0);
        let title = if label.is_empty() {
            fmt_val(val)
        } else {
            format!("{label}: {}", fmt_val(val))
        };
        let attrs = format!(
            " data-chart-part=\"dataPoint\" data-chart-series=\"{si}\" data-chart-point=\"{i}\""
        );
        o.push_str(&marker_markup(
            m.symbol.as_deref(),
            m.size,
            *x,
            *y,
            &fill,
            DEFAULT_MARKER_RADIUS,
            &attrs,
            &esc(&title),
        ));
    }
}

/// `buildLineLegendSwatch`: 10px line plus the series' own marker at the fixed legend radius.
pub(super) fn legend_swatch(s: &LineSeries, color: &str) -> String {
    let m = resolve_marker(s, -1);
    let fill = m.fill.unwrap_or_else(|| color.to_owned());
    let mut o = format!(
        "<line x1=\"0\" y1=\"-2\" x2=\"10\" y2=\"-2\" stroke=\"{color}\" stroke-width=\"2\" opacity=\"1\"></line>"
    );
    o.push_str(&marker_markup(
        m.symbol.as_deref(),
        None,
        5.0,
        -2.0,
        &fill,
        LEGEND_MARKER_RADIUS,
        "",
        "",
    ));
    o
}

/// `computeStackedSeriesPlots` cumulative heights (positive and negative stacks independent).
pub(super) fn stacked_cumulative(values: &[Vec<f64>], cat_count: usize) -> Vec<Vec<f64>> {
    let mut pos = vec![0.0; cat_count];
    let mut neg = vec![0.0; cat_count];
    values
        .iter()
        .map(|vals| {
            (0..cat_count)
                .map(|ci| {
                    let v = vals.get(ci).copied().unwrap_or(0.0);
                    let run = if v < 0.0 { &mut neg[ci] } else { &mut pos[ci] };
                    *run += v;
                    *run
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: stacked line plotting each series at its own value (lines overlap) instead of the
    // running sum, and positive/negative stacks cancelling each other.
    #[test]
    fn stacked_cumulative_runs_per_sign() {
        let vals = vec![vec![2.0, 3.0], vec![2.0, -1.0], vec![1.0, 4.0]];
        let c = stacked_cumulative(&vals, 2);
        assert_eq!(c[1][0], 4.0);
        assert_eq!(c[2], vec![5.0, 7.0]);
        // Negative control: the negative value starts its own stack from zero.
        assert_eq!(c[1][1], -1.0);
    }

    // Catches: automatic markers all drawn as circles (upstream cycles by c:idx: idx1 square, idx2 triangle).
    #[test]
    fn automatic_marker_cycles_by_series_idx() {
        let mk = |idx| LineSeries {
            idx,
            color: None,
            has_marker: true,
            symbol: None,
            size: None,
            marker_fill: None,
            stroke_px: 2.4,
            points: vec![],
        };
        assert_eq!(resolve_marker(&mk(1), 0).symbol.as_deref(), Some("square"));
        assert_eq!(
            resolve_marker(&mk(2), 0).symbol.as_deref(),
            Some("triangle")
        );
        // Negative control: no c:marker element at all keeps an undefined symbol (default dot).
        let mut none = mk(2);
        none.has_marker = false;
        assert_eq!(resolve_marker(&none, 0).symbol, None);
    }
}
