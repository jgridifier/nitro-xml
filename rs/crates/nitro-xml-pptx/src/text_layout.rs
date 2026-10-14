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

/// Own `a:bodyPr` autofit, else inherited placeholder `txBody` (layout ← master).
pub fn parse_body_autofit_cascade(
    own_tx_body: &Element,
    inherited_tx_body: Option<&Element>,
) -> Option<TextAutoFitMode> {
    parse_body_autofit(own_tx_body).or_else(|| inherited_tx_body.and_then(parse_body_autofit))
}

fn effective_body_pr<'a>(
    own_tx_body: &'a Element,
    inherited_tx_body: Option<&'a Element>,
) -> Option<&'a Element> {
    child_local(own_tx_body, "bodyPr", ns::A)
        .or_else(|| inherited_tx_body.and_then(|tx| child_local(tx, "bodyPr", ns::A)))
}

/// `a:normAutofit/@fontScale` when PowerPoint stored a shrink percentage (1.0 = 100%).
pub fn norm_autofit_font_scale_multiplier(
    own_tx_body: &Element,
    inherited_tx_body: Option<&Element>,
) -> Option<f64> {
    let body_pr = effective_body_pr(own_tx_body, inherited_tx_body)?;
    let norm = child_local(body_pr, "normAutofit", ns::A)?;
    let raw = attr(norm, "fontScale")?.parse::<f64>().ok()?;
    if raw <= 0.0 {
        return None;
    }
    Some(raw / 100_000.0)
}

/// Estimate `normAutofit` shrink when no authored `fontScale` (whole-point rounding).
pub fn scale_norm_autofit_font_size_px(
    font_size_px: f64,
    box_height_px: f64,
    inset_top_bottom_px: f64,
    paragraph_count: usize,
) -> f64 {
    let inner = (box_height_px - inset_top_bottom_px).max(1.0);
    let lines = paragraph_count.max(1) as f64;
    let needed = lines * font_size_px * 1.2;
    if needed <= inner + 0.5 {
        return font_size_px;
    }
    let scaled_pt = (font_size_px * (72.0 / 96.0) * inner / needed).round();
    (scaled_pt * 96.0 / 72.0).max(6.0)
}

/// Default `a:bodyPr` text insets at 96 DPI (91440×45720 EMU), matching upstream constants.
pub const DEFAULT_BODY_INSET_LR_PX: f64 = 91440.0 / 9525.0;
pub const DEFAULT_BODY_INSET_TB_PX: f64 = 45720.0 / 9525.0;

/// PowerPoint single-spacing pitch as a unitless CSS multiplier (COM-measured).
pub const DEFAULT_LINE_HEIGHT: f64 = 1.2;
const PT_TO_PX: f64 = 96.0 / 72.0;

/// Vertical anchor from `a:bodyPr/@anchor`, mapped like upstream `TextStyle.vAlign`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextBodyVAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

/// Layout parsed from `p:txBody` for painting.
#[derive(Debug, Clone)]
pub struct TextBodyLayout {
    pub v_align: TextBodyVAlign,
    pub text_align: Option<TextAlignCss>,
    pub inset_top_px: f64,
    pub inset_bottom_px: f64,
    pub inset_left_px: f64,
    pub inset_right_px: f64,
    /// `a:bodyPr/@numCol` (1..16); values above 1 enable CSS multicol.
    pub column_count: u8,
    /// `a:bodyPr/@spcCol` in CSS px.
    pub column_gap_px: f64,
    /// `a:bodyPr/@anchorCtr="1"` → `align-items: center` on the text box.
    pub anchor_center: bool,
    /// `vertOverflow` / `horzOverflow` clip (upstream `resolveTextOverflowClip`).
    pub overflow_clip: bool,
}

impl Default for TextBodyLayout {
    fn default() -> Self {
        Self {
            v_align: TextBodyVAlign::default(),
            text_align: None,
            inset_top_px: 0.0,
            inset_bottom_px: 0.0,
            inset_left_px: 0.0,
            inset_right_px: 0.0,
            column_count: 1,
            column_gap_px: 0.0,
            anchor_center: false,
            overflow_clip: false,
        }
    }
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

#[allow(dead_code)]
pub fn parse_text_body_layout(tx_body: &Element) -> TextBodyLayout {
    parse_text_body_layout_with_levels(tx_body, None, None)
}

/// Like [`parse_text_body_layout`] but falls back to merged placeholder `algn`.
pub fn parse_text_body_layout_with_levels(
    tx_body: &Element,
    effective_levels: Option<&crate::level_style::LevelStylesMap>,
    inherited_tx_body: Option<&Element>,
) -> TextBodyLayout {
    let parsed = parse_drawing_text_body(Some(tx_body));
    let inherited_parsed = inherited_tx_body.and_then(|tx| parse_drawing_text_body(Some(tx)));
    let v_align = parsed
        .as_ref()
        .and_then(|b| b.anchor.as_deref())
        .or_else(|| inherited_parsed.as_ref().and_then(|b| b.anchor.as_deref()))
        .and_then(parse_body_anchor)
        .unwrap_or(TextBodyVAlign::Top);

    let mut inset_top = DEFAULT_BODY_INSET_TB_PX;
    let mut inset_bottom = DEFAULT_BODY_INSET_TB_PX;
    let mut inset_left = DEFAULT_BODY_INSET_LR_PX;
    let mut inset_right = DEFAULT_BODY_INSET_LR_PX;
    let insets = parsed
        .as_ref()
        .and_then(|b| b.insets_emu.as_ref())
        .or_else(|| {
            inherited_parsed
                .as_ref()
                .and_then(|b| b.insets_emu.as_ref())
        });
    if let Some(insets) = insets {
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

    let text_align = resolve_first_paragraph_align(tx_body, parsed.as_ref()).or_else(|| {
        effective_levels.and_then(|map| {
            crate::level_style::level_style_for(map, 0).and_then(|s| s.paragraph_align)
        })
    });

    let body_pr = child_local(tx_body, "bodyPr", ns::A)
        .or_else(|| inherited_tx_body.and_then(|tx| child_local(tx, "bodyPr", ns::A)));
    let (column_count, column_gap_px, anchor_center, overflow_clip) =
        parse_body_pr_layout_flags(body_pr);

    TextBodyLayout {
        v_align,
        text_align,
        inset_top_px: inset_top,
        inset_bottom_px: inset_bottom,
        inset_left_px: inset_left,
        inset_right_px: inset_right,
        column_count,
        column_gap_px,
        anchor_center,
        overflow_clip,
    }
}

fn parse_body_pr_layout_flags(body_pr: Option<&Element>) -> (u8, f64, bool, bool) {
    let Some(bp) = body_pr else {
        return (1, 0.0, false, false);
    };
    let column_count = attr(bp, "numCol")
        .and_then(|s| s.parse::<u8>().ok())
        .map(|n| n.clamp(1, 16))
        .unwrap_or(1);
    let column_gap_px = attr(bp, "spcCol")
        .and_then(|s| s.parse::<i64>().ok())
        .map(emu_to_pixels)
        .unwrap_or(0.0);
    let anchor_center = parse_optional_bool_attr(attr(bp, "anchorCtr")) == Some(true);
    let vert = attr(bp, "vertOverflow");
    let horz = attr(bp, "horzOverflow");
    let overflow_clip = vert == Some("clip") || vert == Some("ellipsis") || horz == Some("clip");
    (column_count, column_gap_px, anchor_center, overflow_clip)
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
    /// `a:fld/@type` or inherited header/footer field tag (paint substitution).
    pub field_type: Option<String>,
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
    pub level_font_size_px: Option<f64>,
    pub level_font_family: Option<String>,
    pub level_font_bold: Option<bool>,
    pub level_font_italic: Option<bool>,
    pub level_text_color: Option<String>,
    pub list_level: i8,
    /// CSS `line-height` (unitless multiplier or `"Npx"`).
    pub line_height_css: Option<String>,
    /// Trailing gap after the paragraph (`spcBef` + `spcAft` combined).
    pub margin_bottom_px: Option<f64>,
    /// Exact line spacing: runs use `line-height: 0` inside the paragraph strut.
    pub run_line_height_zero: bool,
}

/// Master/layout `txStyles` + `lstStyle` cascade for list paint (#27).
pub struct TextPaintListStyleCtx<'a> {
    pub own_tx_body: Option<&'a Element>,
    pub inherited_tx_body: Option<&'a Element>,
    pub is_body_placeholder: bool,
    pub effective_level_styles: Option<&'a crate::level_style::LevelStylesMap>,
    pub presentation_level_styles: Option<&'a crate::level_style::LevelStylesMap>,
    pub font_scheme: Option<&'a crate::theme::ThemeFontScheme>,
    pub space_first_last: bool,
    pub compat_line_spacing: bool,
    pub line_spacing_reduction: Option<f64>,
    /// Stamp on plain runs when header/footer placeholders inherit literal text.
    pub inherited_field_type: Option<String>,
    pub placeholder_type: Option<String>,
}

/// Walk `p:txBody` paragraphs for multi-`p` / per-run BiDi painting (#27).
#[allow(dead_code)]
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
    let para_count = paras.len();
    let mut auto_seq = AutoNumberSequence::default();
    paras
        .iter()
        .enumerate()
        .map(|(para_index, p)| {
            let p_pr = child_local(p, "pPr", ns::A);
            let mut align = p_pr
                .and_then(|pp| attr(pp, "algn"))
                .and_then(parse_paragraph_align);
            if align.is_none() {
                if let Some(ctx) = list_styles {
                    let level = paragraph_level(p) as i8;
                    align = ctx
                        .effective_level_styles
                        .and_then(|map| crate::level_style::level_style_for(map, level))
                        .and_then(|s| s.paragraph_align);
                }
            }
            let rtl = p_pr.and_then(|pp| parse_optional_bool_attr(attr(pp, "rtl")));
            let tab_stops = p_pr.map(parse_tab_stops).unwrap_or_default();
            let inherited_field = list_styles.and_then(|c| c.inherited_field_type.as_deref());
            let runs = collect_paint_runs(p, inherited_field);
            let para_level = paragraph_level(p) as i8;
            let level_run_style = list_styles
                .and_then(|ctx| {
                    ctx.effective_level_styles.map(|map| {
                        crate::level_style::paragraph_level_run_style(
                            map,
                            para_level,
                            ctx.font_scheme,
                        )
                    })
                })
                .unwrap_or_default();

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
                    if !info.none && has_renderable {
                        if let Some(ch) = &info.char {
                            marker = Some(ch.clone());
                        } else if let Some(scheme) = auto_num_scheme.as_ref() {
                            let start = info.auto_num_start_at;
                            let ord = next_auto_number(&mut auto_seq, level, scheme, start);
                            marker = Some(format_auto_number_bullet(scheme, ord));
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

            let spacing = resolve_paint_paragraph_spacing(
                p_pr,
                list_styles,
                para_level,
                level_run_style.font_size_px,
                para_index,
                para_count,
            );

            TextPaintParagraph {
                runs,
                align,
                rtl,
                tab_stops,
                bullet_marker,
                margin_left_px,
                text_indent_px,
                level_font_size_px: level_run_style.font_size_px,
                level_font_family: level_run_style.font_family,
                level_font_bold: level_run_style.font_bold,
                level_font_italic: level_run_style.font_italic,
                level_text_color: level_run_style.text_color_hex,
                list_level: para_level,
                line_height_css: spacing.line_height_css,
                margin_bottom_px: spacing.margin_bottom_px,
                run_line_height_zero: spacing.run_line_height_zero,
            }
        })
        .collect()
}

struct ParagraphPaintSpacing {
    line_height_css: Option<String>,
    margin_bottom_px: Option<f64>,
    run_line_height_zero: bool,
}

fn resolve_paint_paragraph_spacing(
    p_pr: Option<&Element>,
    list_styles: Option<&TextPaintListStyleCtx<'_>>,
    para_level: i8,
    _para_font_size_px: Option<f64>,
    para_index: usize,
    para_count: usize,
) -> ParagraphPaintSpacing {
    let ph_type = list_styles.and_then(|c| c.placeholder_type.as_deref());
    let is_hf_ph = ph_type.is_some_and(|t| crate::inheritance::is_header_footer_placeholder(Some(t)));
    let is_title_ph = ph_type.is_some_and(|t| {
        matches!(
            t.trim().to_ascii_lowercase().as_str(),
            "title" | "ctrtitle"
        )
    });
    let cascade_line = is_hf_ph || is_title_ph;
    let cascade_margin = is_hf_ph;
    let body_level = list_styles.and_then(|ctx| {
        ctx.effective_level_styles
            .and_then(|m| crate::level_style::level_style_for(m, 0))
    });
    let level_style = list_styles.and_then(|ctx| {
        ctx.effective_level_styles
            .and_then(|m| crate::level_style::level_style_for(m, para_level))
    });
    let own = p_pr.map(paragraph_spacing_from_p_pr);
    let level = level_style.map(level_spacing_from_style);
    let body = body_level.map(level_spacing_from_style);

    let space_first_last = list_styles.is_some_and(|c| c.space_first_last);
    let compat = list_styles.is_some_and(|c| c.compat_line_spacing);
    let reduction = list_styles.and_then(|c| c.line_spacing_reduction);

    let is_first = para_index == 0;
    let is_last = para_index + 1 == para_count;

    let before = if cascade_margin {
        pick_spacing(
            own.as_ref().and_then(|o| o.before_px),
            level.as_ref().and_then(|l| l.before_px),
            body.as_ref().and_then(|b| b.before_px),
        )
    } else {
        own.as_ref().and_then(|o| o.before_px).unwrap_or(0.0)
    };
    let after = if cascade_margin {
        pick_spacing(
            own.as_ref().and_then(|o| o.after_px),
            level.as_ref().and_then(|l| l.after_px),
            body.as_ref().and_then(|b| b.after_px),
        )
    } else {
        own.as_ref().and_then(|o| o.after_px).unwrap_or(0.0)
    };
    let effective_before = if before > 0.0 && (!is_first || space_first_last) {
        before
    } else {
        0.0
    };
    let effective_after = if after > 0.0 && (!is_last || space_first_last) {
        after
    } else {
        0.0
    };
    let margin_bottom_px = {
        let combined = effective_before + effective_after;
        if combined > 0.0 {
            Some(combined)
        } else {
            None
        }
    };

    let has_own_ln = own
        .as_ref()
        .is_some_and(|o| o.line_exact_pt.is_some() || o.line_mult.is_some());
    let line_exact = if has_own_ln {
        own.as_ref().and_then(|o| o.line_exact_pt)
    } else if cascade_line {
        level
            .as_ref()
            .and_then(|l| l.line_exact_pt)
            .or_else(|| body.as_ref().and_then(|b| b.line_exact_pt))
    } else {
        None
    };
    let line_mult = if has_own_ln {
        own.as_ref().and_then(|o| o.line_mult)
    } else if cascade_line {
        level
            .as_ref()
            .and_then(|l| l.line_mult)
            .or_else(|| body.as_ref().and_then(|b| b.line_mult))
    } else {
        None
    };

    let (line_height_css, run_line_height_zero) =
        if let Some(exact_pt) = line_exact.filter(|v| *v > 0.0) {
            let px = exact_pt * PT_TO_PX;
            (
                Some(format!(
                    "{}px",
                    px.to_string().trim_end_matches('0').trim_end_matches('.')
                )),
                true,
            )
        } else if let Some(mult) = line_mult.filter(|m| *m > 0.0) {
            let proportional = if compat {
                mult
            } else {
                mult * DEFAULT_LINE_HEIGHT
            };
            let adjusted = reduction
                .filter(|r| *r > 0.0 && *r < 1.0)
                .map(|r| proportional * (1.0 - r))
                .unwrap_or(proportional);
            (Some(format_spacing_multiplier(adjusted)), false)
        } else {
            (None, false)
        };

    ParagraphPaintSpacing {
        line_height_css,
        margin_bottom_px,
        run_line_height_zero,
    }
}

fn format_spacing_multiplier(v: f64) -> String {
    format!("{:.6}", v)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

fn pick_spacing(own: Option<f64>, level: Option<f64>, body: Option<f64>) -> f64 {
    own.or(level).or(body).unwrap_or(0.0)
}

struct SpacingTriplet {
    line_mult: Option<f64>,
    line_exact_pt: Option<f64>,
    before_px: Option<f64>,
    after_px: Option<f64>,
}

fn level_spacing_from_style(
    style: &crate::level_style::PlaceholderTextLevelStyle,
) -> SpacingTriplet {
    SpacingTriplet {
        line_mult: style.line_spacing_multiplier,
        line_exact_pt: style.line_spacing_exact_pt,
        before_px: style.paragraph_spacing_before_px,
        after_px: style.paragraph_spacing_after_px,
    }
}

fn paragraph_spacing_from_p_pr(p_pr: &Element) -> SpacingTriplet {
    let basis = child_local(p_pr, "defRPr", ns::A)
        .and_then(|d| attr(d, "sz"))
        .and_then(|s| s.parse::<f64>().ok())
        .map(|sz| sz / 100.0 * PT_TO_PX);
    let mut out = SpacingTriplet {
        line_mult: None,
        line_exact_pt: None,
        before_px: None,
        after_px: None,
    };
    if let Some(ln) = child_local(p_pr, "lnSpc", ns::A) {
        if let Some(spc_pts) = child_local(ln, "spcPts", ns::A) {
            let raw = attr(spc_pts, "val").and_then(|s| s.parse::<i64>().ok());
            if let Some(raw) = raw.filter(|r| *r > 0) {
                out.line_exact_pt = Some(raw as f64 / 100.0);
            }
        } else if let Some(spc_pct) = child_local(ln, "spcPct", ns::A) {
            if let Some(raw) = attr(spc_pct, "val") {
                out.line_mult = parse_ooxml_percent_local(raw).map(|m| m.clamp(0.1, 5.0));
            }
        }
    }
    if let Some(bef) = child_local(p_pr, "spcBef", ns::A) {
        out.before_px = spacing_px_from_node(bef, basis);
    }
    if let Some(aft) = child_local(p_pr, "spcAft", ns::A) {
        out.after_px = spacing_px_from_node(aft, basis);
    }
    out
}

fn parse_ooxml_percent_local(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(stripped) = trimmed.strip_suffix('%') {
        return stripped.parse::<f64>().ok().map(|n| n / 100.0);
    }
    trimmed.parse::<f64>().ok().map(|n| n / 100_000.0)
}

fn spacing_px_from_node(node: &Element, basis_font_size_px: Option<f64>) -> Option<f64> {
    if let Some(spc_pts) = child_local(node, "spcPts", ns::A) {
        let raw = attr(spc_pts, "val").and_then(|s| s.parse::<i64>().ok())?;
        if raw > 0 {
            return Some(raw as f64 * PT_TO_PX / 100.0);
        }
    }
    if let Some(spc_pct) = child_local(node, "spcPct", ns::A) {
        let fraction = parse_ooxml_percent_local(attr(spc_pct, "val")?)?;
        let basis = basis_font_size_px.filter(|b| *b > 0.0)?;
        return Some(fraction * basis * DEFAULT_LINE_HEIGHT);
    }
    None
}

fn collect_paint_runs(p: &Element, inherited_field_type: Option<&str>) -> Vec<TextPaintRun> {
    let mut runs = Vec::new();
    for child in &p.children {
        if let Node::Element(el) = child {
            if el.ns.as_deref() != Some(ns::A) {
                continue;
            }
            let text = if el.local == "r" {
                run_text_content(el)
            } else if el.local == "fld" {
                field_text_content(el)
            } else {
                None
            };
            if let Some(text) = text {
                let r_pr = child_local(el, "rPr", ns::A);
                let rtl = r_pr.and_then(|rp| parse_optional_bool_attr(attr(rp, "rtl")));
                let underline = r_pr.map(rpr_has_underline).unwrap_or(false);
                let ruby = if el.local == "r" {
                    parse_run_ruby(el)
                } else {
                    None
                };
                let field_type = if el.local == "fld" {
                    attr(el, "type").map(|s| s.to_owned())
                } else if inherited_field_type.is_some() && !text.is_empty() {
                    inherited_field_type.map(|s| s.to_owned())
                } else {
                    None
                };
                runs.push(TextPaintRun {
                    text,
                    rtl,
                    underline,
                    ruby,
                    field_type,
                });
            }
        }
    }
    runs
}

fn field_text_content(fld: &Element) -> Option<String> {
    child_local(fld, "t", ns::A)
        .map(|t| t.text_content())
        .filter(|s| !s.is_empty())
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

    // Catches: slide-number/date `a:fld` runs dropped from paint paragraphs so hf chrome is blank.
    #[test]
    fn field_runs_become_paint_runs() {
        let doc = parse_xml_str(
            r#"<p:txBody xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <a:bodyPr/>
              <a:p><a:fld type="slidenum"><a:rPr lang="en-US"/><a:t>1</a:t></a:fld></a:p>
            </p:txBody>"#,
        )
        .unwrap();
        let paras = parse_text_paint_paragraphs(&doc.root);
        assert_eq!(paras.len(), 1);
        assert_eq!(paras[0].runs.len(), 1);
        assert_eq!(paras[0].runs[0].text, "1");
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
