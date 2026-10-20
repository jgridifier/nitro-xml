// SPDX-License-Identifier: MIT
//! UAX #9 visual order for a laid-out line.
//!
//! Upstream pagination does not reorder. [`reorder_line_fragments_uax9`] is applied
//! on the real-measure path and on [`super::session::LayoutSession`] visual pages,
//! not on the fake-measurer `layout_document` dump.

use std::ops::Range;

use serde_json::Value;
use unicode_bidi::BidiInfo;

use super::result::{LayoutFragment, LayoutResult};

/// Reorder `fragments` into visual order and recompute `x_px` from the line's left edge.
///
/// Fragments are treated as atomic runs. An already-visual line (typical LTR) keeps
/// its x positions, including justification.
pub fn reorder_line_fragments_uax9(fragments: &mut [LayoutFragment]) {
    if fragments.len() <= 1 {
        return;
    }
    let mut logical = String::new();
    let mut ranges: Vec<Range<usize>> = Vec::with_capacity(fragments.len());
    for fragment in fragments.iter() {
        let start = logical.len();
        logical.push_str(&fragment.text);
        ranges.push(start..logical.len());
    }
    if logical.is_empty() {
        return;
    }
    let info = BidiInfo::new(&logical, None);
    let mut visual_index = vec![usize::MAX; logical.len()];
    let mut seq = 0usize;
    for para in &info.paragraphs {
        let (levels, runs) = info.visual_runs(para, para.range.clone());
        for run in runs {
            let rtl = levels
                .get(run.start)
                .map(|level| level.number() % 2 == 1)
                .unwrap_or(false);
            let mut chars = char_ranges(&logical, run);
            if rtl {
                chars.reverse();
            }
            for ch in chars {
                for byte_idx in ch {
                    if byte_idx < visual_index.len() {
                        visual_index[byte_idx] = seq;
                    }
                }
                seq += 1;
            }
        }
    }
    let mut order: Vec<usize> = (0..fragments.len()).collect();
    order.sort_by_key(|&index| (leftmost_visual(&ranges[index], &visual_index), index));
    if order.iter().enumerate().all(|(slot, idx)| *idx == slot) {
        return;
    }
    let origin = fragments
        .iter()
        .map(|fragment| fragment.x_px)
        .fold(f64::INFINITY, f64::min);
    let mut x = if origin.is_finite() { origin } else { 0.0 };
    let original = fragments.to_vec();
    for (slot, index) in order.into_iter().enumerate() {
        let mut placed = original[index].clone();
        placed.x_px = x;
        x += placed.width_px;
        fragments[slot] = placed;
    }
}

fn char_ranges(text: &str, range: Range<usize>) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let end = range.end.min(text.len());
    let mut index = range.start.min(end);
    while index < end {
        let Some(ch) = text[index..end].chars().next() else {
            break;
        };
        let next = index + ch.len_utf8();
        out.push(index..next);
        index = next;
    }
    out
}

fn leftmost_visual(range: &Range<usize>, visual_index: &[usize]) -> usize {
    let mut best = usize::MAX;
    for index in range.clone() {
        if let Some(pos) = visual_index.get(index) {
            if *pos != usize::MAX {
                best = best.min(*pos);
            }
        }
    }
    best
}

/// Walk a layout result and reorder every line. Page breaks are unchanged.
pub fn apply_uax9_result(result: &mut LayoutResult) {
    for page in &mut result.pages {
        for column in &mut page.columns {
            for block in &mut column.blocks {
                walk_block(block);
            }
        }
        if let Some(notes) = &mut page.footnotes {
            for note in notes {
                for paragraph in &mut note.paragraphs {
                    for line in &mut paragraph.lines {
                        reorder_line_fragments_uax9(&mut line.fragments);
                    }
                }
            }
        }
    }
}

fn walk_block(value: &mut Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                walk_block(item);
            }
        }
        Value::Object(map) => {
            if map.contains_key("fragments") && map.contains_key("yPx") {
                if let Some(fragments) = map.get_mut("fragments") {
                    reorder_fragments_value(fragments);
                }
                return;
            }
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in keys {
                if let Some(child) = map.get_mut(&key) {
                    walk_block(child);
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn reorder_fragments_value(fragments: &mut Value) {
    let Ok(mut parsed) = serde_json::from_value::<Vec<LayoutFragment>>(fragments.clone()) else {
        return;
    };
    reorder_line_fragments_uax9(&mut parsed);
    if let Ok(encoded) = serde_json::to_value(&parsed) {
        *fragments = encoded;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frag(text: &str, x_px: f64, width_px: f64) -> LayoutFragment {
        LayoutFragment {
            text: text.to_string(),
            x_px,
            width_px,
            run_index: 0,
            source_start: None,
            source_end: None,
            bold: None,
            italic: None,
            font_family: None,
            font_size_pt: None,
            text_scale_percent: None,
            ligatures: None,
            character_spacing_px: None,
            kerning_threshold_pt: None,
            object: None,
            color: None,
            underline: None,
            strike: None,
            highlight: None,
            script: None,
            top_px: None,
            box_height_px: None,
            leader: None,
        }
    }

    #[test]
    fn rtl_hebrew_then_latin_digits_are_visual() {
        // Catches: RTL Hebrew followed by Latin digits stays in logical order instead of UAX #9 visual order.
        let mut fragments = vec![
            frag("אבג", 0.0, 30.0),
            frag(" ", 30.0, 6.0),
            frag("123", 36.0, 24.0),
        ];
        reorder_line_fragments_uax9(&mut fragments);
        let texts: Vec<&str> = fragments
            .iter()
            .map(|fragment| fragment.text.as_str())
            .collect();
        assert_eq!(texts, vec!["123", " ", "אבג"]);
        let x = |text: &str| {
            fragments
                .iter()
                .find(|fragment| fragment.text == text)
                .unwrap()
                .x_px
        };
        assert!(x("123") < x(" "));
        assert!(x(" ") < x("אבג"));
        assert!((x("123") - 0.0).abs() < 1e-9);
        assert!((x("אבג") - 30.0).abs() < 1e-9);
    }
}
