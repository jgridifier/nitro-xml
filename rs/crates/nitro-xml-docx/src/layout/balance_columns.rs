// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/balance-columns.ts; changes listed in PROVENANCE.md.

use super::input::LayoutBlock;
use super::result::LayoutPageBox;
use super::section_flow::SectionFlow;
use super::util::clone_pages;

pub fn can_balance_block(block: &LayoutBlock) -> bool {
    match block {
        LayoutBlock::Paragraph(paragraph) => {
            paragraph
                .floats
                .as_ref()
                .map(|f| f.is_empty())
                .unwrap_or(true)
                && !paragraph.runs.iter().any(|run| run.break_after.is_some())
        }
        LayoutBlock::Table(table) => {
            !table.rows.is_empty()
                && table.rows.iter().all(|row| {
                    row.cells.iter().all(|cell| {
                        cell.paragraphs.iter().all(|paragraph| {
                            paragraph
                                .footnotes
                                .as_ref()
                                .map(|f| f.is_empty())
                                .unwrap_or(true)
                                && can_balance_block(&LayoutBlock::Paragraph(paragraph.clone()))
                        })
                    })
                })
        }
    }
}

pub fn balance_columns(
    base_pages: Vec<LayoutPageBox>,
    page_index: usize,
    full_height_px: f64,
    run: impl Fn(Vec<LayoutPageBox>, f64) -> SectionFlow,
) -> (Vec<LayoutPageBox>, SectionFlow) {
    let attempt = |height_px: f64| {
        let pages = clone_pages(&base_pages);
        let flow = run(pages.clone(), height_px);
        (pages, flow)
    };
    let mut low = 0.0;
    let mut high = full_height_px;
    let mut best = attempt(high);
    for _ in 0..16 {
        if high - low <= 1.0 / 15.0 {
            break;
        }
        let middle = (low + high) / 2.0;
        let candidate = attempt(middle);
        if candidate.0.len() <= page_index + 1 && candidate.1.balance_valid {
            high = middle;
            best = candidate;
        } else {
            low = middle;
        }
    }
    best
}
