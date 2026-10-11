// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/input.ts; changes listed in PROVENANCE.md.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutChart {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub svg: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutObject {
    pub part_name: String,
    pub content_type: String,
    pub width_px: f64,
    pub height_px: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart: Option<LayoutChart>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutFloat {
    pub part_name: String,
    pub content_type: String,
    pub width_px: f64,
    pub height_px: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative_from_h: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_h: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset_x_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative_from_v: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_v: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset_y_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behind_text: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListMarker {
    pub length: usize,
    pub alignment: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutRun {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_length: Option<usize>,
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
    pub position_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub break_after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<LayoutObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synthetic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker: Option<ListMarker>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
}

pub type TabLeader = String;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutTabStop {
    pub pos_px: f64,
    pub align: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leader: Option<TabLeader>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutBorder {
    pub width_px: f64,
    pub style: String,
    pub color: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutParagraphBorder {
    pub width_px: f64,
    pub style: String,
    pub color: String,
    pub space_px: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutParagraphBorders {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<LayoutParagraphBorder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom: Option<LayoutParagraphBorder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<LayoutParagraphBorder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<LayoutParagraphBorder>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutNote {
    pub id: String,
    pub paragraphs: Vec<LayoutParagraph>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutParagraph {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_table_section_break: Option<bool>,
    pub kind: String,
    pub id: String,
    pub runs: Vec<LayoutRun>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spacing_before_twips: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spacing_after_twips: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_spacing_twips: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_spacing_rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent_left_twips: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent_right_twips: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent_start_twips: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent_end_twips: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_line_twips: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hanging_twips: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contextual_spacing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_break_before: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_next: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_lines: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widow_control: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub floats: Option<Vec<LayoutFloat>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_stops: Option<Vec<LayoutTabStop>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footnotes: Option<Vec<LayoutNote>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub borders: Option<LayoutParagraphBorders>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shading: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutCellBorders {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<LayoutBorder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<LayoutBorder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom: Option<LayoutBorder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<LayoutBorder>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutCellPadding {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutTableCell {
    pub paragraphs: Vec<LayoutParagraph>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<LayoutCellPadding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub borders: Option<LayoutCellBorders>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shading: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_align: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutTableRow {
    pub cells: Vec<LayoutTableCell>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cant_split: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height_rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_header: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutTable {
    pub kind: String,
    pub id: String,
    pub rows: Vec<LayoutTableRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alignment: Option<String>,
}

#[derive(Clone, Debug)]
pub enum LayoutBlock {
    Paragraph(LayoutParagraph),
    Table(LayoutTable),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutPageGeometry {
    pub width_px: f64,
    pub height_px: f64,
    pub margin_top_px: f64,
    pub margin_right_px: f64,
    pub margin_bottom_px: f64,
    pub margin_left_px: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutColumnWidth {
    pub width_px: f64,
    pub gap_px: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutColumns {
    pub count: usize,
    pub gap_px: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widths: Option<Vec<LayoutColumnWidth>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSection {
    pub page: LayoutPageGeometry,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<LayoutColumns>,
    pub blocks: Vec<serde_json::Value>,
    #[serde(rename = "break", skip_serializing_if = "Option::is_none")]
    pub section_break: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_align: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutDocumentInput {
    pub sections: Vec<LayoutSectionSerde>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSectionSerde {
    pub page: LayoutPageGeometry,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<LayoutColumns>,
    pub blocks: Vec<serde_json::Value>,
    #[serde(rename = "break", skip_serializing_if = "Option::is_none")]
    pub section_break: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_align: Option<String>,
}

pub fn block_to_value(block: &LayoutBlock) -> serde_json::Value {
    match block {
        LayoutBlock::Paragraph(p) => serde_json::to_value(p).expect("paragraph"),
        LayoutBlock::Table(t) => serde_json::to_value(t).expect("table"),
    }
}

pub fn parse_block(value: &serde_json::Value) -> LayoutBlock {
    if value.get("kind").and_then(|v| v.as_str()) == Some("table") {
        LayoutBlock::Table(serde_json::from_value(value.clone()).expect("table block"))
    } else {
        LayoutBlock::Paragraph(serde_json::from_value(value.clone()).expect("paragraph block"))
    }
}
