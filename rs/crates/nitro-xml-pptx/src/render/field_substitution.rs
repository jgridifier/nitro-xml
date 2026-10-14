// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/text-field-substitution.ts
// and field-context.ts; changes in PROVENANCE.md.

use crate::presentation::Slide;

/// Context for substituting `a:fld` placeholder text at paint time.
#[derive(Debug, Clone, Default)]
pub struct FieldSubstitutionContext {
    pub slide_number: Option<u32>,
    pub slide_title: Option<String>,
    pub footer_text: Option<String>,
    pub header_text: Option<String>,
}

const TITLE_PLACEHOLDER_TYPES: &[&str] = &["title", "ctrtitle"];

/// Build substitution context for one slide (deck-wide hf fields can be wired later).
pub fn build_field_context(slide_index: usize, slide: &Slide) -> FieldSubstitutionContext {
    FieldSubstitutionContext {
        slide_number: Some((slide_index + 1) as u32),
        slide_title: derive_slide_title(slide),
        footer_text: None,
        header_text: None,
    }
}

fn derive_slide_title(slide: &Slide) -> Option<String> {
    for el in &slide.elements {
        let ph = el.placeholder_type.as_deref().map(str::to_ascii_lowercase);
        let is_title = ph
            .as_deref()
            .is_some_and(|t| TITLE_PLACEHOLDER_TYPES.contains(&t));
        if !is_title {
            continue;
        }
        let text = el.text.as_deref().unwrap_or("").trim();
        if !text.is_empty() {
            return Some(text.to_owned());
        }
    }
    None
}

/// Apply field substitution to a text segment when it has a `fieldType`.
pub fn substitute_field_text(
    segment_text: &str,
    field_type: Option<&str>,
    ctx: &FieldSubstitutionContext,
) -> String {
    let Some(field_type) = field_type else {
        return segment_text.to_owned();
    };
    let fl = field_type.to_ascii_lowercase();
    if fl == "slidenum" {
        if let Some(n) = ctx.slide_number {
            return n.to_string();
        }
    }
    if fl == "slidetitle" {
        if let Some(title) = &ctx.slide_title {
            return title.clone();
        }
    }
    if fl == "footer" {
        if let Some(t) = &ctx.footer_text {
            return t.clone();
        }
    }
    if fl == "header" {
        if let Some(t) = &ctx.header_text {
            return t.clone();
        }
    }
    segment_text.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: `a:fld` slidenum/slidetitle leaving authored placeholder glyphs on screen.
    #[test]
    fn substitutes_slide_number_and_title_fields() {
        let ctx = FieldSubstitutionContext {
            slide_number: Some(3),
            slide_title: Some("Alpha".to_owned()),
            footer_text: None,
            header_text: None,
        };
        assert_eq!(substitute_field_text("#", Some("slidenum"), &ctx), "3");
        assert_eq!(
            substitute_field_text("Title", Some("slidetitle"), &ctx),
            "Alpha"
        );
        assert_eq!(substitute_field_text("plain", None, &ctx), "plain");
    }
}
