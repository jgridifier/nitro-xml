// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-view.ts

use nitro_xml_opc::drawingml::{
    resolve_drawing_color, DrawingFill, HslRounding, ResolveOptions, TransformOrder,
};
use std::collections::HashMap;

use super::chart_appearance::{chart_appearance, ChartAppearance};
use super::chart_colors::{auto_series_color, chart_color_scheme};
use super::chart_scale::{nice_scale, AxisScale, NiceScaleOptions};
use super::chart_title_text::chart_title_text;
use super::chart_value_axis::value_axis;
use super::color_palettes::{chart_palette_series_color, find_chart_color_palette};
use super::drawing_color_css::drawing_color_css;
use super::drawing_shadow::{resolve_drawing_shadow_xml, DrawingSvgShadow};
use super::gradient_definition::{resolve_chart_gradient, ChartGradientFill};
use super::summary::{ChartSummary, ChartSummaryType, ChartThemePalette};

#[derive(Clone, Debug)]
pub struct ChartSeriesView {
    pub name: String,
    pub values: Vec<Option<f64>>,
    pub x_values: Option<Vec<Option<f64>>>,
    pub color: String,
    pub point_colors: Option<Vec<String>>,
    pub shadow_filter: Option<String>,
    pub shadow: Option<DrawingSvgShadow>,
    pub gradient: Option<ChartGradientFill>,
    pub point_gradients: Option<HashMap<usize, ChartGradientFill>>,
}

#[derive(Clone, Debug)]
pub struct ValueAxisView {
    pub scale: AxisScale,
    pub percent: bool,
}

#[derive(Clone, Debug)]
pub struct ChartViewModel {
    pub type_: ChartSummaryType,
    pub grouping: String,
    pub title: Option<String>,
    pub show_legend: bool,
    pub legend_position: String,
    pub categories: Vec<String>,
    pub series: Vec<ChartSeriesView>,
    pub value_axis: Option<ValueAxisView>,
    pub x_axis: Option<AxisScale>,
    pub horizontal: bool,
    pub supported: bool,
    pub appearance: Option<ChartAppearance>,
    pub bar_gap_width: Option<i32>,
    pub bar_overlap: Option<i32>,
    pub title_overlay: bool,
    pub legend_overlay: bool,
}

fn supported_type(t: ChartSummaryType) -> bool {
    matches!(
        t,
        ChartSummaryType::Bar
            | ChartSummaryType::Column
            | ChartSummaryType::Line
            | ChartSummaryType::Area
            | ChartSummaryType::Pie
            | ChartSummaryType::Doughnut
            | ChartSummaryType::Scatter
            | ChartSummaryType::Radar
    )
}

fn to_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

pub fn chart_summary_view(chart: &ChartSummary, theme: &ChartThemePalette) -> ChartViewModel {
    let type_ = chart.chart_type;
    let radial = matches!(type_, ChartSummaryType::Pie | ChartSummaryType::Doughnut);
    let palette = chart.color_palette.and_then(find_chart_color_palette);
    let scheme = chart_color_scheme(theme);
    let resolve_opts = ResolveOptions {
        transform_order: TransformOrder::Document,
        hsl_rounding: HslRounding::Nearest,
    };
    let drawing_color = |color: Option<&nitro_xml_opc::drawingml::DrawingColor>| -> Option<String> {
        color.and_then(|c| {
            drawing_color_css(resolve_drawing_color(c, Some(&scheme), Some(resolve_opts)).as_ref())
        })
    };
    let gradient = |fill: Option<&DrawingFill>| -> Option<ChartGradientFill> {
        fill.and_then(|f| {
            if matches!(f, DrawingFill::Gradient { .. }) {
                resolve_chart_gradient(f, &scheme)
            } else {
                None
            }
        })
    };
    let grouping = chart.grouping.clone().unwrap_or_else(|| {
        if matches!(type_, ChartSummaryType::Bar | ChartSummaryType::Column) {
            "clustered".to_string()
        } else {
            "standard".to_string()
        }
    });
    let mut categories: Vec<String> = Vec::new();
    let mut raw_categories: Vec<Vec<serde_json::Value>> = Vec::new();
    let mut series: Vec<ChartSeriesView> = Vec::new();
    for (i, s) in chart.series.iter().enumerate() {
        let values: Vec<Option<f64>> = s.values.clone();
        let cats = s.categories.clone();
        raw_categories.push(cats);
        let palette_color =
            palette.map(|p| chart_palette_series_color(p, i, chart.series.len(), &scheme));
        let color = if matches!(s.fill.as_ref(), Some(DrawingFill::None)) {
            "none".to_string()
        } else {
            drawing_color(s.drawing_color.as_ref())
                .or(palette_color)
                .unwrap_or_else(|| auto_series_color(theme, i))
        };
        let mut view = ChartSeriesView {
            name: s.name.clone().unwrap_or_else(|| format!("Series{}", i + 1)),
            values,
            x_values: None,
            color,
            point_colors: None,
            shadow_filter: None,
            shadow: None,
            gradient: gradient(s.fill.as_ref()),
            point_gradients: None,
        };
        if let Some(effects) = &s.effects_xml {
            view.shadow = resolve_drawing_shadow_xml(effects, &scheme);
        }
        if let Some(point_fills) = &s.point_fills {
            let mut points = HashMap::new();
            for (k, fill) in point_fills {
                if let Some(g) = gradient(Some(fill)) {
                    points.insert(k.parse().unwrap_or(0), g);
                }
            }
            if !points.is_empty() {
                view.point_gradients = Some(points);
            }
        }
        series.push(view);
    }
    let longest = series.iter().map(|s| s.values.len()).max().unwrap_or(0);
    let first_cats = raw_categories
        .iter()
        .find(|c| !c.is_empty())
        .cloned()
        .unwrap_or_default();
    let len = longest.max(first_cats.len());
    categories = (0..len)
        .map(|i| {
            if let Some(c) = first_cats.get(i) {
                to_text(c)
            } else {
                (i + 1).to_string()
            }
        })
        .collect();
    if radial {
        for (si, s) in series.iter_mut().enumerate() {
            let src = &chart.series[si];
            s.point_colors = Some(
                s.values
                    .iter()
                    .enumerate()
                    .map(|(i, _)| {
                        let explicit = src
                            .point_colors
                            .as_ref()
                            .and_then(|m| m.get(&i.to_string()))
                            .and_then(|c| drawing_color(Some(c)));
                        explicit.unwrap_or_else(|| {
                            if let Some(p) = palette {
                                chart_palette_series_color(p, i, s.values.len(), &scheme)
                            } else {
                                auto_series_color(theme, i)
                            }
                        })
                    })
                    .collect(),
            );
        }
    } else {
        for (si, s) in series.iter_mut().enumerate() {
            if let Some(points) = &chart.series[si].point_colors {
                s.point_colors = Some(
                    s.values
                        .iter()
                        .enumerate()
                        .map(|(i, _)| {
                            drawing_color(points.get(&i.to_string()).as_ref().map(|x| x))
                                .unwrap_or_else(|| s.color.clone())
                        })
                        .collect(),
                );
            }
        }
    }
    let mut model = ChartViewModel {
        type_,
        grouping,
        title: chart.title.clone(),
        show_legend: chart.show_legend,
        legend_position: chart
            .legend_position
            .clone()
            .unwrap_or_else(|| "r".to_string()),
        categories,
        series,
        value_axis: None,
        x_axis: None,
        horizontal: type_ == ChartSummaryType::Bar,
        supported: supported_type(type_),
        appearance: chart_appearance(chart, theme),
        bar_gap_width: chart.bar_gap_width,
        bar_overlap: chart.bar_overlap,
        title_overlay: false,
        legend_overlay: false,
    };
    if type_ == ChartSummaryType::Scatter {
        for (i, s) in model.series.iter_mut().enumerate() {
            let cats = &raw_categories[i];
            let numeric = !cats.is_empty() && cats.iter().all(|c| c.is_number());
            s.x_values = Some(
                s.values
                    .iter()
                    .enumerate()
                    .map(|(k, _)| {
                        if numeric {
                            cats.get(k).and_then(|c| c.as_f64())
                        } else {
                            Some((k + 1) as f64)
                        }
                    })
                    .collect(),
            );
        }
        let xs: Vec<f64> = model
            .series
            .iter()
            .flat_map(|s| s.x_values.clone().unwrap_or_default())
            .flatten()
            .collect();
        model.x_axis = Some(if xs.is_empty() {
            nice_scale(0.0, 1.0, NiceScaleOptions::default())
        } else {
            nice_scale(
                xs.iter().copied().fold(f64::INFINITY, f64::min),
                xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                NiceScaleOptions {
                    no_zero: true,
                    max_intervals: None,
                },
            )
        });
    }
    if !radial {
        model.value_axis = Some(value_axis(&model));
    }
    let _ = chart_title_text(chart, theme);
    model
}

pub fn category_totals(model: &ChartViewModel) -> Vec<f64> {
    super::chart_value_axis::category_totals(model)
}
