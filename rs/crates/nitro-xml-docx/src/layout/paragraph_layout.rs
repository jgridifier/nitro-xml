// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/paragraph-layout.ts; changes listed in PROVENANCE.md.

use std::collections::HashSet;

use super::input::LayoutParagraph;
use super::line_metrics::token_extent;
use super::measure::TextMeasurer;
use super::paragraph_fragments::{align_fragments, build_fragments};
use super::paragraph_tokens::{
    font_of, resolve_indents, run_text_width, tokenize_paragraph, PlacedToken,
};
use super::result::{LayoutLine, LayoutParagraphFrame};
use super::tab_stops::place_tab;
use super::text_breaks::BreakToken;
use super::units::{NO_TWIPS, twips_to_px};
use super::util::expect_defined;

#[derive(Clone)]
pub struct ParagraphLayoutResult {
    pub lines: Vec<LayoutLine>,
    pub spacing_before_px: f64,
    pub spacing_after_px: f64,
    pub content_height_px: f64,
    pub page_break_after_line: HashSet<usize>,
    pub column_break_after_line: HashSet<usize>,
    pub inset_top_px: f64,
    pub inset_bottom_px: f64,
    pub frame: Option<LayoutParagraphFrame>,
}

pub type LineBoxFn = dyn Fn(f64, f64) -> LineBoxInsets;

#[derive(Clone)]
pub struct LineBoxInsets {
    pub left_inset_px: f64,
    pub right_inset_px: f64,
    pub gap_before_px: f64,
}

pub fn layout_paragraph(
    paragraph: &LayoutParagraph,
    content_width_px: f64,
    measurer: &dyn TextMeasurer,
    note: &mut dyn FnMut(&str),
    line_box: Option<&LineBoxFn>,
    start_token: usize,
) -> ParagraphLayoutResult {
    let indents = resolve_indents(paragraph);
    let left_px = indents.left_px;
    let right_px = indents.right_px;
    let first_line_extra_px = if start_token > 0 {
        0.0
    } else {
        indents.first_line_extra_px
    };
    let body_width = (content_width_px - left_px - right_px).max(1.0);
    let fonts = paragraph.runs.iter().map(font_of).collect::<Vec<_>>();
    let tokenized = tokenize_paragraph(paragraph);
    let tokens = tokenized.tokens;
    let run_offsets = tokenized.run_offsets;
    let global_offset = |token: &BreakToken| -> usize {
        let run_index = token_run_index(token);
        expect_defined(run_offsets.get(run_index).copied(), "run offset index") + token_source_start(token)
    };
    let effective_align = paragraph
        .align
        .as_deref()
        .unwrap_or(if paragraph.direction.as_deref() == Some("rtl") {
            "right"
        } else {
            "left"
        });

    let mut lines: Vec<LayoutLine> = Vec::new();
    let mut page_break_after_line = HashSet::new();
    let mut column_break_after_line = HashSet::new();
    let mut width_overflow_noted = false;

    let mut placed: Vec<PlacedToken> = Vec::new();
    let mut line_width_px = 0.0;
    let mut token_cursor = 0usize;
    let mut first_token = start_token;

    let guess_height = measurer.line_height_of(&fonts[0]);
    let mut box_state = LineBoxState {
        boxes: Vec::new(),
        running_y: 0.0,
        body_width,
        first_line_extra_px,
        left_px,
        line_box,
        guess_height,
    };
    let token_width = |token: &BreakToken, _index: usize| -> f64 {
        match token {
            BreakToken::Word { text, run_index, .. } | BreakToken::Space { text, run_index, .. } => {
                run_text_width(
                    text,
                    expect_defined(paragraph.runs.get(*run_index), "token run"),
                    measurer,
                )
            }
            BreakToken::Object { run_index, .. } => paragraph
                .runs
                .get(*run_index)
                .and_then(|r| r.object.as_ref())
                .map(|o| o.width_px)
                .unwrap_or(0.0),
            _ => 0.0,
        }
    };
    let widths: Vec<f64> = tokens
        .iter()
        .enumerate()
        .map(|(i, t)| token_width(t, i))
        .collect();

    for (token_index, token) in tokens.iter().enumerate() {
        if token_index < start_token {
            continue;
        }
        token_cursor = token_index + 1;
        match token {
            BreakToken::LineBreak { .. } => {
                if placed.is_empty() {
                    first_token = token_index;
                }
                flush_paragraph_line(
                    &mut box_state,
                    &mut placed,
                    &mut lines,
                    &mut page_break_after_line,
                    &mut column_break_after_line,
                    paragraph,
                    &fonts,
                    measurer,
                    &global_offset,
                    token_cursor,
                    first_token,
                    true,
                    None,
                    &mut line_width_px,
                );
                continue;
            }
            BreakToken::PageBreak { .. } => {
                if placed.is_empty() {
                    first_token = token_index;
                }
                flush_paragraph_line(
                    &mut box_state,
                    &mut placed,
                    &mut lines,
                    &mut page_break_after_line,
                    &mut column_break_after_line,
                    paragraph,
                    &fonts,
                    measurer,
                    &global_offset,
                    token_cursor,
                    first_token,
                    true,
                    Some("page"),
                    &mut line_width_px,
                );
                continue;
            }
            BreakToken::ColumnBreak { .. } => {
                if placed.is_empty() {
                    first_token = token_index;
                }
                flush_paragraph_line(
                    &mut box_state,
                    &mut placed,
                    &mut lines,
                    &mut page_break_after_line,
                    &mut column_break_after_line,
                    paragraph,
                    &fonts,
                    measurer,
                    &global_offset,
                    token_cursor,
                    first_token,
                    true,
                    Some("column"),
                    &mut line_width_px,
                );
                continue;
            }
            _ => {}
        }
        let mut width = expect_defined(widths.get(token_index).copied(), "token width");
        let marker = paragraph
            .runs
            .get(token_run_index(token))
            .and_then(|r| r.marker.as_ref());
        let marker_factor = if marker.is_some() && matches!(token, BreakToken::Word { .. }) && token_index == 0 {
            match marker.unwrap().alignment.as_str() {
                "right" => 1.0,
                "center" => 0.5,
                _ => 0.0,
            }
        } else {
            0.0
        };
        let advance = width * (1.0 - marker_factor);
        let leader = if matches!(token, BreakToken::Tab { .. }) {
            let (following_px, before_decimal_px) = segment_after(
                token_index,
                &tokens,
                &widths,
                paragraph,
                measurer,
            );
            let tab = place_tab(
                box_state.line_start(lines.len()) + line_width_px,
                paragraph.tab_stops.as_deref().unwrap_or(&[]),
                if first_line_extra_px < 0.0 {
                    Some(left_px)
                } else {
                    None
                },
                following_px,
                before_decimal_px,
            );
            width = tab.width_px;
            tab.leader
        } else {
            None
        };
        let line_index = lines.len();
        let limit = box_state.available_width(line_index);
        let would_fit = line_width_px + if matches!(token, BreakToken::Tab { .. }) { width } else { advance } <= limit
            || placed.iter().all(|p| {
                paragraph
                    .runs
                    .get(p.token_run_index())
                    .map(|r| r.synthetic.unwrap_or(false))
                    .unwrap_or(false)
            });
        if !would_fit && !matches!(token, BreakToken::Space { .. }) {
            token_cursor = token_index;
            flush_paragraph_line(
                &mut box_state,
                &mut placed,
                &mut lines,
                &mut page_break_after_line,
                &mut column_break_after_line,
                paragraph,
                &fonts,
                measurer,
                &global_offset,
                token_cursor,
                first_token,
                false,
                None,
                &mut line_width_px,
            );
            token_cursor = token_index + 1;
            if width > box_state.available_width(lines.len()) && !width_overflow_noted {
                width_overflow_noted = true;
                note("A word wider than its line/column was not split mid-word; it overflows the line box.");
            }
        }
        if matches!(token, BreakToken::Space { .. }) && placed.is_empty() {
            continue;
        }
        if placed.is_empty() {
            first_token = token_index;
        }
        placed.push(PlacedToken {
            token: token.clone(),
            width_px: width,
            x_offset_px: if marker_factor > 0.0 {
                Some(-width * marker_factor)
            } else {
                None
            },
            advance_px: if marker_factor > 0.0 {
                Some(advance)
            } else {
                None
            },
            leader,
        });
        line_width_px += if matches!(token, BreakToken::Tab { .. }) {
            width
        } else {
            advance
        };
    }
    let force_final_line = lines.is_empty();
    flush_paragraph_line(
        &mut box_state,
        &mut placed,
        &mut lines,
        &mut page_break_after_line,
        &mut column_break_after_line,
        paragraph,
        &fonts,
        measurer,
        &global_offset,
        token_cursor,
        first_token,
        force_final_line,
        None,
        &mut line_width_px,
    );

    let last_index = lines.len().saturating_sub(1);
    for (index, line) in lines.iter_mut().enumerate() {
        let aligned = align_fragments(
            line.fragments.clone(),
            box_state.available_width(index),
            index == last_index,
            effective_align,
        );
        line.fragments = aligned
            .into_iter()
            .map(|f| LayoutFragment {
                x_px: f.x_px + box_state.line_start(index),
                ..f
            })
            .collect();
    }

    let borders = paragraph.borders.as_ref();
    let extent = |side: &str| -> f64 {
        borders
            .and_then(|b| match side {
                "top" => b.top.as_ref(),
                "bottom" => b.bottom.as_ref(),
                "left" => b.left.as_ref(),
                "right" => b.right.as_ref(),
                _ => None,
            })
            .map(|b| b.width_px + b.space_px)
            .unwrap_or(0.0)
    };
    let inset_top_px = extent("top");
    let inset_bottom_px = extent("bottom");
    let mut y = inset_top_px;
    for line in &mut lines {
        y += line.gap_before_px.unwrap_or(0.0);
        line.y_px = y;
        y += line.height_px;
    }
    let frame = if borders.is_some() || paragraph.shading.is_some() {
        Some(LayoutParagraphFrame {
            left_px: left_px - extent("left"),
            width_px: body_width + extent("left") + extent("right"),
            borders: borders.cloned(),
            shading: paragraph.shading.clone(),
        })
    } else {
        None
    };

    ParagraphLayoutResult {
        lines,
        spacing_before_px: twips_to_px(paragraph.spacing_before_twips.unwrap_or(NO_TWIPS)),
        spacing_after_px: twips_to_px(paragraph.spacing_after_twips.unwrap_or(NO_TWIPS)),
        content_height_px: y + inset_bottom_px,
        page_break_after_line,
        column_break_after_line,
        inset_top_px,
        inset_bottom_px,
        frame,
    }
}

use super::result::LayoutFragment;

fn token_run_index(token: &BreakToken) -> usize {
    match token {
        BreakToken::Word { run_index, .. }
        | BreakToken::Space { run_index, .. }
        | BreakToken::Tab { run_index, .. }
        | BreakToken::Object { run_index, .. }
        | BreakToken::LineBreak { run_index, .. }
        | BreakToken::PageBreak { run_index, .. }
        | BreakToken::ColumnBreak { run_index, .. } => *run_index,
    }
}

fn token_source_start(token: &BreakToken) -> usize {
    match token {
        BreakToken::Word { source_start, .. }
        | BreakToken::Space { source_start, .. }
        | BreakToken::Tab { source_start, .. }
        | BreakToken::Object { source_start, .. }
        | BreakToken::LineBreak { source_start, .. }
        | BreakToken::PageBreak { source_start, .. }
        | BreakToken::ColumnBreak { source_start, .. } => *source_start,
    }
}

struct LineBoxState<'a> {
    boxes: Vec<LineBoxInsets>,
    running_y: f64,
    body_width: f64,
    first_line_extra_px: f64,
    left_px: f64,
    line_box: Option<&'a LineBoxFn>,
    guess_height: f64,
}

impl<'a> LineBoxState<'a> {
    fn ensure_box(&mut self, line_index: usize) {
        while self.boxes.len() <= line_index {
            let mut gap = 0.0;
            let mut inset = self.line_box.map(|f| f(self.running_y, self.guess_height));
            if let Some(ref mut b) = inset {
                for _ in 0..8 {
                    if b.gap_before_px <= 0.0 {
                        break;
                    }
                    gap += b.gap_before_px;
                    if let Some(f) = self.line_box {
                        *b = f(self.running_y + gap, self.guess_height);
                    }
                }
            }
            let left_inset_px = inset.as_ref().map(|b| b.left_inset_px).unwrap_or(0.0);
            let right_inset_px = inset.as_ref().map(|b| b.right_inset_px).unwrap_or(0.0);
            self.boxes.push(LineBoxInsets {
                left_inset_px,
                right_inset_px,
                gap_before_px: gap,
            });
        }
    }

    fn box_at(&mut self, line_index: usize) -> LineBoxInsets {
        self.ensure_box(line_index);
        self.boxes[line_index].clone()
    }

    fn available_width(&mut self, line_index: usize) -> f64 {
        let inset = self.box_at(line_index);
        (self.body_width
            - if line_index == 0 {
                self.first_line_extra_px
            } else {
                0.0
            }
            - inset.left_inset_px
            - inset.right_inset_px)
            .max(1.0)
    }

    fn line_start(&mut self, line_index: usize) -> f64 {
        self.left_px
            + if line_index == 0 {
                self.first_line_extra_px
            } else {
                0.0
            }
            + self.box_at(line_index).left_inset_px
    }
}

fn flush_paragraph_line(
    box_state: &mut LineBoxState<'_>,
    placed: &mut Vec<PlacedToken>,
    lines: &mut Vec<LayoutLine>,
    page_break_after_line: &mut HashSet<usize>,
    column_break_after_line: &mut HashSet<usize>,
    paragraph: &LayoutParagraph,
    fonts: &[super::measure::LayoutFontSpec],
    measurer: &dyn TextMeasurer,
    global_offset: &dyn Fn(&BreakToken) -> usize,
    token_cursor: usize,
    first_token: usize,
    force_empty: bool,
    forced_by_break: Option<&str>,
    line_width_px: &mut f64,
) {
    if placed.is_empty() && !force_empty {
        return;
    }
    let line_index = lines.len();
    let mut rendered: Vec<PlacedToken> = placed.clone();
    while rendered.last().is_some_and(|p| matches!(p.token, BreakToken::Space { .. })) {
        rendered.pop();
    }
    let mut fragments = build_fragments(&rendered, paragraph);
    let start = placed
        .first()
        .map(|p| global_offset(&p.token))
        .unwrap_or(0);
    let end = if let Some(last) = placed.last() {
        let base = global_offset(&last.token);
        match &last.token {
            BreakToken::Word { text, .. } | BreakToken::Space { text, .. } => base + text.len(),
            _ => base + 1,
        }
    } else {
        start
    };
    let gap_before_px = box_state.box_at(line_index).gap_before_px;
    let metrics = super::line_metrics::line_metrics(placed, fonts, paragraph, measurer);
    for (index, placed_token) in rendered.iter().enumerate() {
        let extent = token_extent(&placed_token.token, paragraph, fonts, measurer);
        if let Some(fragment) = fragments.get_mut(index) {
            let run = paragraph.runs.get(placed_token.token_run_index());
            fragment.source_start = Some(global_offset(&placed_token.token));
            fragment.source_end = Some(
                fragment.source_start.unwrap()
                    + if run.map(|r| r.synthetic.unwrap_or(false)).unwrap_or(false) {
                        0
                    } else {
                        match &placed_token.token {
                            BreakToken::Word { text, .. } | BreakToken::Space { text, .. } => {
                                text.len()
                            }
                            _ => 1,
                        }
                    },
            );
            fragment.top_px = Some(metrics.baseline_px - extent.above);
            if !matches!(placed_token.token, BreakToken::Object { .. }) {
                fragment.box_height_px = Some(extent.ascent + extent.descent);
            }
        }
    }
    lines.push(LayoutLine {
        y_px: 0.0,
        height_px: metrics.height_px,
        baseline_px: Some(metrics.baseline_px),
        fragments,
        source_start: start,
        source_end: end,
        next_token: Some(token_cursor),
        first_token: Some(first_token),
        gap_before_px: if gap_before_px > 0.0 {
            Some(gap_before_px)
        } else {
            None
        },
    });
    box_state.running_y += gap_before_px + metrics.height_px;
    if forced_by_break == Some("page") {
        page_break_after_line.insert(line_index);
    }
    if forced_by_break == Some("column") {
        column_break_after_line.insert(line_index);
    }
    placed.clear();
    *line_width_px = 0.0;
}

fn segment_after(
    index: usize,
    tokens: &[BreakToken],
    widths: &[f64],
    paragraph: &LayoutParagraph,
    measurer: &dyn TextMeasurer,
) -> (f64, f64) {
    let _ends = ["tab", "lineBreak", "pageBreak", "columnBreak"];
    let mut following_px = 0.0;
    let mut before_decimal_px: Option<f64> = None;
    for next in (index + 1)..tokens.len() {
        let token = expect_defined(tokens.get(next), "token index");
        if match token {
            BreakToken::Tab { .. } => true,
            BreakToken::LineBreak { .. } => true,
            BreakToken::PageBreak { .. } => true,
            BreakToken::ColumnBreak { .. } => true,
            _ => false,
        } {
            break;
        }
        if before_decimal_px.is_none() {
            if let BreakToken::Word { text, run_index, .. } = token {
                if let Some(pos) = text.find('.').or_else(|| text.find(',')) {
                    let prefix = &text[..pos];
                    before_decimal_px = Some(
                        following_px
                            + run_text_width(
                                prefix,
                                expect_defined(paragraph.runs.get(*run_index), "decimal run"),
                                measurer,
                            ),
                    );
                }
            }
        }
        following_px += widths.get(next).copied().unwrap_or(0.0);
    }
    (following_px, before_decimal_px.unwrap_or(following_px))
}
