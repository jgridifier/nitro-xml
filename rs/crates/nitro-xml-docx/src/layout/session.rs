// SPDX-License-Identifier: MIT
//! Lazy pagination session.
//!
//! [`LayoutSession::tick`] lays out a bounded number of pages. When it finishes,
//! the logical pages match [`super::layout::layout_document`] for the same measurer.
//! UAX #9 reordering is a visual post-pass (`visual_pages`) and does not change
//! the page count.

use std::collections::HashSet;

use super::bidi::apply_uax9_result;
use super::input::LayoutDocumentInput;
use super::layout::finalize_floats;
use super::measure::{create_fake_measurer, TextMeasurer};
use super::result::{LayoutPageBox, LayoutResult};
use super::section_flow::SectionFlow;

#[derive(Clone, Debug)]
pub struct TickStatus {
    pub pages_ready: usize,
    pub complete: bool,
    pub page_count_so_far: usize,
}

enum Phase {
    InSection { index: usize, flow: SectionFlow },
    Finalize,
    Done,
}

pub struct LayoutSession {
    input: LayoutDocumentInput,
    priority_start: usize,
    priority_end: usize,
    uax9: bool,
    phase: Phase,
    notes: HashSet<String>,
    pending_pages: Vec<LayoutPageBox>,
    result_pages: Vec<LayoutPageBox>,
    result_approximations: Vec<String>,
    visual_pages: Option<Vec<LayoutPageBox>>,
    complete: bool,
}

impl LayoutSession {
    pub fn new(input: LayoutDocumentInput) -> Self {
        let mut session = Self {
            input,
            priority_start: 0,
            priority_end: usize::MAX,
            uax9: true,
            phase: Phase::Done,
            notes: HashSet::new(),
            pending_pages: Vec::new(),
            result_pages: Vec::new(),
            result_approximations: Vec::new(),
            visual_pages: None,
            complete: false,
        };
        if session.input.sections.is_empty() {
            session.complete = true;
            session.phase = Phase::Done;
            return session;
        }
        let section = session.input.sections[0].clone();
        let flow = SectionFlow::new(
            section,
            Vec::new(),
            &create_fake_measurer(),
            0,
            None,
            None,
            None,
        );
        session.phase = Phase::InSection { index: 0, flow };
        session
    }

    /// Pages in `[start, end)` are the priority window. A tick may return once
    /// that window is committed, then later ticks continue through the document.
    pub fn set_priority(&mut self, start: usize, end: usize) {
        self.priority_start = start;
        self.priority_end = end;
    }

    /// When set, [`Self::visual_pages`] reorders committed lines with UAX #9.
    pub fn set_uax9(&mut self, enabled: bool) {
        self.uax9 = enabled;
        if !enabled {
            self.visual_pages = None;
        }
    }

    pub fn tick(&mut self, measurer: &dyn TextMeasurer, page_budget: usize) -> TickStatus {
        if self.complete {
            return self.status();
        }
        let priority_done_at_start = self.priority_satisfied();
        let start_committed = self.committed_count();
        let budget = page_budget.max(1);
        let mut steps = 0usize;
        loop {
            if self.complete || steps > 100_000 {
                break;
            }
            if !self.step_one(measurer) {
                break;
            }
            steps += 1;
            let gained = self.committed_count().saturating_sub(start_committed);
            if gained >= budget {
                break;
            }
            if !priority_done_at_start && self.priority_satisfied() {
                break;
            }
        }
        if self.complete {
            self.ensure_visual();
        }
        self.status()
    }

    /// Logical pages committed so far (no UAX #9). Grows until [`Self::result`] is available.
    pub fn committed_layout_result(&self) -> LayoutResult {
        LayoutResult {
            pages: self.committed_pages(),
            approximations: self.notes.iter().cloned().collect(),
        }
    }

    /// Logical pages. Matches eager [`super::layout::layout_document`] once complete.
    pub fn result(&self) -> Option<LayoutResult> {
        if !self.complete {
            return None;
        }
        Some(LayoutResult {
            pages: self.result_pages.clone(),
            approximations: self.result_approximations.clone(),
        })
    }

    /// Pages with UAX #9 visual fragment order, when the session was constructed
    /// with reordering enabled (the default).
    pub fn visual_pages(&self) -> Option<&[LayoutPageBox]> {
        self.visual_pages.as_deref()
    }

    fn ensure_visual(&mut self) {
        if !self.uax9 || self.visual_pages.is_some() {
            return;
        }
        let mut result = LayoutResult {
            pages: self.result_pages.clone(),
            approximations: Vec::new(),
        };
        apply_uax9_result(&mut result);
        self.visual_pages = Some(result.pages);
    }

    fn status(&self) -> TickStatus {
        TickStatus {
            pages_ready: self.pages_ready(),
            complete: self.complete,
            page_count_so_far: self.page_count_so_far(),
        }
    }

    fn pages_ready(&self) -> usize {
        let committed = self.committed_count();
        let start = self.priority_start;
        let end = self.priority_end;
        if end <= start {
            return 0;
        }
        committed.min(end).saturating_sub(start)
    }

    fn priority_satisfied(&self) -> bool {
        if self.priority_end <= self.priority_start {
            return true;
        }
        self.committed_count() >= self.priority_end
    }

    fn page_count_so_far(&self) -> usize {
        if self.complete {
            return self.result_pages.len();
        }
        match &self.phase {
            Phase::InSection { flow, .. } => flow.cursor.pages.len(),
            Phase::Finalize => self.pending_pages.len(),
            Phase::Done => self.result_pages.len(),
        }
    }

    fn committed_pages(&self) -> Vec<LayoutPageBox> {
        if self.complete {
            return self.result_pages.clone();
        }
        match &self.phase {
            Phase::InSection { flow, .. } => {
                let n = self.committed_count();
                flow.cursor.pages.iter().take(n).cloned().collect()
            }
            Phase::Finalize => self.pending_pages.clone(),
            Phase::Done => self.result_pages.clone(),
        }
    }

    fn committed_count(&self) -> usize {
        if self.complete {
            return self.result_pages.len();
        }
        match &self.phase {
            Phase::InSection { flow, .. } => {
                let len = flow.cursor.pages.len();
                if flow.is_finished() {
                    len
                } else {
                    len.saturating_sub(1)
                }
            }
            Phase::Finalize => self.pending_pages.len(),
            Phase::Done => self.result_pages.len(),
        }
    }

    fn step_one(&mut self, measurer: &dyn TextMeasurer) -> bool {
        let phase = std::mem::replace(&mut self.phase, Phase::Done);
        match phase {
            Phase::Done => false,
            Phase::Finalize => {
                let pages = std::mem::take(&mut self.pending_pages);
                let notes = std::mem::take(&mut self.notes);
                let result = LayoutResult {
                    pages,
                    approximations: notes.into_iter().collect(),
                };
                let result = finalize_floats(&self.input, measurer, result);
                self.result_pages = result.pages;
                self.result_approximations = result.approximations;
                self.complete = true;
                self.phase = Phase::Done;
                true
            }
            Phase::InSection { index, mut flow } => {
                if !flow.is_finished() {
                    let mut messages = Vec::new();
                    {
                        let mut note = |message: &str| messages.push(message.to_string());
                        flow.step(measurer, &mut note);
                    }
                    for message in messages {
                        self.notes.insert(message);
                    }
                    self.phase = Phase::InSection { index, flow };
                    return true;
                }
                if index + 1 < self.input.sections.len() {
                    let next_section = self.input.sections[index + 1].clone();
                    let continuous = next_section.section_break.as_deref() == Some("continuous");
                    let continue_at = if continuous {
                        Some(flow.cursor.finish_band())
                    } else {
                        None
                    };
                    let pages = flow.cursor.pages.clone();
                    let next = SectionFlow::new(
                        next_section,
                        pages,
                        measurer,
                        index + 1,
                        None,
                        continue_at,
                        None,
                    );
                    self.phase = Phase::InSection {
                        index: index + 1,
                        flow: next,
                    };
                    return true;
                }
                self.pending_pages = flow.cursor.pages;
                self.phase = Phase::Finalize;
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::input::{LayoutDocumentInput, LayoutPageGeometry, LayoutSectionSerde};
    use crate::layout::layout::layout_document;
    use crate::layout::measure::create_fake_measurer;
    use serde_json::json;

    fn sample_input() -> LayoutDocumentInput {
        let blocks = (0..4)
            .map(|index| {
                json!({
                    "kind": "paragraph",
                    "id": format!("p{index}"),
                    "pageBreakBefore": index > 0,
                    "widowControl": false,
                    "runs": [{ "text": format!("Paragraph {index} stays on its own page") }]
                })
            })
            .collect();
        LayoutDocumentInput {
            sections: vec![LayoutSectionSerde {
                page: LayoutPageGeometry {
                    width_px: 612.0,
                    height_px: 792.0,
                    margin_top_px: 72.0,
                    margin_right_px: 72.0,
                    margin_bottom_px: 72.0,
                    margin_left_px: 72.0,
                },
                columns: None,
                blocks,
                section_break: None,
                vertical_align: None,
            }],
        }
    }

    #[test]
    fn lazy_session_page_count_matches_eager() {
        // Catches: lazy session final page count differs from eager layout.
        let input = sample_input();
        let measurer = create_fake_measurer();
        let eager = layout_document(&input, &measurer);
        let mut session = LayoutSession::new(input);
        session.set_priority(0, 2);
        let first = session.tick(&measurer, 1);
        assert!(
            !first.complete,
            "page budget 1 must not finish a multi-page document"
        );
        assert!(first.page_count_so_far < eager.pages.len());
        let mut status = first;
        let mut guard = 0;
        while !status.complete {
            status = session.tick(&measurer, 1);
            guard += 1;
            assert!(guard < 32, "session did not finish");
        }
        assert_eq!(status.page_count_so_far, eager.pages.len());
        assert_eq!(
            session.result().map(|result| result.pages.len()),
            Some(eager.pages.len())
        );
        assert!(session.visual_pages().is_some());
    }
}
