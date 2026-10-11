// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/floats.ts; changes listed in PROVENANCE.md.

use super::input::{LayoutDocumentInput, LayoutFloat};
use super::result::{LayoutColumnBox, LayoutFloatBox, LayoutPageBox};

pub const FLOAT_WRAP_NOTE: &str =
    "Text wraps around floating pictures on their largest side (square, tight and through wrapping use the picture’s rectangle, not its outline); lines continuing onto a later page keep their wrapped widths.";

struct Span {
    start: f64,
    size: f64,
}

fn horizontal_area(float: &LayoutFloat, page: &LayoutPageBox, column: &LayoutColumnBox) -> Span {
    let content_left = page.margin_left_px;
    match float.relative_from_h.as_deref() {
        Some("page") => Span {
            start: 0.0,
            size: page.width_px,
        },
        Some("margin") => Span {
            start: content_left,
            size: page.width_px - content_left - page.margin_right_px,
        },
        Some("leftMargin") | Some("insideMargin") => Span {
            start: 0.0,
            size: content_left,
        },
        Some("rightMargin") | Some("outsideMargin") => Span {
            start: page.width_px - page.margin_right_px,
            size: page.margin_right_px,
        },
        _ => Span {
            start: content_left + column.x_px,
            size: column.width_px,
        },
    }
}

fn vertical_area(
    float: &LayoutFloat,
    page: &LayoutPageBox,
    paragraph_top_px: f64,
    paragraph_height_px: f64,
) -> Span {
    match float.relative_from_v.as_deref() {
        Some("page") => Span {
            start: 0.0,
            size: page.height_px,
        },
        Some("margin") => Span {
            start: page.margin_top_px,
            size: page.height_px - page.margin_top_px - page.margin_bottom_px,
        },
        Some("topMargin") | Some("insideMargin") => Span {
            start: 0.0,
            size: page.margin_top_px,
        },
        Some("bottomMargin") | Some("outsideMargin") => Span {
            start: page.height_px - page.margin_bottom_px,
            size: page.margin_bottom_px,
        },
        _ => Span {
            start: paragraph_top_px,
            size: paragraph_height_px,
        },
    }
}

fn place(area: Span, size: f64, align: Option<&str>, offset: Option<f64>) -> f64 {
    match align {
        Some("center") => area.start + (area.size - size) / 2.0,
        Some("right") | Some("bottom") | Some("outside") => area.start + area.size - size,
        Some(_) => area.start,
        None => area.start + offset.unwrap_or(0.0),
    }
}

pub fn float_position(
    float: &LayoutFloat,
    page: &LayoutPageBox,
    column: &LayoutColumnBox,
    paragraph: (f64, f64),
) -> (f64, f64) {
    let h = horizontal_area(float, page, column);
    let v = vertical_area(float, page, paragraph.0, paragraph.1);
    (
        place(
            h,
            float.width_px,
            float.align_h.as_deref(),
            float.offset_x_px,
        ),
        place(
            v,
            float.height_px,
            float.align_v.as_deref(),
            float.offset_y_px,
        ),
    )
}

pub fn position_floats(input: &LayoutDocumentInput, pages: &mut [LayoutPageBox]) -> bool {
    let mut floats_by_id = std::collections::HashMap::new();
    for section in &input.sections {
        for block in &section.blocks {
            if let Ok(paragraph) =
                serde_json::from_value::<super::input::LayoutParagraph>(block.clone())
            {
                if let Some(floats) = paragraph.floats {
                    floats_by_id.insert(paragraph.id, floats);
                }
            }
        }
    }
    if floats_by_id.is_empty() {
        return false;
    }
    let mut placed = std::collections::HashSet::new();
    for page in pages {
        let mut boxes = Vec::new();
        for column in &page.columns {
            for block in &column.blocks {
                let kind = block.get("kind").and_then(|v| v.as_str());
                let block_id = block.get("blockId").and_then(|v| v.as_str());
                if kind != Some("paragraph") || block_id.is_none() {
                    continue;
                }
                let block_id = block_id.unwrap();
                if placed.contains(block_id) {
                    continue;
                }
                let floats = floats_by_id.get(block_id);
                if floats.is_none() {
                    continue;
                }
                placed.insert(block_id);
                let y_px = block.get("yPx").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let height_px = block
                    .get("heightPx")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let top = page.margin_top_px + y_px;
                for float in floats.unwrap() {
                    let (x_px, y) = float_position(float, page, column, (top, height_px));
                    boxes.push(LayoutFloatBox {
                        block_id: block_id.to_string(),
                        x_px,
                        y_px: y,
                        width_px: float.width_px,
                        height_px: float.height_px,
                        part_name: float.part_name.clone(),
                        content_type: float.content_type.clone(),
                        behind_text: float.behind_text.unwrap_or(false),
                        wrap: float.wrap.clone(),
                    });
                }
            }
        }
        if !boxes.is_empty() {
            page.floats = Some(boxes);
        }
    }
    true
}
