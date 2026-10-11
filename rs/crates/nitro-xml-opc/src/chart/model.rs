// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/model.ts, model-series.ts, model-chrome.ts,
// manual-layout.ts; changes listed in PROVENANCE.md.

use crate::drawingml::{DrawingFill, DrawingLine, DrawingTextBody};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartParseIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartShapeProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<DrawingFill>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<DrawingLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects_xml: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_xml: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartTextBody {
    #[serde(flatten)]
    pub body: DrawingTextBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_xml: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartManualLayout {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartLayout {
    #[serde(flatten)]
    pub manual: ChartManualLayout,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_xml: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartLines {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp_pr: Option<ChartShapeProperties>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartNumberFormat {
    pub format_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_linked: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartCachePoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<i64>,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartDataCache {
    #[serde(rename = "type")]
    pub cache_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_code: Option<String>,
    pub points: Vec<ChartCachePoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartDataSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<ChartDataCache>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levels: Option<Vec<ChartDataCache>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level_point_count: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartText {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rich: Option<ChartTextBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<ChartDataSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartTitle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx: Option<ChartText>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<ChartLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_pr: Option<ChartTextBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_lst: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartDataLabelOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_legend_key: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_value: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_category_name: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_series_name: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_percent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_bubble_size: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_format: Option<ChartNumberFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_pr: Option<ChartTextBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_lst: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartDataLabel {
    #[serde(flatten)]
    pub options: ChartDataLabelOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx: Option<ChartText>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<ChartLayout>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartDataLabels {
    #[serde(flatten)]
    pub options: ChartDataLabelOptions,
    #[serde(default)]
    pub labels: Vec<ChartDataLabel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_leader_lines: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leader_lines: Option<ChartLines>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartMarker {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_lst: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartDataPoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invert_if_negative: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bubble3_d: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explosion: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker: Option<ChartMarker>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_lst: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSpaceSeries {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx: Option<ChartText>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker: Option<ChartMarker>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_labels: Option<ChartDataLabels>,
    pub data_points: Vec<ChartDataPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<ChartDataSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<ChartDataSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_values: Option<ChartDataSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_values: Option<ChartDataSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bubble_sizes: Option<ChartDataSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smooth: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invert_if_negative: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explosion: Option<f64>,
    #[serde(rename = "bubble3D", skip_serializing_if = "Option::is_none")]
    pub bubble3_d: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_lst: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartGroupKind {
    #[serde(rename = "bar")]
    Bar,
    #[serde(rename = "line")]
    Line,
    #[serde(rename = "area")]
    Area,
    #[serde(rename = "pie")]
    Pie,
    #[serde(rename = "doughnut")]
    Doughnut,
    #[serde(rename = "ofPie")]
    OfPie,
    #[serde(rename = "scatter")]
    Scatter,
    #[serde(rename = "radar")]
    Radar,
    #[serde(rename = "bubble")]
    Bubble,
    #[serde(rename = "stock")]
    Stock,
    #[serde(rename = "surface")]
    Surface,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartPlotGroup {
    pub kind: ChartGroupKind,
    pub element: String,
    #[serde(rename = "is3D")]
    pub is3_d: bool,
    pub series: Vec<ChartSpaceSeries>,
    pub axis_ids: Vec<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vary_colors: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_labels: Option<ChartDataLabels>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bar_direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grouping: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlap: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap_depth: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smooth: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop_lines: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hi_low_lines: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up_down_bars: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_lines: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop_lines_sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hi_low_lines_sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_lines_sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up_down_bars_gap_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up_bars: Option<ChartLines>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub down_bars: Option<ChartLines>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_slice_angle: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hole_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub of_pie_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scatter_style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radar_style: Option<String>,
    #[serde(rename = "bubble3D", skip_serializing_if = "Option::is_none")]
    pub bubble3_d: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bubble_scale: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_negative_bubbles: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_represents: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wireframe: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_lst: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartAxisScaling {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orientation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_base: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartAxis {
    pub kind: String,
    pub scaling: ChartAxisScaling,
    pub major_gridlines: bool,
    pub minor_gridlines: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub major_gridlines_sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minor_gridlines_sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cross_axis_id: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<ChartTitle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_format: Option<ChartNumberFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub major_tick_mark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minor_tick_mark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick_label_position: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crosses: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crosses_at: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cross_between: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub major_unit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minor_unit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_offset: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick_label_skip: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick_mark_skip: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_multi_level_labels: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_time_unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_units: Option<ChartDisplayUnits>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_pr: Option<ChartTextBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_lst: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSurface {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thickness: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture_options_xml: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_lst: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartDataTable {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_horizontal_border: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_vertical_border: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_outline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_keys: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_pr: Option<ChartTextBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_lst: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartDisplayUnitsLabel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<ChartLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx: Option<ChartText>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_pr: Option<ChartTextBody>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartDisplayUnits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub built_in_unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_unit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<ChartDisplayUnitsLabel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_lst: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartPlotArea {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<ChartLayout>,
    pub groups: Vec<ChartPlotGroup>,
    pub axes: Vec<ChartAxis>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_table: Option<ChartDataTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_lst: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartLegendEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_pr: Option<ChartTextBody>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartLegend {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay: Option<bool>,
    pub entries: Vec<ChartLegendEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<ChartLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_pr: Option<ChartTextBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_lst: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartView3D {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rot_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rot_y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right_angle_axes: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perspective: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceDeclaration {
    pub prefix: String,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSpace {
    pub plot_area: ChartPlotArea,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date1904: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rounded_corners: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c14_style: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<ChartTitle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_title_deleted: Option<bool>,
    #[serde(rename = "view3D", skip_serializing_if = "Option::is_none")]
    pub view3_d: Option<ChartView3D>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub floor: Option<ChartSurface>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side_wall: Option<ChartSurface>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub back_wall: Option<ChartSurface>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legend: Option<ChartLegend>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plot_visible_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_blanks_as: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_data_labels_over_max: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp_pr: Option<ChartShapeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_pr: Option<ChartTextBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_data_rel_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_data_auto_update: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub print_settings: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_shapes_rel_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart_ext_lst: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_lst: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace_declarations: Option<Vec<NamespaceDeclaration>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSpaceParseResult {
    pub chart_space: ChartSpace,
    pub issues: Vec<ChartParseIssue>,
}
