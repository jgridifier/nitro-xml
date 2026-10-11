// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/page-cursor.ts; changes listed in PROVENANCE.md.
//
// Upstream keeps `activeColumns` as the same object references as `page.columns`
// (or a suffix of them for continuous sections). Rust cannot alias those mutably,
// so we store a `column_base` index into `pages[last].columns` and always mutate
// through the page.

use super::input::{LayoutColumns, LayoutPageGeometry};
use super::result::{
    push_block_box, LayoutBlockBox, LayoutColumnBox, LayoutFootnoteBox, LayoutPageBox,
};
use super::util::expect_defined;

pub const FOOTNOTE_SEPARATOR_PX: f64 = 12.0;

pub struct BalanceHint {
    pub page_index: usize,
    pub height_px: f64,
}

struct PendingFootnotes {
    block_id: String,
    notes: Vec<LayoutFootnoteBox>,
    height_px: f64,
}

pub struct PageCursor {
    pub pages: Vec<LayoutPageBox>,
    equal_column_width_px: f64,
    /// Index of `activeColumns[0]` within `pages[last].columns` (shared-ref semantics).
    column_base: usize,
    column_top_px: f64,
    geometry: LayoutPageGeometry,
    columns: LayoutColumns,
    column_index: usize,
    y_px: f64,
    section_index: usize,
    page_in_section: usize,
    reserved_px: f64,
    pending: Option<PendingFootnotes>,
    balance: Option<BalanceHint>,
}

impl PageCursor {
    pub fn new(
        pages: Vec<LayoutPageBox>,
        geometry: LayoutPageGeometry,
        columns: LayoutColumns,
        section_index: usize,
        continue_at: Option<f64>,
        balance: Option<BalanceHint>,
    ) -> Self {
        let content_width = geometry.width_px - geometry.margin_left_px - geometry.margin_right_px;
        let equal_column_width_px = ((content_width
            - columns.gap_px * (columns.count as f64 - 1.0))
            / columns.count as f64)
            .max(1.0);
        let mut cursor = Self {
            pages,
            equal_column_width_px,
            column_base: 0,
            column_top_px: 0.0,
            geometry,
            columns,
            column_index: 0,
            y_px: 0.0,
            section_index,
            page_in_section: 0,
            reserved_px: 0.0,
            pending: None,
            balance,
        };
        if continue_at.is_none() {
            cursor.push_page();
        } else {
            let continue_at = continue_at.unwrap();
            cursor.column_top_px = continue_at;
            cursor.y_px = continue_at;
            let offset = cursor.geometry.margin_left_px - cursor.page().margin_left_px;
            let mut band = cursor.column_boxes(offset);
            for column in &mut band {
                column.section_index = Some(section_index);
                column.start_y_px = Some(continue_at);
                column.separator = Some(cursor.columns.separator.unwrap_or(false));
            }
            let base = cursor.page().columns.len();
            cursor.page_mut().columns.extend(band);
            cursor.column_base = base;
            cursor.column_index = 0;
        }
        cursor
    }

    fn column_boxes(&self, offset_px: f64) -> Vec<LayoutColumnBox> {
        let mut column_boxes = Vec::new();
        let mut x_px = offset_px;
        for i in 0..self.columns.count {
            let explicit = self.columns.widths.as_ref().and_then(|w| w.get(i));
            let width_px = explicit
                .map(|e| e.width_px)
                .unwrap_or(self.equal_column_width_px)
                .max(1.0);
            column_boxes.push(LayoutColumnBox {
                x_px,
                width_px,
                blocks: Vec::new(),
                section_index: None,
                start_y_px: None,
                end_y_px: None,
                separator: None,
            });
            x_px += width_px + explicit.map(|e| e.gap_px).unwrap_or(self.columns.gap_px);
        }
        column_boxes
    }

    fn push_page(&mut self) {
        let column_boxes = self.column_boxes(0.0);
        self.column_base = 0;
        self.pages.push(LayoutPageBox {
            index: self.pages.len(),
            section_index: self.section_index,
            section_indices: None,
            page_in_section: self.page_in_section,
            width_px: self.geometry.width_px,
            height_px: self.geometry.height_px,
            margin_top_px: self.geometry.margin_top_px,
            margin_right_px: self.geometry.margin_right_px,
            margin_bottom_px: self.geometry.margin_bottom_px,
            margin_left_px: self.geometry.margin_left_px,
            columns: column_boxes,
            column_separator: if self.columns.separator.unwrap_or(false) {
                Some(true)
            } else {
                None
            },
            floats: None,
            footnotes: None,
        });
        self.page_in_section += 1;
        self.column_index = 0;
        self.column_top_px = 0.0;
        self.y_px = 0.0;
        self.reserved_px = 0.0;
    }

    pub fn page(&self) -> &LayoutPageBox {
        self.pages.last().expect("page")
    }

    fn page_mut(&mut self) -> &mut LayoutPageBox {
        self.pages.last_mut().expect("page")
    }

    fn absolute_column_index(&self) -> usize {
        self.column_base + self.column_index
    }

    pub fn column(&self) -> &LayoutColumnBox {
        expect_defined(
            self.page().columns.get(self.absolute_column_index()),
            "current page column",
        )
    }

    pub fn column_width_px(&self) -> f64 {
        self.column().width_px
    }

    pub fn column_content_fits(&self) -> bool {
        let limit = self.column_top_px + self.column_height_px();
        let page = self.page();
        page.columns[self.column_base..self.column_base + self.columns.count]
            .iter()
            .all(|column| {
                column.blocks.iter().all(|block| {
                    let y = block.get("yPx").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let h = block
                        .get("heightPx")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    y + h <= limit
                })
            })
    }

    pub fn end_balance(&mut self) {
        self.balance = None;
    }

    pub fn y(&self) -> f64 {
        self.y_px
    }

    pub fn used_column_height_px(&self) -> f64 {
        self.y_px - self.column_top_px
    }

    pub fn at_column_top(&self) -> bool {
        self.y_px == self.column_top_px
    }

    pub fn has_page_content(&self) -> bool {
        self.page().columns.iter().any(|c| !c.blocks.is_empty())
    }

    pub fn column_height_px(&self) -> f64 {
        let height = self.physical_column_height_px();
        if self
            .balance
            .as_ref()
            .is_some_and(|b| b.page_index == self.page().index)
        {
            height.min(self.balance.as_ref().unwrap().height_px)
        } else {
            height
        }
    }

    pub fn physical_column_height_px(&self) -> f64 {
        (self.page().height_px
            - self.page().margin_top_px
            - self.geometry.margin_bottom_px
            - self.column_top_px)
            .max(1.0)
    }

    pub fn remaining_height_px(&self) -> f64 {
        self.column_height_px()
            - (self.y_px - self.column_top_px)
            - self.reserved_px
            - self.pending_height_px()
    }

    pub fn finish_band(&mut self) -> f64 {
        let base = self.column_base;
        let count = self.columns.count;
        let column_top = self.column_top_px;
        let section_index = self.section_index;
        let separator = Some(self.columns.separator.unwrap_or(false));
        let y_px = self.y_px;
        let bottom = {
            let page = self.page();
            page.columns[base..base + count]
                .iter()
                .flat_map(|column| {
                    column.blocks.iter().map(|block| {
                        let y = block.get("yPx").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let h = block
                            .get("heightPx")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        y + h
                    })
                })
                .fold(y_px, f64::max)
        };
        let page = self.page_mut();
        for column in &mut page.columns[base..base + count] {
            column.section_index = Some(section_index);
            if column.start_y_px.is_none() {
                column.start_y_px = Some(column_top);
            }
            column.end_y_px = Some(bottom);
            column.separator = separator;
        }
        bottom
    }

    fn pending_height_px(&self) -> f64 {
        if self.pending.is_none() {
            return 0.0;
        }
        let pending = self.pending.as_ref().unwrap();
        pending.height_px
            + if self
                .page()
                .footnotes
                .as_ref()
                .map(|f| !f.is_empty())
                .unwrap_or(false)
            {
                0.0
            } else {
                FOOTNOTE_SEPARATOR_PX
            }
    }

    pub fn hold_footnotes(&mut self, block_id: String, notes: Vec<LayoutFootnoteBox>) {
        let height_px = notes.iter().map(|n| n.height_px).sum();
        self.pending = if notes.is_empty() {
            None
        } else {
            Some(PendingFootnotes {
                block_id,
                notes,
                height_px,
            })
        };
    }

    pub fn new_page(&mut self) {
        self.push_page();
    }

    pub fn new_column(&mut self) {
        if self.column_index + 1 < self.columns.count {
            self.column_index += 1;
            self.y_px = self.column_top_px;
        } else {
            self.push_page();
        }
    }

    pub fn place(&mut self, mut block_box: LayoutBlockBox, advance_px: f64) {
        let section_index = self.section_index;
        let y_px = self.y_px;
        let page_index = self.pages.len() - 1;
        let col = self.absolute_column_index();
        if self.pages[page_index].section_index != section_index {
            let base_section = self.pages[page_index].section_index;
            let indices = self.pages[page_index]
                .section_indices
                .get_or_insert_with(|| vec![base_section]);
            if !indices.contains(&section_index) {
                indices.push(section_index);
                self.page_in_section = 1;
            }
        }
        match &mut block_box {
            LayoutBlockBox::Paragraph(p) => p.y_px = y_px,
            LayoutBlockBox::Table(t) => t.y_px = y_px,
        }
        let block_id = match &block_box {
            LayoutBlockBox::Paragraph(p) => p.block_id.clone(),
            LayoutBlockBox::Table(t) => t.block_id.clone(),
        };
        push_block_box(&mut self.pages[page_index].columns[col], block_box);
        self.y_px += advance_px;
        let release_footnotes = self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.block_id == block_id);
        if release_footnotes {
            self.reserved_px += self.pending_height_px();
            let mut top = self.pages[page_index]
                .footnotes
                .as_ref()
                .map(|f| f.iter().map(|n| n.height_px).sum())
                .unwrap_or(0.0);
            let notes = self.pending.take().unwrap().notes;
            let footnotes = self.pages[page_index]
                .footnotes
                .get_or_insert_with(Vec::new);
            for note in notes {
                let height_px = note.height_px;
                footnotes.push(LayoutFootnoteBox { y_px: top, ..note });
                top += height_px;
            }
        }
    }
}
