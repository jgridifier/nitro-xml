// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/utils/text-body-properties-parser.ts
// and src/ui/src/pptx/render/text-body-layout.ts; changes in PROVENANCE.md.

use nitro_xml_opc::drawingml::{emu_to_pixels, parse_drawing_text_body, DrawingTextBody};
use nitro_xml_opc::xml::{ns, Element};

use crate::inheritance::{attr, child_local, children_local, parse_optional_bool_attr};
use nitro_xml_opc::xml::Node;

/// `a:bodyPr` autofit mode (upstream `TextStyle.autoFitMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAutoFitMode {
    /// `a:spAutoFit` — resize the shape to fit text.
    Shrink,
    /// `a:normAutofit` — shrink text to fit the box.
    Normal,
    /// `a:noAutofit`.
    None,
}

/// Parse autofit mode from `p:txBody` / `a:bodyPr` children (upstream `parseBodyAutofit`).
pub fn parse_body_autofit(tx_body: &Element) -> Option<TextAutoFitMode> {
    let body_pr = child_local(tx_body, "bodyPr", ns::A)?;
    if child_local(body_pr, "spAutoFit", ns::A).is_some() {
        return Some(TextAutoFitMode::Shrink);
    }
    if child_local(body_pr, "normAutofit", ns::A).is_some() {
        return Some(TextAutoFitMode::Normal);
    }
    if child_local(body_pr, "noAutofit", ns::A).is_some() {
        return Some(TextAutoFitMode::None);
    }
    None
}

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

/// One styled run inside a painted paragraph (upstream `ParagraphRun` subset).
#[derive(Debug, Clone)]
pub struct TextPaintRun {
    pub text: String,
    pub rtl: Option<bool>,
}

/// Paragraph-level BiDi + alignment for painters (upstream `RenderParagraph` subset).
#[derive(Debug, Clone)]
pub struct TextPaintParagraph {
    pub runs: Vec<TextPaintRun>,
    pub align: Option<TextAlignCss>,
    pub rtl: Option<bool>,
}

/// Walk `p:txBody` paragraphs for multi-`p` / per-run BiDi painting (#27).
pub fn parse_text_paint_paragraphs(tx_body: &Element) -> Vec<TextPaintParagraph> {
    let paras = children_local(tx_body, "p", ns::A);
    paras
        .iter()
        .map(|p| {
            let p_pr = child_local(p, "pPr", ns::A);
            let align = p_pr
                .and_then(|pp| attr(pp, "algn"))
                .and_then(parse_paragraph_align);
            let rtl = p_pr.and_then(|pp| parse_optional_bool_attr(attr(pp, "rtl")));
            let runs = collect_paint_runs(p);
            TextPaintParagraph { runs, align, rtl }
        })
        .collect()
}

fn collect_paint_runs(p: &Element) -> Vec<TextPaintRun> {
    let mut runs = Vec::new();
    for child in &p.children {
        if let Node::Element(el) = child {
            if el.ns.as_deref() == Some(ns::A) && el.local == "r" {
                if let Some(text) = run_text_content(el) {
                    let r_pr = child_local(el, "rPr", ns::A);
                    let rtl = r_pr.and_then(|rp| parse_optional_bool_attr(attr(rp, "rtl")));
                    runs.push(TextPaintRun { text, rtl });
                }
            }
        }
    }
    if runs.is_empty() {
        let flat = p.text_content();
        if !flat.is_empty() {
            runs.push(TextPaintRun {
                text: flat,
                rtl: None,
            });
        }
    }
    runs
}

fn run_text_content(r: &Element) -> Option<String> {
    if let Some(t) = child_local(r, "t", ns::A) {
        let s = t.text_content();
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

/// Map OOXML alignment + RTL to CSS `text-align` (upstream `resolveCssTextAlign`).
pub fn resolve_css_text_align(align: Option<TextAlignCss>, is_rtl: bool) -> Option<TextAlignCss> {
    if align.is_some() {
        return align;
    }
    if is_rtl {
        Some(TextAlignCss::Right)
    } else {
        None
    }
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

    // Catches: keyframes title shapes with a:spAutoFit never queue ScrollHeight resize.
    #[test]
    fn sp_autofit_body_pr_parses_as_shrink() {
        let doc = parse_xml_str(
            r#"<p:txBody xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <a:bodyPr><a:spAutoFit/></a:bodyPr>
              <a:p><a:r><a:t>Title</a:t></a:r></a:p>
            </p:txBody>"#,
        )
        .unwrap();
        assert_eq!(parse_body_autofit(&doc.root), Some(TextAutoFitMode::Shrink));
    }

    // Catches: `a:pPr/@rtl` paragraphs painted as one LTR `<p>` so Arabic/Hebrew slides miss the gate.
    #[test]
    fn bidi_fixture_paragraph_rtl_and_runs() {
        let doc = parse_xml_str(
            r#"<p:txBody xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <a:bodyPr/>
              <a:p><a:pPr algn="r" rtl="1"/><a:r><a:rPr lang="ar-SA"/><a:t>مرحبا</a:t></a:r></a:p>
              <a:p><a:pPr algn="l"/><a:r><a:rPr lang="en-US"/><a:t>Hello </a:t></a:r>
              <a:r><a:rPr lang="he-IL"/><a:t>שלום</a:t></a:r></a:p>
            </p:txBody>"#,
        )
        .unwrap();
        let paras = parse_text_paint_paragraphs(&doc.root);
        assert_eq!(paras.len(), 2);
        assert_eq!(paras[0].rtl, Some(true));
        assert_eq!(paras[0].align, Some(TextAlignCss::Right));
        assert_eq!(paras[1].runs.len(), 2);
        assert_eq!(paras[1].rtl, None);
    }
}
