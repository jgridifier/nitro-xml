// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/line-metrics.ts; changes listed in PROVENANCE.md.

use super::input::LayoutParagraph;
use super::measure::{font_metrics, LayoutFontSpec, TextMeasurer};
use super::paragraph_tokens::{font_of, PlacedToken};
use super::text_breaks::BreakToken;
use super::units::{pt_to_px, twips_to_px, DEFAULT_FONT_SIZE_PT};

const SUPER_RAISE: f64 = 0.33;
const SUB_DROP: f64 = 0.14;

pub struct TokenExtent {
    pub above: f64,
    pub below: f64,
    pub ascent: f64,
    pub descent: f64,
}

pub fn token_extent(
    token: &BreakToken,
    paragraph: &LayoutParagraph,
    fonts: &[LayoutFontSpec],
    measurer: &dyn TextMeasurer,
) -> TokenExtent {
    let run_index = match token {
        BreakToken::Word { run_index, .. }
        | BreakToken::Space { run_index, .. }
        | BreakToken::Tab { run_index, .. }
        | BreakToken::Object { run_index, .. }
        | BreakToken::LineBreak { run_index, .. }
        | BreakToken::PageBreak { run_index, .. }
        | BreakToken::ColumnBreak { run_index, .. } => *run_index,
    };
    let run = paragraph.runs.get(run_index);
    if matches!(token, BreakToken::Object { .. }) {
        if let Some(obj) = run.and_then(|r| r.object.as_ref()) {
            return TokenExtent {
                above: obj.height_px,
                below: 0.0,
                ascent: obj.height_px,
                descent: 0.0,
            };
        }
    }
    let font = fonts.get(run_index).cloned().unwrap_or_else(|| {
        font_of(&LayoutRun {
            text: String::new(),
            hidden: None,
            source_length: None,
            bold: None,
            italic: None,
            font_family: None,
            font_size_pt: None,
            text_scale_percent: None,
            ligatures: None,
            character_spacing_px: None,
            kerning_threshold_pt: None,
            position_px: None,
            break_after: None,
            object: None,
            synthetic: None,
            marker: None,
            script: None,
            color: None,
            underline: None,
            strike: None,
        })
    });
    let (ascent, descent) = font_metrics(measurer, &font);
    let size = pt_to_px(
        run.and_then(|r| r.font_size_pt)
            .unwrap_or(DEFAULT_FONT_SIZE_PT),
    );
    let script_shift = match run.and_then(|r| r.script.as_deref()) {
        Some("super") => SUPER_RAISE * size,
        Some("sub") => -SUB_DROP * size,
        _ => 0.0,
    };
    let shift = script_shift + run.and_then(|r| r.position_px).unwrap_or(0.0);
    TokenExtent {
        above: ascent + shift,
        below: descent - shift,
        ascent,
        descent,
    }
}

use super::input::LayoutRun;

pub struct LineMetrics {
    pub height_px: f64,
    pub baseline_px: f64,
}

pub fn line_metrics(
    placed: &[PlacedToken],
    fonts: &[LayoutFontSpec],
    paragraph: &LayoutParagraph,
    measurer: &dyn TextMeasurer,
) -> LineMetrics {
    let fallback_space = BreakToken::Space {
        text: " ".to_string(),
        run_index: 0,
        source_start: 0,
    };
    let tokens: Vec<&BreakToken> = if placed.is_empty() {
        vec![&fallback_space]
    } else {
        placed.iter().map(|p| &p.token).collect()
    };
    let mut above = 0.0f64;
    let mut below = 0.0f64;
    for token in tokens {
        let extent = token_extent(token, paragraph, fonts, measurer);
        above = above.max(extent.above);
        below = below.max(extent.below);
    }
    let natural = above + below;
    let rule = paragraph.line_spacing_rule.as_deref().unwrap_or("auto");
    let spacing_twips = paragraph.line_spacing_twips;
    let height_px = match rule {
        "exact" => spacing_twips.map(twips_to_px).unwrap_or(natural),
        "atLeast" => natural.max(spacing_twips.map(twips_to_px).unwrap_or(0.0)),
        _ => natural * (spacing_twips.unwrap_or(240) as f64 / 240.0),
    };
    LineMetrics {
        height_px,
        baseline_px: height_px - below,
    }
}
