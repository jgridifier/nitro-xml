// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/flow-paragraph.ts; changes listed in PROVENANCE.md.

use super::input::LayoutParagraph;
use super::keep_rules::{adjust_for_widow_orphan, widow_control_enabled};
use super::measure::TextMeasurer;
use super::page_cursor::PageCursor;
use super::paragraph_layout::{layout_paragraph, ParagraphLayoutResult};
use super::result::{LayoutBlockBox, LayoutParagraphBox};
use super::util::expect_defined;

pub fn place_paragraph(
    cursor: &mut PageCursor,
    paragraph: &LayoutParagraph,
    mut layout: ParagraphLayoutResult,
    spacing_before_px: f64,
    required_together_px: f64,
    measurer: &dyn TextMeasurer,
    note: &mut dyn FnMut(&str),
) {
    let mut width = cursor.column_width_px();
    if paragraph.page_break_before.unwrap_or(false) && cursor.has_page_content() {
        cursor.new_page();
    }
    if required_together_px > 0.0
        && required_together_px <= cursor.column_height_px()
        && required_together_px > cursor.remaining_height_px()
        && !cursor.at_column_top()
    {
        cursor.new_column();
    }
    let widow = widow_control_enabled(paragraph.widow_control);
    let mut line_cursor = 0usize;
    let mut next_token = 0usize;
    let mut first_segment = true;
    if layout.lines.is_empty() {
        cursor.place(
            LayoutBlockBox::Paragraph(LayoutParagraphBox {
                kind: "paragraph".into(),
                block_id: paragraph.id.clone(),
                y_px: 0.0,
                height_px: 0.0,
                lines: Vec::new(),
                frame: None,
            }),
            spacing_before_px + layout.spacing_after_px,
        );
        return;
    }
    while line_cursor < layout.lines.len() {
        if cursor.column_width_px() != width {
            layout = layout_paragraph(paragraph, cursor.column_width_px(), measurer, note, None, next_token);
            line_cursor = 0;
            width = cursor.column_width_px();
        }
        let budget = cursor.remaining_height_px() - if first_segment { spacing_before_px } else { 0.0 };
        let mut cumulative = 0.0;
        let mut end = line_cursor;
        let mut forced: Option<&str> = None;
        for i in line_cursor..layout.lines.len() {
            let line = expect_defined(layout.lines.get(i), "paragraph line index");
            let gap = if i == line_cursor && !first_segment {
                0.0
            } else {
                line.gap_before_px.unwrap_or(0.0)
            };
            let height = line.height_px + gap;
            if cumulative + height > budget && (end > line_cursor || !cursor.at_column_top()) {
                break;
            }
            cumulative += height;
            end = i + 1;
            if layout.page_break_after_line.contains(&i) {
                forced = Some("page");
                break;
            }
            if layout.column_break_after_line.contains(&i) {
                forced = Some("column");
                break;
            }
        }
        let mut placed_count = end - line_cursor;
        if forced.is_none() && end < layout.lines.len() {
            placed_count = adjust_for_widow_orphan(
                layout.lines.len() - line_cursor,
                placed_count,
                widow,
            );
        }
        if placed_count == 0 {
            if !cursor.at_column_top() {
                cursor.new_column();
                continue;
            }
            note("A paragraph line was taller than an empty page/column and was placed without splitting.");
            placed_count = (end - line_cursor).max(1);
        }
        let first = expect_defined(layout.lines.get(line_cursor), "first line of paragraph segment");
        let origin = first.y_px - if first_segment {
            first.gap_before_px.unwrap_or(0.0) + layout.inset_top_px
        } else {
            0.0
        };
        let segment_lines: Vec<super::result::LayoutLine> = layout.lines[line_cursor..line_cursor + placed_count]
            .iter()
            .map(|line| {
                let mut l = line.clone();
                l.y_px -= origin;
                l
            })
            .collect();
        let is_final = line_cursor + placed_count >= layout.lines.len();
        let last_line = segment_lines.last();
        let is_last_segment = is_final;
        let segment_height = last_line.map(|l| l.y_px + l.height_px).unwrap_or(0.0)
            + if is_last_segment { layout.inset_bottom_px } else { 0.0 };
        let advance = (if first_segment { spacing_before_px } else { 0.0 })
            + segment_height
            + if is_final { layout.spacing_after_px } else { 0.0 };
        cursor.place(
            LayoutBlockBox::Paragraph(LayoutParagraphBox {
                kind: "paragraph".into(),
                block_id: paragraph.id.clone(),
                y_px: 0.0,
                height_px: segment_height,
                lines: segment_lines,
                frame: layout.frame.clone(),
            }),
            advance,
        );
        next_token = layout
            .lines
            .get(line_cursor + placed_count - 1)
            .and_then(|l| l.next_token)
            .unwrap_or(next_token);
        line_cursor += placed_count;
        first_segment = false;
        if forced == Some("page") {
            cursor.new_page();
        } else if forced == Some("column") {
            cursor.new_column();
        } else if line_cursor < layout.lines.len() {
            cursor.new_column();
        }
    }
}
