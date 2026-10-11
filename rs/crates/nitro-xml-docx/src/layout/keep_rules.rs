// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/keep-rules.ts; changes listed in PROVENANCE.md.

pub fn adjust_for_widow_orphan(
    total_lines: usize,
    fitting_lines: usize,
    widow_control: bool,
) -> usize {
    if !widow_control {
        return fitting_lines;
    }
    if fitting_lines == 0 || fitting_lines >= total_lines {
        return fitting_lines;
    }
    let remaining = total_lines - fitting_lines;
    if fitting_lines == 1 {
        return 0;
    }
    if remaining == 1 {
        return fitting_lines - 1;
    }
    fitting_lines
}

pub fn suppresses_spacing(
    previous: Option<&ParagraphSpacingCtx>,
    current: &ParagraphSpacingCtx,
) -> bool {
    // Upstream: no previous, or previous has no styleId → never suppress (None ≠ None).
    let previous = match previous {
        Some(p) => p,
        None => return false,
    };
    let Some(prev_style) = previous.style_id.as_deref() else {
        return false;
    };
    if Some(prev_style) != current.style_id.as_deref() {
        return false;
    }
    previous.contextual_spacing || current.contextual_spacing
}

pub struct ParagraphSpacingCtx {
    pub style_id: Option<String>,
    pub contextual_spacing: bool,
}

pub fn widow_control_enabled(value: Option<bool>) -> bool {
    value != Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_none_style_id_does_not_suppress_contextual_spacing() {
        // Catches: treating missing styleId as equal (None == None) collapsed
        // spacingBefore between unstyled contextualSpacing paragraphs (poi-bug65649).
        let prev = ParagraphSpacingCtx {
            style_id: None,
            contextual_spacing: true,
        };
        let cur = ParagraphSpacingCtx {
            style_id: None,
            contextual_spacing: true,
        };
        assert!(!suppresses_spacing(Some(&prev), &cur));
        let prev2 = ParagraphSpacingCtx {
            style_id: Some("Normal".into()),
            contextual_spacing: true,
        };
        let cur2 = ParagraphSpacingCtx {
            style_id: Some("Normal".into()),
            contextual_spacing: true,
        };
        assert!(suppresses_spacing(Some(&prev2), &cur2));
    }
}
