// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/table-layout.ts; changes listed in PROVENANCE.md.

use std::collections::HashMap;

use super::input::{LayoutParagraph, LayoutTableRow};
use super::measure::TextMeasurer;
use super::paragraph_layout::layout_paragraph;
use super::result::{LayoutCellGeometry, LayoutParagraphBox};

pub struct RowLayout {
    pub height_px: f64,
    pub cells: Vec<Vec<LayoutParagraphBox>>,
    pub geometry: Vec<LayoutCellGeometry>,
}

pub fn stack_paragraphs(
    paragraphs: &[LayoutParagraph],
    width_px: f64,
    measurer: &dyn TextMeasurer,
    note: &mut dyn FnMut(&str),
    top_px: f64,
    start_tokens: Option<&HashMap<String, usize>>,
) -> (f64, Vec<LayoutParagraphBox>) {
    let mut y = top_px;
    let mut boxes = Vec::new();
    for paragraph in paragraphs {
        let start = start_tokens
            .and_then(|m| m.get(&paragraph.id))
            .copied()
            .unwrap_or(0);
        let layout = layout_paragraph(paragraph, width_px, measurer, note, None, start);
        y += layout.spacing_before_px;
        let top = y;
        let lines = layout
            .lines
            .into_iter()
            .map(|line| {
                let mut l = line;
                l.y_px += top;
                l
            })
            .collect();
        // Upstream `stackParagraphs` omits `frame`: cell/footnote stacks draw cell shading
        // (and note chrome) instead of per-paragraph border/shading frames.
        boxes.push(LayoutParagraphBox {
            kind: "paragraph".into(),
            block_id: paragraph.id.clone(),
            y_px: top,
            height_px: layout.content_height_px,
            lines,
            frame: None,
        });
        y += layout.content_height_px + layout.spacing_after_px;
    }
    (y, boxes)
}

pub fn layout_row(
    row: &LayoutTableRow,
    table_width_px: f64,
    measurer: &dyn TextMeasurer,
    note: &mut dyn FnMut(&str),
    remaining: Option<&[Vec<LayoutParagraphBox>]>,
) -> RowLayout {
    let count = row.cells.len().max(1);
    let mut x = 0.0;
    let mut geometry = Vec::new();
    let laid_out: Vec<(f64, Vec<LayoutParagraphBox>)> = row
        .cells
        .iter()
        .enumerate()
        .map(|(cell_index, cell)| {
            let continuation = remaining.and_then(|r| r.get(cell_index));
            let starts: Option<HashMap<String, usize>> = continuation.map(|boxes| {
                boxes
                    .iter()
                    .map(|b| (b.block_id.clone(), b.lines.first().and_then(|l| l.first_token).unwrap_or(0)))
                    .collect()
            });
            let width_px = cell.width_px.unwrap_or(table_width_px / count as f64);
            let padding = cell.padding.clone().unwrap_or(super::input::LayoutCellPadding {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            });
            let paragraphs: Vec<LayoutParagraph> = if let Some(starts) = &starts {
                cell.paragraphs
                    .iter()
                    .filter(|p| starts.contains_key(&p.id))
                    .cloned()
                    .collect()
            } else {
                cell.paragraphs.clone()
            };
            let content = stack_paragraphs(
                &paragraphs,
                (width_px - padding.left - padding.right).max(1.0),
                measurer,
                note,
                padding.top,
                starts.as_ref(),
            );
            let height_px = content.0 + padding.bottom;
            geometry.push(LayoutCellGeometry {
                x_px: cell.x_px.unwrap_or(x),
                width_px,
                padding_left_px: padding.left,
                padding_right_px: padding.right,
                content_height_px: height_px,
                borders: cell.borders.clone(),
                shading: cell.shading.clone(),
                vertical_align: cell.vertical_align.clone(),
            });
            x = cell.x_px.unwrap_or(x) + width_px;
            (height_px, content.1)
        })
        .collect();
    let content = laid_out.iter().map(|(h, _)| *h).fold(0.0, f64::max);
    let height_px = match row.height_px {
        None => content,
        Some(h) if row.height_rule.as_deref() == Some("exact") => h,
        Some(h) => content.max(h),
    };
    RowLayout {
        height_px,
        cells: laid_out.into_iter().map(|(_, boxes)| boxes).collect(),
        geometry,
    }
}

struct CellSplit {
    before: Vec<LayoutParagraphBox>,
    after: Vec<LayoutParagraphBox>,
    before_height_px: f64,
}

fn split_cell_at_height(boxes: &[LayoutParagraphBox], cut_height_px: f64) -> CellSplit {
    let mut before = Vec::new();
    let mut after = Vec::new();
    let mut after_origin: Option<f64> = None;
    let mut before_height_px = 0.0;
    for block_box in boxes {
        if after_origin.is_some() {
            let origin = after_origin.unwrap();
            let mut b = block_box.clone();
            b.y_px -= origin;
            after.push(b);
            continue;
        }
        if block_box.y_px + block_box.height_px <= cut_height_px {
            before.push(block_box.clone());
            before_height_px = block_box.y_px + block_box.height_px;
            continue;
        }
        if block_box.y_px >= cut_height_px {
            after_origin = Some(block_box.y_px);
            let mut b = block_box.clone();
            b.y_px = 0.0;
            after.push(b);
            continue;
        }
        let before_lines: Vec<_> = block_box
            .lines
            .iter()
            .filter(|line| block_box.y_px + line.y_px + line.height_px <= cut_height_px)
            .cloned()
            .collect();
        let after_lines: Vec<_> = block_box.lines[before_lines.len()..].to_vec();
        if let Some(last) = before_lines.last() {
            let bottom = last.y_px + last.height_px;
            before.push(LayoutParagraphBox {
                height_px: bottom,
                lines: before_lines,
                ..block_box.clone()
            });
            before_height_px = block_box.y_px + bottom;
        }
        if let Some(first_after) = after_lines.first() {
            let shift = first_after.y_px;
            after_origin = Some(block_box.y_px + shift);
            let last = after_lines.last().unwrap();
            after.push(LayoutParagraphBox {
                y_px: 0.0,
                height_px: last.y_px + last.height_px - shift,
                lines: after_lines
                    .into_iter()
                    .map(|line| {
                        let mut l = line;
                        l.y_px -= shift;
                        l
                    })
                    .collect(),
                ..block_box.clone()
            });
        } else {
            after_origin = Some(block_box.y_px + block_box.height_px);
        }
    }
    CellSplit {
        before,
        after,
        before_height_px,
    }
}

pub struct RowSplit {
    pub before: RowLayout,
    pub after: Option<RowLayout>,
}

pub fn split_row_at_height(row: RowLayout, cut_height_px: f64) -> RowSplit {
    let splits: Vec<CellSplit> = row
        .cells
        .iter()
        .map(|cell| split_cell_at_height(cell, cut_height_px))
        .collect();
    let any_after = splits.iter().any(|s| !s.after.is_empty());
    let before = RowLayout {
        height_px: splits.iter().map(|s| s.before_height_px).fold(0.0, f64::max),
        cells: splits.iter().map(|s| s.before.clone()).collect(),
        geometry: row.geometry.clone(),
    };
    if !any_after {
        return RowSplit {
            before,
            after: None,
        };
    }
    let after = RowLayout {
        height_px: splits
            .iter()
            .map(|s| {
                s.after
                    .iter()
                    .map(|b| b.y_px + b.height_px)
                    .fold(0.0, f64::max)
            })
            .fold(0.0, f64::max),
        cells: splits.iter().map(|s| s.after.clone()).collect(),
        geometry: row.geometry.clone(),
    };
    RowSplit { before, after: Some(after) }
}
