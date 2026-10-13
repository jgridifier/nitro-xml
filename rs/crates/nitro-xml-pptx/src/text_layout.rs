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

/// Tab stop from `a:pPr/a:tabLst` (upstream `TabStopSpec` subset).
#[derive(Debug, Clone)]
pub struct TextPaintTabStop {
    pub position_px: f64,
    pub align: TextTabAlign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTabAlign {
    Left,
    Center,
    Right,
    Decimal,
}

/// Phonetic guide on a run (`a:ruby`), ready for `<ruby>` / `<rt>` painting.
#[derive(Debug, Clone)]
pub struct TextPaintRuby {
    pub annotation: String,
    pub font_size_px: Option<f64>,
    pub text_align: Option<&'static str>,
    pub lang: Option<String>,
}

/// One styled run inside a painted paragraph (upstream `ParagraphRun` subset).
#[derive(Debug, Clone)]
pub struct TextPaintRun {
    pub text: String,
    pub rtl: Option<bool>,
    pub underline: bool,
    pub ruby: Option<TextPaintRuby>,
}

/// Paragraph-level BiDi + alignment for painters (upstream `RenderParagraph` subset).
#[derive(Debug, Clone)]
pub struct TextPaintParagraph {
    pub runs: Vec<TextPaintRun>,
    pub align: Option<TextAlignCss>,
    pub rtl: Option<bool>,
    pub tab_stops: Vec<TextPaintTabStop>,
    /// Bullet glyph or formatted auto-number marker (paint only).
    pub bullet_marker: Option<String>,
    pub margin_left_px: Option<f64>,
    pub text_indent_px: Option<f64>,
}

/// Master/layout `txStyles` + `lstStyle` cascade for list paint (#27).
pub struct TextPaintListStyleCtx<'a> {
    pub own_tx_body: Option<&'a Element>,
    pub inherited_tx_body: Option<&'a Element>,
    pub is_body_placeholder: bool,
    pub effective_level_styles: Option<&'a crate::level_style::LevelStylesMap>,
    pub presentation_level_styles: Option<&'a crate::level_style::LevelStylesMap>,
}

/// Walk `p:txBody` paragraphs for multi-`p` / per-run BiDi painting (#27).
pub fn parse_text_paint_paragraphs(tx_body: &Element) -> Vec<TextPaintParagraph> {
    parse_text_paint_paragraphs_with_list_styles(tx_body, None)
}

/// Like [`parse_text_paint_paragraphs`] but resolves bullets and indents from merged txStyles.
pub fn parse_text_paint_paragraphs_with_list_styles(
    tx_body: &Element,
    list_styles: Option<&TextPaintListStyleCtx<'_>>,
) -> Vec<TextPaintParagraph> {
    use crate::auto_number::{
        break_auto_number_run, format_auto_number_bullet, next_auto_number, AutoNumberSequence,
    };
    use crate::text::{
        paragraph_has_renderable_content, resolve_paragraph_bullet_info,
        resolve_paragraph_indent_layout,
    };

    let paras = children_local(tx_body, "p", ns::A);
    let mut auto_seq = AutoNumberSequence::default();
    paras
        .iter()
        .map(|p| {
            let p_pr = child_local(p, "pPr", ns::A);
            let align = p_pr
                .and_then(|pp| attr(pp, "algn"))
                .and_then(parse_paragraph_align);
            let rtl = p_pr.and_then(|pp| parse_optional_bool_attr(attr(pp, "rtl")));
            let tab_stops = p_pr.map(parse_tab_stops).unwrap_or_default();
            let runs = collect_paint_runs(p);

            let (bullet_marker, margin_left_px, text_indent_px) = if let Some(ctx) = list_styles {
                let has_renderable = paragraph_has_renderable_content(p);
                let bullet = resolve_paragraph_bullet_info(
                    p,
                    ctx.own_tx_body,
                    ctx.inherited_tx_body,
                    ctx.is_body_placeholder,
                    ctx.effective_level_styles,
                    ctx.presentation_level_styles,
                );
                let auto_num_scheme = bullet.as_ref().and_then(|b| {
                    if !b.none && b.char.is_none() {
                        b.auto_num_type.clone()
                    } else {
                        None
                    }
                });
                let level = paragraph_level(p);
                let mut marker = None;
                if let Some(ref info) = bullet {
                    if !info.none {
                        let show = has_renderable || auto_num_scheme.is_none();
                        if show {
                            if let Some(ch) = &info.char {
                                marker = Some(ch.clone());
                            } else if let Some(scheme) = auto_num_scheme.as_ref() {
                                if has_renderable {
                                    let start = info.auto_num_start_at;
                                    let ord =
                                        next_auto_number(&mut auto_seq, level, &scheme, start);
                                    marker = Some(format_auto_number_bullet(&scheme, ord));
                                }
                            }
                        }
                    }
                }
                if auto_num_scheme.is_none() {
                    break_auto_number_run(&mut auto_seq, level);
                }
                let has_bullet = marker.is_some();
                let indent = resolve_paragraph_indent_layout(
                    p,
                    ctx.own_tx_body,
                    ctx.inherited_tx_body,
                    ctx.is_body_placeholder,
                    ctx.effective_level_styles,
                    ctx.presentation_level_styles,
                    has_bullet,
                );
                (marker, indent.margin_left_px, indent.text_indent_px)
            } else {
                (None, None, None)
            };

            TextPaintParagraph {
                runs,
                align,
                rtl,
                tab_stops,
                bullet_marker,
                margin_left_px,
                text_indent_px,
            }
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
                    let underline = r_pr.map(rpr_has_underline).unwrap_or(false);
                    let ruby = parse_run_ruby(el);
                    runs.push(TextPaintRun {
                        text,
                        rtl,
                        underline,
                        ruby,
                    });
                }
            }
        }
    }
    runs
}

fn run_text_content(r: &Element) -> Option<String> {
    if let Some(ruby) = child_local(r, "ruby", ns::A) {
        if let Some(text) = ruby_base_text(ruby) {
            return Some(text);
        }
    }
    if let Some(t) = child_local(r, "t", ns::A) {
        let s = t.text_content();
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

fn ruby_base_text(ruby: &Element) -> Option<String> {
    let base = child_local(ruby, "rubyBase", ns::A)?;
    let mut out = String::new();
    for child in &base.children {
        if let Node::Element(e) = child {
            if e.local == "r" && e.ns.as_deref() == Some(ns::A) {
                if let Some(t) = child_local(e, "t", ns::A) {
                    out.push_str(&t.text_content());
                }
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn ruby_rt_text(ruby: &Element) -> Option<String> {
    let rt = child_local(ruby, "rt", ns::A)?;
    let mut out = String::new();
    for child in &rt.children {
        if let Node::Element(e) = child {
            if e.local == "r" && e.ns.as_deref() == Some(ns::A) {
                if let Some(t) = child_local(e, "t", ns::A) {
                    out.push_str(&t.text_content());
                }
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn parse_run_ruby(r: &Element) -> Option<TextPaintRuby> {
    let ruby = child_local(r, "ruby", ns::A)?;
    let annotation = ruby_rt_text(ruby)?;
    let ruby_pr = child_local(ruby, "rubyPr", ns::A);
    let hps_px = ruby_pr
        .and_then(|rp| attr(rp, "hps"))
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|sz| *sz > 0.0)
        .map(|sz| sz / 100.0 * 96.0 / 72.0);
    let text_align = ruby_pr
        .and_then(|rp| attr(rp, "algn"))
        .map(ruby_pr_align_to_css);
    let lang = ruby_rt_lang(ruby);
    Some(TextPaintRuby {
        annotation,
        font_size_px: hps_px,
        text_align,
        lang,
    })
}

fn ruby_rt_lang(ruby: &Element) -> Option<String> {
    let rt = child_local(ruby, "rt", ns::A)?;
    for child in &rt.children {
        if let Node::Element(e) = child {
            if e.local == "r" && e.ns.as_deref() == Some(ns::A) {
                let r_pr = child_local(e, "rPr", ns::A);
                if let Some(lang) = r_pr.and_then(|rp| attr(rp, "lang")) {
                    return Some(lang.to_string());
                }
            }
        }
    }
    None
}

fn ruby_pr_align_to_css(algn: &str) -> &'static str {
    match algn {
        "l" => "left",
        "r" => "right",
        "dist" | "distCat" | "distLetter" => "justify",
        _ => "center",
    }
}

fn rpr_has_underline(rpr: &Element) -> bool {
    match attr(rpr, "u") {
        None | Some("none") => false,
        Some(_) => true,
    }
}

fn parse_tab_stops(p_pr: &Element) -> Vec<TextPaintTabStop> {
    let Some(tab_lst) = child_local(p_pr, "tabLst", ns::A) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for tab in children_local(tab_lst, "tab", ns::A) {
        let pos = attr(tab, "pos")
            .and_then(|s| s.parse::<i64>().ok())
            .filter(|p| *p > 0)
            .map(emu_to_pixels);
        if let Some(position_px) = pos {
            let align = match attr(tab, "algn") {
                Some("ctr") => TextTabAlign::Center,
                Some("r") => TextTabAlign::Right,
                Some("dec") => TextTabAlign::Decimal,
                _ => TextTabAlign::Left,
            };
            out.push(TextPaintTabStop { position_px, align });
        }
    }
    out.sort_by(|a, b| {
        a.position_px
            .partial_cmp(&b.position_px)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Lay out tab-separated segments (upstream `computeTabbedLayout` subset).
pub fn compute_tabbed_line_layout(
    segments: &[&str],
    tab_stops: &[TextPaintTabStop],
    mut measure: impl FnMut(&str) -> f64,
    default_tab_px: f64,
) -> Vec<(String, f64)> {
    const EPS: f64 = 0.01;
    const FALLBACK_STEP_PX: f64 = 48.0;
    let step = if default_tab_px > 0.0 {
        default_tab_px
    } else {
        FALLBACK_STEP_PX
    };
    let mut pieces: Vec<(String, f64)> = Vec::new();
    let mut cursor = 0.0;
    for (i, text) in segments.iter().enumerate() {
        if i == 0 {
            pieces.push((text.to_string(), 0.0));
            cursor = measure(text);
            continue;
        }
        let width = measure(text);
        let stop = tab_stops.iter().find(|s| s.position_px > cursor + EPS);
        let (tab_x, align) = if let Some(s) = stop {
            (s.position_px, s.align)
        } else {
            let tab_x = (cursor / step + 1.0 + EPS).floor() * step;
            (tab_x, TextTabAlign::Left)
        };
        let left = match align {
            TextTabAlign::Center => tab_x - width / 2.0,
            TextTabAlign::Right => tab_x - width,
            TextTabAlign::Decimal | TextTabAlign::Left => tab_x,
        };
        let left = left.max(cursor);
        let leader = left - cursor;
        pieces.push((text.to_string(), leader));
        cursor = left + width;
    }
    pieces
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

    // Catches: ruby runs flattened to `p.text_content()` so annotation duplicates base text in paint.
    #[test]
    fn underline_words_ruby_tab_fixture_paint_runs() {
        let doc = parse_xml_str(
            r#"<p:txBody xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <a:bodyPr wrap="none"><a:noAutofit/></a:bodyPr>
              <a:p><a:r><a:rPr lang="ja-JP" sz="2400" u="words"/><a:ruby>
                <a:rubyPr algn="ctr" hps="1200"/>
                <a:rt><a:r><a:rPr lang="ja-JP" sz="1200"/><a:t>あるふぁべと</a:t></a:r></a:rt>
                <a:rubyBase><a:r><a:rPr lang="en-US" sz="2400" u="words"/><a:t>ALFA BETO</a:t></a:r></a:rubyBase>
              </a:ruby></a:r></a:p>
              <a:p><a:pPr><a:tabLst><a:tab pos="2743200" algn="l"/></a:tabLst></a:pPr>
              <a:r><a:rPr lang="en-US" sz="2400" u="words"/><a:t>GAMA DELTO	EPSI</a:t></a:r></a:p>
            </p:txBody>"#,
        )
        .unwrap();
        let paras = parse_text_paint_paragraphs(&doc.root);
        assert_eq!(paras.len(), 2);
        assert_eq!(paras[0].runs.len(), 1);
        assert_eq!(paras[0].runs[0].text, "ALFA BETO");
        assert!(paras[0].runs[0].underline);
        assert_eq!(
            paras[0].runs[0]
                .ruby
                .as_ref()
                .map(|r| r.annotation.as_str()),
            Some("あるふぁべと")
        );
        assert_eq!(paras[1].tab_stops.len(), 1);
        assert!((paras[1].tab_stops[0].position_px - 288.0).abs() < 0.1);
        assert!(paras[1].runs[0].text.contains('\t'));
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
