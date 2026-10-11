// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/page-flow.ts; changes listed in PROVENANCE.md.

use std::collections::{HashMap, HashSet};

use super::input::LayoutDocumentInput;
use super::measure::FakeMeasurer;
use super::result::{LayoutPageBox, LayoutResult};
use super::section_flow::SectionFlow;
use super::wrap::Exclusion;

pub fn layout_sections(
    input: &LayoutDocumentInput,
    measurer: &FakeMeasurer,
    exclusions: Option<HashMap<usize, Vec<Exclusion>>>,
) -> LayoutResult {
    let mut approximations = HashSet::new();
    let mut pages: Vec<LayoutPageBox> = Vec::new();
    let mut previous_flow: Option<SectionFlow> = None;
    for (index, section) in input.sections.iter().enumerate() {
        let continue_at = if section.section_break.as_deref() == Some("continuous") {
            previous_flow.as_mut().map(|f| f.cursor.finish_band())
        } else {
            None
        };
        let mut flow = SectionFlow::new(
            section.clone(),
            pages,
            measurer,
            index,
            exclusions.clone(),
            continue_at,
            None,
        );
        let mut note = |message: &str| {
            approximations.insert(message.to_string());
        };
        flow.run(measurer, &mut note);
        pages = flow.cursor.pages.clone();
        previous_flow = Some(flow);
    }
    LayoutResult {
        pages,
        approximations: approximations.into_iter().collect(),
    }
}
