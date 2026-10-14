// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/paragraph-trailing-space.ts; changes in PROVENANCE.md.

use crate::text_layout::{TextAlignCss, TextPaintRun};

fn aligns_trailing_edge(align: Option<TextAlignCss>, rtl: bool) -> bool {
    match align {
        Some(TextAlignCss::Center) | Some(TextAlignCss::Justify) => true,
        Some(TextAlignCss::Right) => !rtl,
        Some(TextAlignCss::Left) => rtl,
        None => false,
    }
}

/// Drop paragraph-final whitespace when alignment would let it push visible text off the margin.
pub fn trim_paragraph_trailing_runs(
    runs: &[TextPaintRun],
    align: Option<TextAlignCss>,
    rtl: Option<bool>,
) -> Vec<TextPaintRun> {
    if runs.is_empty() || !aligns_trailing_edge(align, rtl == Some(true)) {
        return runs.to_vec();
    }
    let mut out = runs.to_vec();
    let mut changed = false;
    while let Some(last) = out.last() {
        if last.equation_mathml.is_some()
            || last.ruby.is_some()
            || !last.text.ends_with(' ') && !last.text.ends_with('\u{3000}')
            || last.text.contains('\n')
            || (out.len() == 1 && last.text.trim().is_empty())
        {
            break;
        }
        let trimmed = last
            .text
            .trim_end_matches([' ', '\u{3000}'])
            .to_owned();
        changed = true;
        if !trimmed.is_empty() {
            let mut run = out.pop().unwrap();
            run.text = trimmed;
            out.push(run);
            break;
        }
        out.pop();
    }
    if changed {
        out
    } else {
        runs.to_vec()
    }
}
