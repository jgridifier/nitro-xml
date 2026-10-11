// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/paragraph-fragments.ts; changes listed in PROVENANCE.md.

use super::input::LayoutParagraph;
use super::paragraph_tokens::PlacedToken;
use super::result::LayoutFragment;
use super::text_breaks::BreakToken;

pub fn build_fragments(
    tokens_on_line: &[PlacedToken],
    paragraph: &LayoutParagraph,
) -> Vec<LayoutFragment> {
    let mut fragments = Vec::new();
    let mut x = 0.0;
    for placed in tokens_on_line {
        let run = paragraph.runs.get(placed.token_run_index());
        let text = match &placed.token {
            BreakToken::Word { text, .. } | BreakToken::Space { text, .. } => text.clone(),
            _ => String::new(),
        };
        let fragment = LayoutFragment {
            text,
            x_px: x + placed.x_offset_px.unwrap_or(0.0),
            width_px: placed.width_px,
            run_index: placed.token_run_index(),
            source_start: None,
            source_end: None,
            bold: run.and_then(|r| r.bold),
            italic: run.and_then(|r| r.italic),
            font_family: run.and_then(|r| r.font_family.clone()),
            font_size_pt: run.and_then(|r| r.font_size_pt),
            text_scale_percent: run.and_then(|r| r.text_scale_percent),
            ligatures: run.and_then(|r| r.ligatures.clone()),
            character_spacing_px: run.and_then(|r| r.character_spacing_px),
            kerning_threshold_pt: run.and_then(|r| r.kerning_threshold_pt),
            object: if matches!(placed.token, BreakToken::Object { .. }) {
                run.and_then(|r| r.object.clone())
            } else {
                None
            },
            color: run.and_then(|r| r.color.clone()),
            underline: run.and_then(|r| r.underline),
            strike: run.and_then(|r| r.strike),
            script: run.and_then(|r| r.script.clone()),
            top_px: None,
            box_height_px: None,
            leader: placed.leader.clone(),
        };
        fragments.push(fragment);
        x += placed.advance_px.unwrap_or(placed.width_px);
    }
    fragments
}

impl PlacedToken {
    pub fn token_run_index(&self) -> usize {
        match &self.token {
            BreakToken::Word { run_index, .. }
            | BreakToken::Space { run_index, .. }
            | BreakToken::Tab { run_index, .. }
            | BreakToken::Object { run_index, .. }
            | BreakToken::LineBreak { run_index, .. }
            | BreakToken::PageBreak { run_index, .. }
            | BreakToken::ColumnBreak { run_index, .. } => *run_index,
        }
    }
}

fn justify(fragments: Vec<LayoutFragment>, width: f64) -> Vec<LayoutFragment> {
    let space_indices: Vec<usize> = fragments
        .iter()
        .enumerate()
        .filter_map(|(i, f)| if f.text == " " { Some(i) } else { None })
        .collect();
    if space_indices.is_empty() || fragments.is_empty() {
        return fragments;
    }
    let last = fragments.last().unwrap();
    let natural = last.x_px + last.width_px;
    let extra = (width - natural).max(0.0) / space_indices.len() as f64;
    if extra <= 0.0 {
        return fragments;
    }
    let mut shift = 0.0;
    fragments
        .into_iter()
        .enumerate()
        .map(|(index, fragment)| {
            let mut placed = fragment;
            placed.x_px += shift;
            if space_indices.contains(&index) {
                placed.width_px += extra;
                shift += extra;
            }
            placed
        })
        .collect()
}

pub fn align_fragments(
    fragments: Vec<LayoutFragment>,
    width: f64,
    is_last_line: bool,
    effective_align: &str,
) -> Vec<LayoutFragment> {
    if fragments.is_empty() {
        return fragments;
    }
    if effective_align == "justify" && !is_last_line {
        return justify(fragments, width);
    }
    let last = fragments.last().unwrap();
    let natural = last.x_px + last.width_px;
    let offset = match effective_align {
        "center" => (width - natural).max(0.0) / 2.0,
        "right" => (width - natural).max(0.0),
        _ => 0.0,
    };
    if offset == 0.0 {
        fragments
    } else {
        fragments
            .into_iter()
            .map(|f| LayoutFragment {
                x_px: f.x_px + offset,
                ..f
            })
            .collect()
    }
}
