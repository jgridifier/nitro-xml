// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/summary.ts

use nitro_xml_opc::drawingml::DrawingColor;
use nitro_xml_opc::drawingml::DrawingFill;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChartSummaryType {
    Bar,
    #[default]
    Column,
    Line,
    Pie,
    Doughnut,
    Area,
    Scatter,
    Radar,
    Bubble,
    Stock,
    Surface,
}

impl ChartSummaryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Bar => "bar",
            Self::Column => "column",
            Self::Line => "line",
            Self::Pie => "pie",
            Self::Doughnut => "doughnut",
            Self::Area => "area",
            Self::Scatter => "scatter",
            Self::Radar => "radar",
            Self::Bubble => "bubble",
            Self::Stock => "stock",
            Self::Surface => "surface",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSummarySeries {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values_ref: Option<String>,
    pub categories: Vec<serde_json::Value>,
    pub values: Vec<Option<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drawing_color: Option<DrawingColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point_colors: Option<HashMap<String, DrawingColor>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<DrawingFill>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point_fills: Option<HashMap<String, DrawingFill>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects_xml: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ChartSummary {
    pub chart_type: ChartSummaryType,
    pub is3_d: bool,
    pub grouping: Option<String>,
    pub bar_gap_width: Option<i32>,
    pub bar_overlap: Option<i32>,
    pub title: Option<String>,
    /// `c:title` is present and `c:autoTitleDeleted` is not true, even when no title text is resolved.
    pub title_placeholder: bool,
    pub series: Vec<ChartSummarySeries>,
    pub show_legend: bool,
    pub legend_position: Option<String>,
    pub color_palette: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct ChartThemePalette {
    pub colors: Vec<String>,
    pub major_font: String,
    pub minor_font: String,
}
