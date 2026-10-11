// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/summary-from-space.ts

use nitro_xml_opc::chart::{chart_source_values, ChartGroupKind, ChartSpace, ChartSpaceSeries};
use nitro_xml_opc::drawingml::{DrawingColor, DrawingFill};
use std::collections::HashMap;

use super::summary::{ChartSummary, ChartSummarySeries, ChartSummaryType};

fn solid_color(fill: Option<&DrawingFill>) -> Option<DrawingColor> {
    match fill {
        Some(DrawingFill::Solid { color }) => Some(color.clone()),
        _ => None,
    }
}

fn is_solid_fill(fill: Option<&DrawingFill>) -> bool {
    match fill {
        Some(DrawingFill::Solid { .. }) => true,
        Some(DrawingFill::Unsupported { element }) => element == "solidFill",
        _ => false,
    }
}

fn group_type(kind: ChartGroupKind) -> ChartSummaryType {
    match kind {
        ChartGroupKind::Bar => ChartSummaryType::Bar,
        ChartGroupKind::Line => ChartSummaryType::Line,
        ChartGroupKind::Area => ChartSummaryType::Area,
        ChartGroupKind::Pie | ChartGroupKind::OfPie => ChartSummaryType::Pie,
        ChartGroupKind::Doughnut => ChartSummaryType::Doughnut,
        ChartGroupKind::Scatter => ChartSummaryType::Scatter,
        ChartGroupKind::Radar => ChartSummaryType::Radar,
        ChartGroupKind::Bubble => ChartSummaryType::Bubble,
        ChartGroupKind::Stock => ChartSummaryType::Stock,
        ChartGroupKind::Surface => ChartSummaryType::Surface,
    }
}

fn value_to_f64(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64().filter(|x| x.is_finite()),
        serde_json::Value::String(s) if s.is_empty() => None,
        serde_json::Value::String(s) => s.parse().ok().filter(|x: &f64| x.is_finite()),
        _ => None,
    }
}

fn chart_summary_series(
    source: &ChartSpaceSeries,
    chart_type: ChartSummaryType,
) -> ChartSummarySeries {
    let categories = source.categories.as_ref().or(source.x_values.as_ref());
    let values = source.values.as_ref().or(source.y_values.as_ref());
    let cats = chart_source_values(categories);
    let vals: Vec<Option<f64>> = chart_source_values(values)
        .iter()
        .map(|v| value_to_f64(v))
        .collect();
    let mut out = ChartSummarySeries {
        categories: cats,
        values: vals,
        ..Default::default()
    };
    if let Some(tx) = &source.tx {
        if let Some(text) = &tx.text {
            out.name = Some(text.clone());
        }
        if let Some(ref f) = tx.reference {
            if let Some(formula) = &f.formula {
                out.name_ref = Some(formula.clone());
            }
        }
    }
    if let Some(c) = categories.and_then(|c| c.formula.as_ref()) {
        out.categories_ref = Some(c.clone());
    }
    if let Some(v) = values.and_then(|v| v.formula.as_ref()) {
        out.values_ref = Some(v.clone());
    }
    if let Some(sp) = &source.sp_pr {
        if let Some(e) = &sp.effects_xml {
            out.effects_xml = Some(e.clone());
        }
    }
    let primary = if matches!(
        chart_type,
        ChartSummaryType::Line | ChartSummaryType::Scatter | ChartSummaryType::Radar
    ) {
        source
            .sp_pr
            .as_ref()
            .and_then(|sp| sp.line.as_ref())
            .and_then(|l| l.fill.as_ref())
    } else {
        source.sp_pr.as_ref().and_then(|sp| sp.fill.as_ref())
    };
    let fill = if chart_type == ChartSummaryType::Scatter && !is_solid_fill(primary) {
        source
            .marker
            .as_ref()
            .and_then(|m| m.sp_pr.as_ref())
            .and_then(|sp| sp.fill.as_ref())
    } else {
        primary
    };
    if let Some(c) = solid_color(fill) {
        out.drawing_color = Some(c);
    }
    if let Some(f) = fill {
        let skip = matches!(f, DrawingFill::Solid { .. })
            || (chart_type == ChartSummaryType::Scatter && matches!(f, DrawingFill::None));
        if !skip {
            out.fill = Some(f.clone());
        }
    }
    let mut points: HashMap<String, DrawingColor> = HashMap::new();
    let mut point_fills: HashMap<String, DrawingFill> = HashMap::new();
    for point in &source.data_points {
        if point.index.is_none() {
            continue;
        }
        let idx = point.index.unwrap().to_string();
        let properties = point
            .sp_pr
            .as_ref()
            .or_else(|| point.marker.as_ref().and_then(|m| m.sp_pr.as_ref()));
        if let Some(props) = properties {
            if let Some(c) = solid_color(props.fill.as_ref()) {
                points.insert(idx.clone(), c);
            }
            if let Some(f) = &props.fill {
                if !matches!(f, DrawingFill::Solid { .. }) {
                    point_fills.insert(idx.clone(), f.clone());
                }
            }
        }
    }
    if !points.is_empty() {
        out.point_colors = Some(points);
    }
    if !point_fills.is_empty() {
        out.point_fills = Some(point_fills);
    }
    out
}

pub fn chart_summary_from_space(chart_space: &ChartSpace) -> ChartSummary {
    let group = chart_space.plot_area.groups.first();
    let mut chart_type = group
        .map(|g| group_type(g.kind))
        .unwrap_or(ChartSummaryType::Column);
    if chart_type == ChartSummaryType::Bar {
        if group.and_then(|g| g.bar_direction.as_deref()) != Some("bar") {
            chart_type = ChartSummaryType::Column;
        }
    }
    let series = group
        .map(|g| {
            g.series
                .iter()
                .map(|s| chart_summary_series(s, chart_type))
                .collect()
        })
        .unwrap_or_default();
    let mut summary = ChartSummary {
        chart_type,
        series,
        show_legend: chart_space.legend.is_some(),
        ..Default::default()
    };
    if let Some(g) = group {
        if let Some(grouping) = &g.grouping {
            if ["clustered", "stacked", "percentStacked", "standard"].contains(&grouping.as_str()) {
                summary.grouping = Some(grouping.clone());
            }
        }
        if matches!(chart_type, ChartSummaryType::Bar | ChartSummaryType::Column) {
            let gap = g.gap_width.unwrap_or(150.0);
            if gap.fract() == 0.0 {
                let iv = gap as i32;
                if 0 <= iv && iv <= 500 {
                    summary.bar_gap_width = Some(iv);
                }
            }
            let overlap = g.overlap.unwrap_or(0.0);
            if overlap.fract() == 0.0 {
                let iv = overlap as i32;
                if -100 <= iv && iv <= 100 {
                    summary.bar_overlap = Some(iv);
                }
            }
        }
    }
    if chart_space.title.is_some() && chart_space.auto_title_deleted != Some(true) {
        if let Some(title) = &chart_space.title {
            if let Some(text) = &title.text {
                summary.title = Some(text.clone());
            } else if series.len() == 1 {
                if let Some(name) = &series[0].name {
                    summary.title = Some(name.clone());
                }
            }
        }
    }
    if let Some(legend) = &chart_space.legend {
        if let Some(pos) = &legend.position {
            if matches!(pos.as_str(), "r" | "l" | "t" | "b" | "tr") {
                summary.legend_position = Some(pos.clone());
            }
        }
    }
    summary
}
