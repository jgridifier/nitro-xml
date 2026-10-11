// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/paragraph-tokens.ts; changes listed in PROVENANCE.md.

use unicode_segmentation::UnicodeSegmentation;

use super::input::{LayoutParagraph, LayoutRun};
use super::measure::{LayoutFontSpec, TextMeasurer};
use super::text_breaks::{append_break_marker, tokenize_run, utf16_len, BreakToken};
use super::units::{pt_to_px, twips_to_px, DEFAULT_FONT_FAMILY, DEFAULT_FONT_SIZE_PT, NO_TWIPS};

pub const SCRIPT_SCALE: f64 = 0.65;

pub fn font_of(run: &LayoutRun) -> LayoutFontSpec {
    LayoutFontSpec {
        family: run
            .font_family
            .clone()
            .unwrap_or_else(|| DEFAULT_FONT_FAMILY.to_string()),
        size_px: pt_to_px(run.font_size_pt.unwrap_or(DEFAULT_FONT_SIZE_PT))
            * if run.script.is_some() {
                SCRIPT_SCALE
            } else {
                1.0
            },
        bold: run.bold.unwrap_or(false),
        italic: run.italic.unwrap_or(false),
        kerning: run.kerning_threshold_pt.map(|threshold| {
            if threshold > 0.0 && run.font_size_pt.unwrap_or(DEFAULT_FONT_SIZE_PT) >= threshold {
                "normal".to_string()
            } else {
                "none".to_string()
            }
        }),
        ligatures: run.ligatures.clone(),
        small_caps: false,
    }
}

pub fn run_text_width(text: &str, run: &LayoutRun, measurer: &dyn TextMeasurer) -> f64 {
    let grapheme_count = text.graphemes(true).count();
    (measurer.width_of(text, &font_of(run)) * run.text_scale_percent.unwrap_or(100.0) / 100.0
        + grapheme_count as f64 * run.character_spacing_px.unwrap_or(0.0))
    .max(0.0)
}

pub struct ResolvedIndents {
    pub left_px: f64,
    pub right_px: f64,
    pub first_line_extra_px: f64,
}

pub fn resolve_indents(paragraph: &LayoutParagraph) -> ResolvedIndents {
    let rtl = paragraph.direction.as_deref() == Some("rtl");
    let start_twips = paragraph
        .indent_start_twips
        .or_else(|| {
            if rtl {
                paragraph.indent_right_twips
            } else {
                paragraph.indent_left_twips
            }
        })
        .unwrap_or(NO_TWIPS);
    let end_twips = paragraph
        .indent_end_twips
        .or_else(|| {
            if rtl {
                paragraph.indent_left_twips
            } else {
                paragraph.indent_right_twips
            }
        })
        .unwrap_or(NO_TWIPS);
    let left_twips = if rtl { end_twips } else { start_twips };
    let right_twips = if rtl { start_twips } else { end_twips };
    let first_line_extra_px = if let Some(hanging) = paragraph.hanging_twips {
        -twips_to_px(hanging)
    } else {
        twips_to_px(paragraph.first_line_twips.unwrap_or(NO_TWIPS))
    };
    ResolvedIndents {
        left_px: twips_to_px(left_twips),
        right_px: twips_to_px(right_twips),
        first_line_extra_px,
    }
}

pub struct TokenizeParagraphResult {
    pub tokens: Vec<BreakToken>,
    pub run_offsets: Vec<usize>,
}

pub fn tokenize_paragraph(paragraph: &LayoutParagraph) -> TokenizeParagraphResult {
    let mut tokens = Vec::new();
    let mut run_offsets = Vec::new();
    let mut cursor = 0usize;
    for (run_index, run) in paragraph.runs.iter().enumerate() {
        run_offsets.push(cursor);
        if run.hidden.unwrap_or(false) {
            if !run.synthetic.unwrap_or(false) {
                cursor += run.source_length.unwrap_or_else(|| utf16_len(&run.text));
            }
            continue;
        }
        if run.object.is_some() {
            tokens.push(BreakToken::Object {
                run_index,
                source_start: 0,
            });
        } else if let Some(marker) = &run.marker {
            // marker.length is a character count from the adapter (JS string index).
            let marker_end = run
                .text
                .char_indices()
                .nth(marker.length)
                .map(|(i, _)| i)
                .unwrap_or(run.text.len());
            tokens.push(BreakToken::Word {
                text: run.text[..marker_end].to_string(),
                run_index,
                source_start: 0,
            });
            tokens.extend(tokenize_run(&run.text[marker_end..], run_index));
        } else {
            tokens.extend(tokenize_run(&run.text, run_index));
        }
        if let Some(break_after) = &run.break_after {
            append_break_marker(&mut tokens, run_index, utf16_len(&run.text), break_after);
        }
        if run.synthetic.unwrap_or(false) {
            for token in &mut tokens {
                if token_run_index(token) == run_index {
                    set_token_source_start(token, 0);
                }
            }
        } else {
            cursor += run.source_length.unwrap_or_else(|| utf16_len(&run.text));
        }
    }
    TokenizeParagraphResult {
        tokens,
        run_offsets,
    }
}

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

fn set_token_source_start(token: &mut BreakToken, start: usize) {
    match token {
        BreakToken::Word { source_start, .. }
        | BreakToken::Space { source_start, .. }
        | BreakToken::Tab { source_start, .. }
        | BreakToken::Object { source_start, .. }
        | BreakToken::LineBreak { source_start, .. }
        | BreakToken::PageBreak { source_start, .. }
        | BreakToken::ColumnBreak { source_start, .. } => *source_start = start,
    }
}

#[derive(Clone)]
pub struct PlacedToken {
    pub token: BreakToken,
    pub width_px: f64,
    pub x_offset_px: Option<f64>,
    pub advance_px: Option<f64>,
    pub leader: Option<String>,
}
