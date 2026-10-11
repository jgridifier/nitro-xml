// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/result.ts; changes listed in PROVENANCE.md.

use serde::{Deserialize, Serialize};

use super::input::{LayoutObject, LayoutParagraphBorders};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutFragment {
    pub text: String,
    pub x_px: f64,
    pub width_px: f64,
    pub run_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_start: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_end: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size_pt: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_scale_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ligatures: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character_spacing_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kerning_threshold_pt: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<LayoutObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub box_height_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leader: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutLine {
    pub y_px: f64,
    pub height_px: f64,
    pub fragments: Vec<LayoutFragment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap_before_px: Option<f64>,
    pub source_start: usize,
    pub source_end: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_token: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_token: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutParagraphFrame {
    pub left_px: f64,
    pub width_px: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub borders: Option<LayoutParagraphBorders>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shading: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutParagraphBox {
    pub kind: String,
    pub block_id: String,
    pub y_px: f64,
    pub height_px: f64,
    pub lines: Vec<LayoutLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<LayoutParagraphFrame>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutCellGeometry {
    pub x_px: f64,
    pub width_px: f64,
    pub padding_left_px: f64,
    pub padding_right_px: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub borders: Option<super::input::LayoutCellBorders>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shading: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_align: Option<String>,
    pub content_height_px: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutTableRowBox {
    pub y_px: f64,
    pub height_px: f64,
    pub repeated: bool,
    pub cells: Vec<Vec<LayoutParagraphBox>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<Vec<LayoutCellGeometry>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutTableBox {
    pub kind: String,
    pub block_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_px: Option<f64>,
    pub y_px: f64,
    pub height_px: f64,
    pub rows: Vec<LayoutTableRowBox>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LayoutBlockBox {
    #[serde(rename = "paragraph")]
    Paragraph(LayoutParagraphBox),
    #[serde(rename = "table")]
    Table(LayoutTableBox),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutColumnBox {
    pub x_px: f64,
    pub width_px: f64,
    pub blocks: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_y_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_y_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutFootnoteBox {
    pub id: String,
    pub y_px: f64,
    pub height_px: f64,
    pub paragraphs: Vec<LayoutParagraphBox>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutFloatBox {
    pub block_id: String,
    pub x_px: f64,
    pub y_px: f64,
    pub width_px: f64,
    pub height_px: f64,
    pub part_name: String,
    pub content_type: String,
    pub behind_text: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutPageBox {
    pub index: usize,
    pub section_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_indices: Option<Vec<usize>>,
    pub page_in_section: usize,
    pub width_px: f64,
    pub height_px: f64,
    pub margin_top_px: f64,
    pub margin_right_px: f64,
    pub margin_bottom_px: f64,
    pub margin_left_px: f64,
    pub columns: Vec<LayoutColumnBox>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_separator: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub floats: Option<Vec<LayoutFloatBox>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footnotes: Option<Vec<LayoutFootnoteBox>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutResult {
    pub pages: Vec<LayoutPageBox>,
    pub approximations: Vec<String>,
}

pub fn block_box_to_value(block: &LayoutBlockBox) -> serde_json::Value {
    serde_json::to_value(block).expect("block box")
}

pub fn push_block_box(column: &mut LayoutColumnBox, block: LayoutBlockBox) {
    column.blocks.push(block_box_to_value(&block));
}
