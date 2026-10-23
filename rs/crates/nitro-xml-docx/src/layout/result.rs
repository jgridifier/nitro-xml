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
    pub highlight: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_caps: Option<bool>,
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

fn inject_chart_part_name_fragment(
    fragment: &LayoutFragment,
    fragment_json: &mut serde_json::Value,
) {
    let part_name = fragment
        .object
        .as_ref()
        .and_then(|object| object.chart.as_ref())
        .and_then(|chart| chart.part_name.as_deref())
        .filter(|name| !name.is_empty());
    let Some(part_name) = part_name else {
        return;
    };
    let Some(object_json) = fragment_json
        .get_mut("object")
        .and_then(|value| value.as_object_mut())
    else {
        return;
    };
    let Some(chart_json) = object_json
        .get_mut("chart")
        .and_then(|value| value.as_object_mut())
    else {
        return;
    };
    chart_json.insert(
        "partName".into(),
        serde_json::Value::String(part_name.to_string()),
    );
}

fn inject_chart_part_names_block_box(block: &LayoutBlockBox, value: &mut serde_json::Value) {
    match block {
        LayoutBlockBox::Paragraph(paragraph) => {
            let Some(lines_json) = value.get_mut("lines").and_then(|v| v.as_array_mut()) else {
                return;
            };
            for (line_index, line) in paragraph.lines.iter().enumerate() {
                let Some(line_json) = lines_json.get_mut(line_index) else {
                    continue;
                };
                let Some(fragments_json) = line_json
                    .get_mut("fragments")
                    .and_then(|v| v.as_array_mut())
                else {
                    continue;
                };
                for (fragment_index, fragment) in line.fragments.iter().enumerate() {
                    if let Some(fragment_json) = fragments_json.get_mut(fragment_index) {
                        inject_chart_part_name_fragment(fragment, fragment_json);
                    }
                }
            }
        }
        LayoutBlockBox::Table(table) => {
            let Some(rows_json) = value.get_mut("rows").and_then(|v| v.as_array_mut()) else {
                return;
            };
            for (row_index, row) in table.rows.iter().enumerate() {
                let Some(row_json) = rows_json.get_mut(row_index) else {
                    continue;
                };
                let Some(cells_json) = row_json.get_mut("cells").and_then(|v| v.as_array_mut())
                else {
                    continue;
                };
                for (cell_index, cell) in row.cells.iter().enumerate() {
                    let Some(cell_json) = cells_json.get_mut(cell_index) else {
                        continue;
                    };
                    let Some(paragraphs_json) = cell_json
                        .get_mut("paragraphs")
                        .and_then(|v| v.as_array_mut())
                    else {
                        continue;
                    };
                    for (paragraph_index, paragraph) in cell.iter().enumerate() {
                        let Some(paragraph_json) = paragraphs_json.get_mut(paragraph_index) else {
                            continue;
                        };
                        let Some(lines_json) = paragraph_json
                            .get_mut("lines")
                            .and_then(|v| v.as_array_mut())
                        else {
                            continue;
                        };
                        for (line_index, line) in paragraph.lines.iter().enumerate() {
                            let Some(line_json) = lines_json.get_mut(line_index) else {
                                continue;
                            };
                            let Some(fragments_json) = line_json
                                .get_mut("fragments")
                                .and_then(|v| v.as_array_mut())
                            else {
                                continue;
                            };
                            for (fragment_index, fragment) in line.fragments.iter().enumerate() {
                                if let Some(fragment_json) = fragments_json.get_mut(fragment_index)
                                {
                                    inject_chart_part_name_fragment(fragment, fragment_json);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn block_box_to_value(block: &LayoutBlockBox) -> serde_json::Value {
    let mut value = serde_json::to_value(block).expect("block box");
    inject_chart_part_names_block_box(block, &mut value);
    value
}

/// Remove print-only chart package part names before layout IR parity dumps.
pub fn strip_layout_ir_chart_part_names(result: &mut LayoutResult) {
    for page in &mut result.pages {
        for column in &mut page.columns {
            for block in &mut column.blocks {
                strip_chart_part_names_value(block);
            }
        }
    }
}

fn strip_chart_part_names_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(chart) = map.get_mut("chart") {
                if let Some(chart_map) = chart.as_object_mut() {
                    chart_map.remove("partName");
                }
            }
            for child in map.values_mut() {
                strip_chart_part_names_value(child);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                strip_chart_part_names_value(item);
            }
        }
        _ => {}
    }
}

pub fn push_block_box(column: &mut LayoutColumnBox, block: LayoutBlockBox) {
    column.blocks.push(block_box_to_value(&block));
}
