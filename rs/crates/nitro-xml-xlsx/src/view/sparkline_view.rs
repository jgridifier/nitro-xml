// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/sparkline-view.ts; changes listed in PROVENANCE.md.

use crate::model::{js_num, Color, ThemePalette};
use crate::model::SparklineGroup;
use crate::view::colors::resolve_color;
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct SparklinePointView {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SparklineMarkerView {
    pub x: f64,
    pub y: f64,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SparklineColumnView {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SparklineAxisView {
    pub y: f64,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SparklineView {
    pub type_: String,
    pub color: String,
    pub line_weight_pt: f64,
    pub lines: Vec<Vec<SparklinePointView>>,
    pub markers: Vec<SparklineMarkerView>,
    pub columns: Vec<SparklineColumnView>,
    pub axis: Option<SparklineAxisView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SparklineData {
    pub values: Vec<Option<f64>>,
    pub dates: Option<Vec<Option<f64>>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SparklineScale {
    pub min: f64,
    pub max: f64,
}

const COLUMN_GAP: f64 = 0.2;

type ColorKey = &'static str;

const DEFAULTS: [(&str, &str); 8] = [
    ("colorSeries", "#376092"),
    ("colorNegative", "#D00000"),
    ("colorAxis", "#000000"),
    ("colorMarkers", "#D00000"),
    ("colorFirst", "#D00000"),
    ("colorLast", "#D00000"),
    ("colorHigh", "#D00000"),
    ("colorLow", "#D00000"),
];

fn extent(values: &[Option<f64>]) -> Option<SparklineScale> {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for v in values.iter().flatten() {
        if *v < min {
            min = *v;
        }
        if *v > max {
            max = *v;
        }
    }
    if min <= max {
        Some(SparklineScale { min, max })
    } else {
        None
    }
}

pub fn sparkline_values(group: &SparklineGroup, values: &[Option<f64>]) -> Vec<Option<f64>> {
    if group.display_empty_cells_as.as_str() == "zero" {
        values.iter().map(|v| v.or(Some(0.0))).collect()
    } else {
        values.to_vec()
    }
}

pub fn sparkline_scale(
    group: &SparklineGroup,
    all: &[SparklineData],
    index: usize,
) -> Option<SparklineScale> {
    let own_slice = all.get(index).map(|d| d.values.as_slice()).unwrap_or(&[]);
    let own = extent(&sparkline_values(group, own_slice));
    let needs_group = group.min_axis_type == "group" || group.max_axis_type == "group";
    let shared = if needs_group {
        let flat: Vec<Option<f64>> = all
            .iter()
            .flat_map(|data| sparkline_values(group, &data.values))
            .collect();
        extent(&flat)
    } else {
        None
    };
    let end = |type_: &str, manual: Option<f64>, key: &str| -> Option<f64> {
        if type_ == "custom" {
            return manual;
        }
        if type_ == "group" {
            return shared
                .as_ref()
                .map(|s| if key == "min" { s.min } else { s.max });
        }
        own.as_ref()
            .map(|s| if key == "min" { s.min } else { s.max })
    };
    let min = end(&group.min_axis_type, group.manual_min, "min");
    let max = end(&group.max_axis_type, group.manual_max, "max");
    match (min, max) {
        (Some(min), Some(max)) => Some(SparklineScale { min, max }),
        _ => None,
    }
}

fn positions(group: &SparklineGroup, data: &SparklineData, n: usize) -> Vec<Option<f64>> {
    if group.date_axis {
        if let Some(dates) = &data.dates {
            if dates.len() == n {
                if let Some(span) = extent(dates) {
                    let width = span.max - span.min;
                    return dates
                        .iter()
                        .map(|d| {
                            d.map(|d| {
                                if width != 0.0 {
                                    (d - span.min) / width
                                } else {
                                    0.5
                                }
                            })
                        })
                        .collect();
                }
            }
        }
    }
    if n == 1 {
        vec![Some(0.5)]
    } else {
        (0..n).map(|i| Some(i as f64 / (n as f64 - 1.0))).collect()
    }
}

struct PointFacts {
    high: f64,
    low: f64,
    first: usize,
    last: usize,
}

fn point_facts(values: &[Option<f64>]) -> PointFacts {
    let numbers: Vec<f64> = values.iter().filter_map(|v| *v).collect();
    let mut last = values.len();
    while last > 0 && values[last - 1].is_none() {
        last -= 1;
    }
    let last_idx = if last > 0 { last - 1 } else { 0 };
    PointFacts {
        high: numbers.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        low: numbers.iter().copied().fold(f64::INFINITY, f64::min),
        first: values.iter().position(|v| v.is_some()).unwrap_or(0),
        last: last_idx,
    }
}

fn group_color(group: &SparklineGroup, key: ColorKey) -> Option<&Color> {
    match key {
        "colorSeries" => group.color_series.as_ref(),
        "colorNegative" => group.color_negative.as_ref(),
        "colorAxis" => group.color_axis.as_ref(),
        "colorMarkers" => group.color_markers.as_ref(),
        "colorFirst" => group.color_first.as_ref(),
        "colorLast" => group.color_last.as_ref(),
        "colorHigh" => group.color_high.as_ref(),
        "colorLow" => group.color_low.as_ref(),
        _ => None,
    }
}

fn resolve_colors(
    group: &SparklineGroup,
    theme: &ThemePalette,
    indexed: &[&str],
) -> Vec<(ColorKey, String)> {
    DEFAULTS
        .iter()
        .map(|(key, default)| {
            let c = resolve_color(group_color(group, key), theme, Some(default), indexed)
                .unwrap_or_else(|| default.to_string());
            (*key, c)
        })
        .collect()
}

fn color_get(colors: &[(ColorKey, String)], key: ColorKey) -> String {
    colors
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| "#000000".to_string())
}

fn point_color(
    group: &SparklineGroup,
    v: f64,
    i: usize,
    facts: &PointFacts,
    colors: &[(ColorKey, String)],
    markers: bool,
) -> Option<String> {
    let mut color = if markers && group.markers {
        Some(color_get(colors, "colorMarkers"))
    } else {
        None
    };
    if group.negative && v < 0.0 {
        color = Some(color_get(colors, "colorNegative"));
    }
    if group.high && v == facts.high {
        color = Some(color_get(colors, "colorHigh"));
    }
    if group.low && v == facts.low {
        color = Some(color_get(colors, "colorLow"));
    }
    if group.first && i == facts.first {
        color = Some(color_get(colors, "colorFirst"));
    }
    if group.last && i == facts.last {
        color = Some(color_get(colors, "colorLast"));
    }
    color
}

pub fn sparkline_view(
    group: &SparklineGroup,
    data: &SparklineData,
    scale: Option<SparklineScale>,
    theme: &ThemePalette,
    indexed: &[&str],
) -> SparklineView {
    let colors = resolve_colors(group, theme, indexed);
    let view = SparklineView {
        type_: group.type_.to_string(),
        color: color_get(&colors, "colorSeries"),
        line_weight_pt: group.line_weight,
        lines: Vec::new(),
        markers: Vec::new(),
        columns: Vec::new(),
        axis: None,
    };
    let values = sparkline_values(group, &data.values);
    let n = values.len();
    if n == 0 || scale.is_none() {
        return view;
    }
    let scale = scale.unwrap();
    let facts = point_facts(&values);
    let xs = positions(group, data, n);
    let flip = |x: f64| if group.right_to_left { 1.0 - x } else { x };
    let range = scale.max - scale.min;
    let y_of = |v: f64| {
        if range != 0.0 {
            ((scale.max - v) / range).clamp(0.0, 1.0)
        } else {
            0.5
        }
    };

    let mut view = view;
    if group.type_.as_str() == "line" {
        let mut segment: Vec<SparklinePointView> = Vec::new();
        for (i, v) in values.iter().enumerate() {
            let x = xs.get(i).copied().flatten();
            if v.is_none() || x.is_none() {
                if group.display_empty_cells_as.as_str() == "gap" && !segment.is_empty() {
                    view.lines.push(segment);
                    segment = Vec::new();
                }
                continue;
            }
            let v = v.unwrap();
            let point = SparklinePointView {
                x: flip(x.unwrap()),
                y: y_of(v),
            };
            segment.push(point.clone());
            if let Some(color) = point_color(group, v, i, &facts, &colors, true) {
                view.markers.push(SparklineMarkerView {
                    x: point.x,
                    y: point.y,
                    color,
                });
            }
        }
        if !segment.is_empty() {
            view.lines.push(segment);
        }
    } else {
        let slot = 1.0 / n as f64;
        let w = slot * (1.0 - COLUMN_GAP);
        let base = y_of(scale.min.max(0.0).min(scale.max));
        for (i, v) in values.iter().enumerate() {
            let position = xs.get(i).copied().flatten();
            if v.is_none() || position.is_none() {
                continue;
            }
            let v = v.unwrap();
            if v == 0.0 {
                continue;
            }
            let (top, bottom) = if group.type_.as_str() == "stacked" {
                if v > 0.0 {
                    (0.0, 0.5)
                } else {
                    (0.5, 1.0)
                }
            } else if range == 0.0 {
                (0.0, 1.0)
            } else {
                let yv = y_of(v);
                (base.min(yv), base.max(yv))
            };
            let centre = flip(slot / 2.0 + position.unwrap() * (1.0 - slot));
            let color = point_color(group, v, i, &facts, &colors, false)
                .unwrap_or_else(|| view.color.clone());
            view.columns.push(SparklineColumnView {
                x: centre - w / 2.0,
                y: top,
                w,
                h: bottom - top,
                color,
            });
        }
    }
    if group.type_.as_str() == "stacked" {
        if group.display_x_axis {
            view.axis = Some(SparklineAxisView {
                y: 0.5,
                color: color_get(&colors, "colorAxis"),
            });
        }
    } else if group.display_x_axis && range != 0.0 && scale.min <= 0.0 && scale.max >= 0.0 {
        view.axis = Some(SparklineAxisView {
            y: y_of(0.0),
            color: color_get(&colors, "colorAxis"),
        });
    }
    view
}

impl SparklineView {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("type".into(), Value::String(self.type_.clone()));
        m.insert("color".into(), Value::String(self.color.clone()));
        m.insert("lineWeightPt".into(), js_num(self.line_weight_pt));
        m.insert(
            "lines".into(),
            Value::Array(
                self.lines
                    .iter()
                    .map(|seg| {
                        Value::Array(
                            seg.iter()
                                .map(|p| json!({ "x": js_num(p.x), "y": js_num(p.y) }))
                                .collect(),
                        )
                    })
                    .collect(),
            ),
        );
        m.insert(
            "markers".into(),
            Value::Array(
                self.markers
                    .iter()
                    .map(|p| {
                        json!({
                            "x": js_num(p.x),
                            "y": js_num(p.y),
                            "color": p.color,
                        })
                    })
                    .collect(),
            ),
        );
        m.insert(
            "columns".into(),
            Value::Array(
                self.columns
                    .iter()
                    .map(|c| {
                        json!({
                            "x": js_num(c.x),
                            "y": js_num(c.y),
                            "w": js_num(c.w),
                            "h": js_num(c.h),
                            "color": c.color,
                        })
                    })
                    .collect(),
            ),
        );
        if let Some(axis) = &self.axis {
            m.insert(
                "axis".into(),
                json!({ "y": js_num(axis.y), "color": axis.color }),
            );
        }
        Value::Object(m)
    }
}
