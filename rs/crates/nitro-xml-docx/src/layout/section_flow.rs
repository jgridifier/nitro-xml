// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/section-flow.ts; changes listed in PROVENANCE.md.

use std::collections::HashMap;

use super::flow_paragraph::place_paragraph;
use super::flow_table::place_table;
use super::input::{parse_block, LayoutBlock, LayoutSectionSerde};
use super::keep_rules::{suppresses_spacing, ParagraphSpacingCtx};
use super::measure::TextMeasurer;
use super::page_cursor::{BalanceHint, PageCursor};
use super::paragraph_layout::{layout_paragraph, ParagraphLayoutResult};
use super::result::{LayoutFootnoteBox, LayoutPageBox};
use super::table_layout::{layout_row, stack_paragraphs};
use super::wrap::Exclusion;

pub struct SectionFlow {
    pub cursor: PageCursor,
    pub balance_valid: bool,
    pub balanced: bool,
    blocks: Vec<LayoutBlock>,
    paragraph_cache: HashMap<String, ParagraphLayoutResult>,
    first_row_height_cache: HashMap<String, f64>,
    block_index: usize,
}

impl SectionFlow {
    pub fn new(
        section: LayoutSectionSerde,
        pages: Vec<LayoutPageBox>,
        _measurer: &dyn TextMeasurer,
        section_index: usize,
        _exclusions: Option<HashMap<usize, Vec<Exclusion>>>,
        continue_at: Option<f64>,
        balance: Option<BalanceHint>,
    ) -> Self {
        let blocks = section.blocks.iter().map(parse_block).collect();
        Self {
            cursor: PageCursor::new(
                pages,
                section.page,
                section.columns.unwrap_or(super::input::LayoutColumns {
                    count: 1,
                    gap_px: 0.0,
                    separator: None,
                    widths: None,
                }),
                section_index,
                continue_at,
                balance,
            ),
            balance_valid: true,
            balanced: false,
            blocks,
            paragraph_cache: HashMap::new(),
            first_row_height_cache: HashMap::new(),
            block_index: 0,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.block_index >= self.blocks.len()
    }

    fn paragraph_layout(
        &mut self,
        index: usize,
        measurer: &dyn TextMeasurer,
    ) -> ParagraphLayoutResult {
        let key = format!("{}:{}", index, self.cursor.column_width_px());
        if let Some(cached) = self.paragraph_cache.get(&key) {
            return cached.clone();
        }
        let paragraph = match &self.blocks[index] {
            LayoutBlock::Paragraph(p) => p.clone(),
            _ => panic!("not paragraph"),
        };
        let mut noop = |_message: &str| {};
        let cached = layout_paragraph(
            &paragraph,
            self.cursor.column_width_px(),
            measurer,
            &mut noop,
            None,
            0,
        );
        self.paragraph_cache.insert(key, cached.clone());
        cached
    }

    fn first_row_height(&mut self, index: usize, measurer: &dyn TextMeasurer) -> f64 {
        let key = format!("{}:{}", index, self.cursor.column_width_px());
        if let Some(cached) = self.first_row_height_cache.get(&key) {
            return *cached;
        }
        let table = match &self.blocks[index] {
            LayoutBlock::Table(t) => t,
            _ => return 0.0,
        };
        let cached = table
            .rows
            .first()
            .map(|row| {
                let mut noop = |_message: &str| {};
                layout_row(
                    row,
                    self.cursor.column_width_px(),
                    measurer,
                    &mut noop,
                    None,
                )
                .height_px
            })
            .unwrap_or(0.0);
        self.first_row_height_cache.insert(key, cached);
        cached
    }

    fn spacing_before_for(&mut self, index: usize, measurer: &dyn TextMeasurer) -> f64 {
        if !matches!(self.blocks.get(index), Some(LayoutBlock::Paragraph(_))) {
            return 0.0;
        }
        let layout = self.paragraph_layout(index, measurer);
        let previous = index > 0;
        if previous {
            if let (Some(LayoutBlock::Paragraph(prev)), Some(LayoutBlock::Paragraph(cur))) =
                (self.blocks.get(index - 1), self.blocks.get(index))
            {
                if suppresses_spacing(
                    Some(&ParagraphSpacingCtx {
                        style_id: prev.style_id.clone(),
                        contextual_spacing: prev.contextual_spacing.unwrap_or(false),
                    }),
                    &ParagraphSpacingCtx {
                        style_id: cur.style_id.clone(),
                        contextual_spacing: cur.contextual_spacing.unwrap_or(false),
                    },
                ) {
                    return 0.0;
                }
            }
        }
        layout.spacing_before_px
    }

    fn required_keep_height(
        &mut self,
        index: usize,
        depth: usize,
        measurer: &dyn TextMeasurer,
    ) -> f64 {
        if index >= self.blocks.len() || depth > 64 {
            return 0.0;
        }
        match &self.blocks[index] {
            LayoutBlock::Table(_) => self.first_row_height(index, measurer),
            LayoutBlock::Paragraph(paragraph) => {
                let keep_lines = paragraph.keep_lines.unwrap_or(false);
                let keep_next = paragraph.keep_next.unwrap_or(false);
                let layout = self.paragraph_layout(index, measurer);
                let before = self.spacing_before_for(index, measurer);
                let own = before
                    + if keep_lines {
                        layout.content_height_px + layout.spacing_after_px
                    } else {
                        layout.lines.first().map(|l| l.height_px).unwrap_or(0.0)
                    };
                if keep_next {
                    own + self.required_keep_height(index + 1, depth + 1, measurer)
                } else {
                    own
                }
            }
        }
    }

    /// Lay out the next block. Returns whether a new page was started.
    pub fn step(&mut self, measurer: &dyn TextMeasurer, note: &mut dyn FnMut(&str)) -> bool {
        if self.is_finished() {
            return false;
        }
        let index = self.block_index;
        let pages_before = self.cursor.pages.len();
        match self.blocks[index].clone() {
            LayoutBlock::Table(table) => {
                place_table(&mut self.cursor, &table, measurer, note);
            }
            LayoutBlock::Paragraph(paragraph) => {
                if paragraph.after_table_section_break.unwrap_or(false) {
                    self.cursor.end_balance();
                }
                let spacing_before_px = self.spacing_before_for(index, measurer);
                let layout = self.paragraph_layout(index, measurer);
                if let Some(notes) = &paragraph.footnotes {
                    if !notes.is_empty() {
                        let footnote_boxes: Vec<LayoutFootnoteBox> = notes
                            .iter()
                            .map(|note| {
                                let mut noop = |_message: &str| {};
                                let stacked = stack_paragraphs(
                                    &note.paragraphs,
                                    self.cursor.column_width_px(),
                                    measurer,
                                    &mut noop,
                                    0.0,
                                    None,
                                );
                                LayoutFootnoteBox {
                                    id: note.id.clone(),
                                    y_px: 0.0,
                                    height_px: stacked.0,
                                    paragraphs: stacked.1,
                                }
                            })
                            .collect();
                        self.cursor
                            .hold_footnotes(paragraph.id.clone(), footnote_boxes);
                    }
                }
                let required_together_px = if paragraph.keep_next.unwrap_or(false) {
                    self.required_keep_height(index, 0, measurer)
                } else if paragraph.keep_lines.unwrap_or(false) {
                    spacing_before_px + layout.content_height_px + layout.spacing_after_px
                } else {
                    0.0
                };
                place_paragraph(
                    &mut self.cursor,
                    &paragraph,
                    layout,
                    spacing_before_px,
                    required_together_px,
                    measurer,
                    note,
                );
            }
        }
        self.block_index += 1;
        self.cursor.pages.len() > pages_before
    }

    pub fn run(&mut self, measurer: &dyn TextMeasurer, note: &mut dyn FnMut(&str)) {
        while !self.is_finished() {
            self.step(measurer, note);
        }
    }
}
