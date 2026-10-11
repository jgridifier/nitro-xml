// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/adapt-drop-cap.ts; changes listed in PROVENANCE.md.

use std::collections::HashSet;

use serde_json::Value;

use super::input::{LayoutBlock, LayoutParagraph};

pub fn fold_drop_caps(source: &[Value], blocks: &mut [LayoutBlock]) -> HashSet<usize> {
    let mut folded = HashSet::new();
    for (index, block) in source.iter().enumerate() {
        if block.get("type").and_then(|v| v.as_str()) != Some("paragraph") {
            continue;
        }
        if !block.get("dropCap").is_some() {
            continue;
        }
        if let (Some(LayoutBlock::Paragraph(cap)), Some(LayoutBlock::Paragraph(next))) =
            (blocks.get(index), blocks.get(index + 1))
        {
            let mut merged = next.clone();
            merged.runs = cap.runs.clone().into_iter().chain(merged.runs).collect();
            blocks[index + 1] = LayoutBlock::Paragraph(merged);
            folded.insert(index);
        }
    }
    folded
}
