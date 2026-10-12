// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/utils/text-body-properties-parser.ts
// and src/ui/src/pptx/render/text-body-layout.ts; changes in PROVENANCE.md.

use nitro_xml_opc::drawingml::{emu_to_pixels, parse_drawing_text_body, DrawingTextBody};
use nitro_xml_opc::xml::{ns, Element};

use crate::inheritance::{attr, child_local, children_local};

/// Default `a:bodyPr` text insets at 96 DPI (91440×45720 EMU), matching upstream constants.
pub const DEFAULT_BODY_INSET_LR_PX: f64 = 91440.0 / 9525.0;
pub const DEFAULT_BODY_INSET_TB_PX: f64 = 45720.0 / 9525.0;

/// Vertical anchor from `a:bodyPr/@anchor`, mapped like upstream `TextStyle.vAlign`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextBodyVAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

/// Layout parsed from `p:txBody` for painting.
#[derive(Debug, Clone, Default)]
pub struct TextBodyLayout {
    pub v_align: TextBodyVAlign,
    pub text_align: Option<TextAlignCss>,
    pub inset_top_px: f64,
    pub inset_bottom_px: f64,
    pub inset_left_px: f64,
    pub inset_right_px: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignCss {
    Left,
    Center,
    Right,
    Justify,
}

impl TextAlignCss {
    pub fn as_css(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::Justify => "justify",
        }
    }
}

/// Map `a:bodyPr/@anchor` to vertical alignment (upstream `parseBodyAnchor`).
pub fn parse_body_anchor(value: &str) -> Option<TextBodyVAlign> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }
    match normalized.as_str() {
        "t" | "top" => Some(TextBodyVAlign::Top),
        "ctr" | "center" => Some(TextBodyVAlign::Middle),
        "b" | "bottom" => Some(TextBodyVAlign::Bottom),
        "dist" | "just" => Some(TextBodyVAlign::Middle),
        _ => None,
    }
}

/// Map `a:pPr/@algn` to CSS `text-align` (upstream `resolveCssTextAlign` input tokens).
pub fn parse_paragraph_align(value: &str) -> Option<TextAlignCss> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }
    match normalized.as_str() {
        "l" | "left" => Some(TextAlignCss::Left),
        "ctr" | "center" => Some(TextAlignCss::Center),
        "r" | "right" => Some(TextAlignCss::Right),
        "just" | "justlow" | "dist" | "thaidist" => Some(TextAlignCss::Justify),
        _ => None,
    }
}

pub fn parse_text_body_layout(tx_body: &Element) -> TextBodyLayout {
    let parsed = parse_drawing_text_body(Some(tx_body));
    let v_align = parsed
        .as_ref()
        .and_then(|b| b.anchor.as_deref())
        .and_then(parse_body_anchor)
        .unwrap_or(TextBodyVAlign::Top);

    let mut inset_top = DEFAULT_BODY_INSET_TB_PX;
    let mut inset_bottom = DEFAULT_BODY_INSET_TB_PX;
    let mut inset_left = DEFAULT_BODY_INSET_LR_PX;
    let mut inset_right = DEFAULT_BODY_INSET_LR_PX;
    if let Some(insets) = parsed.as_ref().and_then(|b| b.insets_emu.as_ref()) {
        if let Some(v) = insets.top {
            inset_top = emu_to_pixels(v);
        }
        if let Some(v) = insets.bottom {
            inset_bottom = emu_to_pixels(v);
        }
        if let Some(v) = insets.left {
            inset_left = emu_to_pixels(v);
        }
        if let Some(v) = insets.right {
            inset_right = emu_to_pixels(v);
        }
    }

    let text_align = resolve_first_paragraph_align(tx_body, parsed.as_ref());

    TextBodyLayout {
        v_align,
        text_align,
        inset_top_px: inset_top,
        inset_bottom_px: inset_bottom,
        inset_left_px: inset_left,
        inset_right_px: inset_right,
    }
}

fn resolve_first_paragraph_align(
    tx_body: &Element,
    parsed: Option<&DrawingTextBody>,
) -> Option<TextAlignCss> {
    if let Some(body) = parsed {
        if let Some(p) = body.paragraphs.first() {
            if let Some(algn) = p.align.as_deref().and_then(parse_paragraph_align) {
                return Some(algn);
            }
        }
    }
    let paragraphs = children_local(tx_body, "p", ns::A);
    let first = paragraphs.first()?;
    let level = paragraph_level(first);
    let normalized_level = (level + 1).clamp(1, 9);
    let level_key = format!("lvl{normalized_level}pPr");
    let lst = child_local(tx_body, "lstStyle", ns::A);
    let candidates = [
        child_local(first, "pPr", ns::A),
        lst.and_then(|ls| child_local(ls, &level_key, ns::A)),
        lst.and_then(|ls| child_local(ls, "defPPr", ns::A)),
    ];
    for candidate in candidates.into_iter().flatten() {
        if let Some(algn) = attr(candidate, "algn").and_then(parse_paragraph_align) {
            return Some(algn);
        }
    }
    None
}

fn paragraph_level(p: &Element) -> i32 {
    let p_pr = child_local(p, "pPr", ns::A);
    let lvl = p_pr
        .and_then(|pp| attr(pp, "lvl"))
        .and_then(|s| s.parse().ok());
    lvl.unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use nitro_xml_opc::xml::parse_xml_str;

    use super::*;

    // Catches: menu buttons with anchor="ctr" and pPr algn="ctr" painting text in the corner.
    #[test]
    fn poi_style_button_text_layout() {
        let doc = parse_xml_str(
            r#"<p:txBody xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <a:bodyPr wrap="square" lIns="91440" tIns="45720" rIns="91440" bIns="45720" anchor="ctr"/>
              <a:p><a:pPr algn="ctr"/><a:r><a:t>Menu</a:t></a:r></a:p>
            </p:txBody>"#,
        )
        .unwrap();
        let layout = parse_text_body_layout(&doc.root);
        assert_eq!(layout.v_align, TextBodyVAlign::Middle);
        assert_eq!(layout.text_align, Some(TextAlignCss::Center));
        assert!((layout.inset_left_px - DEFAULT_BODY_INSET_LR_PX).abs() < 0.01);
    }
}
