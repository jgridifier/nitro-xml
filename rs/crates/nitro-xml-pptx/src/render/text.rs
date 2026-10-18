// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/text-block.ts,
// src/ui/src/pptx/render/text-effects.ts; changes in PROVENANCE.md.

use nitro_xml_render::measure::LayoutReadKind;

use super::context::{PaintContext, StringIntern};
use super::dom::{
    create_child, position_box, set_attr, set_inner_markup_stub, set_style, set_text_content,
};
use super::field_substitution;
use super::font_substitution::substitute_font_family_stack;
use super::shape_autofit_resize::queue_sp_autofit_scroll_height;
use crate::east_asian_breaks::{split_east_asian_breaks, EastAsianBreakOptions};
use crate::slide::DumpElement;
use crate::text_body_rect::{resolve_text_body_rect_padding, TextBodyRectPadding};
use crate::text_layout::{
    compute_body_tab_size_px, compute_tabbed_line_layout, line_height_to_px,
    resolve_css_text_align, resolve_vert_overflow_clip_lines, TextAlignCss, TextBodyLayout,
    TextBodyVAlign, TextPaintParagraph, TextPaintRuby, TextPaintRun, TextPaintTabStop,
};

/// Loader `fillMeasureWidths` sentinel: returns GDI letter-spacing px/char (upstream `text-metric-tracking`).
const SIZE_PPTX_METRIC_TRACKING: f32 = -3.0;
const SIZE_PPTX_HANG_SPACE: f32 = -4.0;
/// Visual host pins run glyphs to Liberation (`#ts-wrap *` / shadow font-pin); block keeps `DEFAULT_FONT_FAMILY`.
const VISUAL_HOST_RUN_FONT_STACK: &str = "\"Liberation Sans\", Arial, sans-serif";

#[derive(Clone, Copy)]
pub struct TextBodyPaint<'a> {
    pub text_color: Option<&'a str>,
    pub font_size_px: Option<f64>,
    pub font_family: Option<&'a str>,
    pub font_bold: bool,
    pub font_italic: bool,
    /// `a:rPr/@kern` or merged `defRPr/@kern` (hundredths of a point threshold).
    pub font_kern_hundredths: Option<i32>,
    /// `a:rPr/@cap` or merged `defRPr/@cap` (`all` / `small` / `none`).
    pub text_caps: Option<&'static str>,
    pub text_stroke_width_px: Option<f64>,
    pub text_stroke_color: Option<&'a str>,
    pub layout: Option<&'a TextBodyLayout>,
    pub paragraphs: Option<&'a [TextPaintParagraph]>,
    pub text_shadow: Option<&'a str>,
    pub geom_rect_padding: TextBodyRectPadding,
    /// When true, apply strict kinsoku (`p:defaultTextStyle/@eaLnBrk="1"` decks only).
    pub presentation_kinsoku: bool,
    /// Shape box height for `vertOverflow` line-boundary clip-path.
    pub body_box_height_px: Option<f64>,
    /// `a:bodyPr/@compatLnSpc` — use legacy (97-2003) line-spacing algorithm (line-height 1 instead of 1.2).
    pub compat_line_spacing: bool,
    /// `a:bodyPr/a:prstTxWarp/@prst` (WordArt preset, paint only).
    pub text_warp_preset: Option<&'a str>,
    /// `a:bodyPr/a:prstTxWarp/a:avLst/a:gd[@name="adj"]` adjustment value.
    pub text_warp_adj: Option<i32>,
    /// `a:bodyPr/a:prstTxWarp/a:avLst/a:gd[@name="adj2"]` adjustment value.
    pub text_warp_adj2: Option<i32>,
}

impl<'a> TextBodyPaint<'a> {
    pub fn from_element(el: &'a DumpElement) -> Self {
        Self {
            text_color: el.text_color.as_deref(),
            font_size_px: el.font_size_px,
            font_family: el.font_family.as_deref(),
            font_bold: el.font_bold,
            font_italic: el.font_italic,
            text_stroke_width_px: el.text_stroke_width_px,
            text_stroke_color: el.text_stroke_color.as_deref(),
            layout: el.text_body_layout.as_ref(),
            paragraphs: el.text_paint_paragraphs.as_deref(),
            text_shadow: el.text_shadow.as_deref(),
            geom_rect_padding: resolve_text_body_rect_padding(
                el.shape_type.as_deref(),
                el.width,
                el.height,
                None,
            ),
            presentation_kinsoku: el.presentation_kinsoku,
            font_kern_hundredths: None,
            text_caps: None,
            body_box_height_px: Some(el.height.max(0.0)),
            compat_line_spacing: el.compat_line_spacing,
            text_warp_preset: el.text_warp_preset.as_deref(),
            text_warp_adj: el.text_warp_adj,
            text_warp_adj2: el.text_warp_adj2,
        }
    }
}

/// Ported from upstream `resolveFontKerning` (text-run-style.ts).
fn resolve_font_kerning(kern_hundredths: Option<i32>, font_size_px: f64) -> Option<&'static str> {
    let kern = kern_hundredths?;
    if kern == 0 {
        return Some("none");
    }
    let font_size_pt = font_size_px * (72.0 / 96.0);
    if font_size_pt >= kern as f64 / 100.0 {
        Some("normal")
    } else {
        Some("none")
    }
}

/// Super/subscript glyphs render at ~65% of the run font size (upstream BASELINE_FONT_SCALE).
const BASELINE_FONT_SCALE: f64 = 0.65;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let painted_h = ctx.sp_autofit.painted_height(&el.id, el.height);
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, painted_h);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "text");
    let text = el.text.as_deref().unwrap_or("");

    // Dispatch: if text warp preset is present and supported, render via SVG textPath.
    if let Some(preset) = el.text_warp_preset.as_deref() {
        if super::wordart::should_use_svg_warp(preset) {
            let block = paint_warped_text_body(ctx, id, text, el);
            queue_sp_autofit_scroll_height(
                ctx.layout_reads,
                &mut ctx.layout_read_records,
                &el.id,
                el.auto_fit_mode,
                block,
                painted_h,
            );
            let font_size_prop = ctx.intern.intern("font-size");
            ctx.push_layout_read(block, LayoutReadKind::ComputedStyle, font_size_prop, None);
            ctx.view_ir.push_painted("text", el, None);
            return;
        }
    }

    let block = paint_text_body(ctx, id, text, TextBodyPaint::from_element(el));
    queue_sp_autofit_scroll_height(
        ctx.layout_reads,
        &mut ctx.layout_read_records,
        &el.id,
        el.auto_fit_mode,
        block,
        painted_h,
    );
    // TODO(#27): normAutofit shrink uses ComputedStyle + iterative steps.
    let font_size_prop = ctx.intern.intern("font-size");
    ctx.push_layout_read(block, LayoutReadKind::ComputedStyle, font_size_prop, None);
    ctx.view_ir.push_painted("text", el, None);
}

fn paragraphs_have_paintable_runs(paragraphs: &[TextPaintParagraph]) -> bool {
    paragraphs
        .iter()
        .any(|p| !p.runs.is_empty() || p.bullet_marker.is_some())
}

fn paint_plain_text_runs<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    block: u32,
    text: &str,
    paint: &TextBodyPaint<'_>,
    size: f64,
    stack: &str,
) {
    let para_align = paint.layout.and_then(|l| l.text_align);
    let para = open_paragraph(ctx, block, para_align, None);
    let metric_stack = VISUAL_HOST_RUN_FONT_STACK.to_owned();
    let pieces = split_run_for_metric_pieces(text);
    let emit_gdi = !paint.layout.is_some_and(|l| l.wrap_none) && pieces.len() > 1;
    for piece in pieces {
        let run = create_child(ctx, para, "span");
        set_text_content(ctx, run, &piece);
        if paint.font_family.is_none() {
            set_style(ctx, run, "font-family", VISUAL_HOST_RUN_FONT_STACK);
        }
        paint_run_typography(ctx, run, paint, size, stack);
        if emit_gdi {
            apply_gdi_metric_letter_spacing_with_stack(
                ctx,
                run,
                &piece,
                size,
                &metric_stack,
                paint.font_bold,
                paint.font_italic,
            );
        }
    }
}

fn open_paragraph<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    block: u32,
    text_align: Option<TextAlignCss>,
    paragraph_rtl: Option<bool>,
) -> u32 {
    let para = create_child(ctx, block, "p");
    set_attr(ctx, para, "class", "pptxv-para");
    set_style(ctx, para, "margin", "0");
    if let Some(align) = text_align {
        set_style(ctx, para, "text-align", align.as_css());
    }
    if let Some(rtl) = paragraph_rtl {
        set_style(ctx, para, "direction", if rtl { "rtl" } else { "ltr" });
        set_style(ctx, para, "unicode-bidi", "embed");
    } else {
        set_style(ctx, para, "direction", "ltr");
        set_style(ctx, para, "unicode-bidi", "embed");
    }
    para
}

/// PowerPoint single-spaced envelope line pitch (upstream `LINE_PITCH_EM`).
const ENVELOPE_LINE_PITCH_EM: f64 = 1.2;
/// Cap-height estimate above baseline without ink metrics (`FALLBACK_CAP_HEIGHT_EM`).
const ENVELOPE_CAP_HEIGHT_EM: f64 = 0.72;

/// Measure per-glyph advances via MeasureBatch (canvas `measureText`), matching
/// upstream `measureGlyphAdvances`: a glyph starts at its prefix width minus its
/// lone width, so pair kerning moves the glyph it applies to. Falls back to
/// `0.55em` until the host replies on the next paint pass.
fn envelope_glyph_advances<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    text: &str,
    canvas_font: &str,
    size: f64,
) -> Vec<f64> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }
    let font_id = ctx.intern.intern(canvas_font);
    let estimate = size * 0.55;
    let mut pending = false;
    let mut width = |ctx: &mut PaintContext<'_, I>, s: &str| -> f64 {
        let key = ctx.intern.intern(&format!("envw:{s}"));
        if let Some(cached) = ctx.measure.cached_width(font_id, key) {
            cached
        } else {
            ctx.measure.push(font_id, size as f32, key);
            pending = true;
            0.0
        }
    };
    let mut prefix = String::new();
    let mut starts = Vec::with_capacity(chars.len());
    for ch in &chars {
        prefix.push(*ch);
        let w_prefix = width(ctx, &prefix);
        let w_char = width(ctx, &ch.to_string());
        starts.push(w_prefix - w_char);
    }
    let total = width(ctx, &prefix);
    if pending {
        return vec![estimate; chars.len()];
    }
    (0..starts.len())
        .map(|i| {
            let next = starts.get(i + 1).copied().unwrap_or(total);
            (next - starts[i]).max(0.0)
        })
        .collect()
}

/// One glyph placed in the unwarped envelope block (upstream `EnvelopeBlockGlyph`).
struct EnvelopeGlyph {
    ch: char,
    x: f64,
    baseline: f64,
    advance: f64,
    outline: Option<Vec<Vec<super::wordart::OutlinePoint>>>,
    ink: Option<super::wordart_envelope_map::EnvelopeBlockBox>,
}

/// Paint WordArt glyph-envelope warp via per-glyph SVG <text> elements.
/// 1:1 with upstream `buildGlyphEnvelopeBlock` + React `EnvelopeGlyph` (affine
/// fallback; outline warp deferred — needs font outline host).
#[allow(clippy::too_many_arguments)]
fn paint_warped_text_envelope<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    block: u32,
    text: &str,
    preset: &str,
    w: f64,
    h: f64,
    adj1: Option<i32>,
    adj2: Option<i32>,
    size: f64,
    stack: &str,
    color: &str,
    weight: &str,
    style: &str,
    bold: bool,
    italic: bool,
) -> u32 {
    let paragraphs: Vec<&str> = if text.is_empty() {
        vec![""]
    } else {
        text.split('\n').collect()
    };
    let canvas_font = canvas_measure_font(stack, size, bold, italic);

    // Per-line advances + widths (upstream layoutEnvelopeBlock).
    let line_advances: Vec<Vec<f64>> = paragraphs
        .iter()
        .map(|p| envelope_glyph_advances(ctx, p, &canvas_font, size))
        .collect();
    let line_widths: Vec<f64> = line_advances
        .iter()
        .map(|a| a.iter().sum::<f64>())
        .collect();
    let block_width = line_widths.iter().copied().fold(0.0_f64, f64::max);
    if block_width <= 0.0 {
        set_text_content(ctx, block, text);
        return block;
    }

    // Stack baselines at 1.2em pitch (single size per block, so the
    // `BASELINE_FRACTION` blend reduces to `1.2 * size`).
    let cap = size * ENVELOPE_CAP_HEIGHT_EM;
    let font_id = ctx.intern.intern(&canvas_font);
    let mut lines: Vec<Vec<EnvelopeGlyph>> = Vec::with_capacity(paragraphs.len());
    let mut block_box = super::wordart_envelope_map::EnvelopeBlockBox {
        left: f64::INFINITY,
        top: f64::INFINITY,
        right: f64::NEG_INFINITY,
        bottom: f64::NEG_INFINITY,
    };
    let mut has_ink = false;
    for (para_idx, para) in paragraphs.iter().enumerate() {
        let advances = &line_advances[para_idx];
        let baseline = para_idx as f64 * ENVELOPE_LINE_PITCH_EM * size;
        let mut x = (block_width - line_widths[para_idx]) / 2.0;
        let mut glyphs = Vec::new();
        for (idx, ch) in para.chars().enumerate() {
            let advance = advances.get(idx).copied().unwrap_or(size * 0.55);
            let mut glyph = EnvelopeGlyph {
                ch,
                x,
                baseline,
                advance,
                outline: None,
                ink: None,
            };
            if ch.is_whitespace() {
                block_box.left = block_box.left.min(x);
                block_box.right = block_box.right.max(x + advance);
            } else {
                let char_id = ctx.intern.intern(&ch.to_string());
                match ctx.outlines.cached_contours(font_id, char_id) {
                    Some(em) if em.iter().any(|ring| !ring.is_empty()) => {
                        glyph.outline = Some(
                            em.iter()
                                .map(|ring| {
                                    ring.iter()
                                        .map(|(ex, ey)| super::wordart::OutlinePoint {
                                            x: x + ex * size,
                                            y: baseline + ey * size,
                                        })
                                        .collect()
                                })
                                .collect(),
                        );
                    }
                    Some(_) => {}
                    None => ctx.outlines.push(font_id, char_id, size as f32),
                }
                let ink = match &glyph.outline {
                    Some(contours) => {
                        let mut b = super::wordart_envelope_map::EnvelopeBlockBox {
                            left: f64::INFINITY,
                            top: f64::INFINITY,
                            right: f64::NEG_INFINITY,
                            bottom: f64::NEG_INFINITY,
                        };
                        for p in contours.iter().flatten() {
                            b.left = b.left.min(p.x);
                            b.right = b.right.max(p.x);
                            b.top = b.top.min(p.y);
                            b.bottom = b.bottom.max(p.y);
                        }
                        b
                    }
                    None => super::wordart_envelope_map::EnvelopeBlockBox {
                        left: x,
                        top: baseline - cap,
                        right: x + advance,
                        bottom: baseline,
                    },
                };
                has_ink = true;
                block_box.left = block_box.left.min(ink.left);
                block_box.right = block_box.right.max(ink.right);
                block_box.top = block_box.top.min(ink.top);
                block_box.bottom = block_box.bottom.max(ink.bottom);
                glyph.ink = Some(ink);
            }
            glyphs.push(glyph);
            x += advance;
        }
        lines.push(glyphs);
    }
    if !has_ink || !block_box.left.is_finite() {
        set_text_content(ctx, block, text);
        return block;
    }

    let warp = match super::wordart::create_envelope_warp(preset, w, h, adj1, adj2, block_box) {
        Some(warp) => warp,
        None => {
            set_text_content(ctx, block, text);
            return block;
        }
    };

    let mut svg_content = String::new();
    let safe_id = block.to_string();
    let escaped_stack = xml_escape_attr(stack);
    let max_segment = ((block_box.right - block_box.left) / 240.0).max(1e-3);

    for (para_idx, glyphs) in lines.iter().enumerate() {
        for (idx, g) in glyphs.iter().enumerate() {
            let ch = g.ch;
            if ch.is_whitespace() {
                continue;
            }
            let x = g.x;
            let y_baseline = g.baseline;
            let path_d = g.outline.as_ref().and_then(|contours| {
                super::wordart::build_warped_glyph_outline_path_d(contours, &*warp, max_segment)
            });
            if let Some(path_d) = path_d {
                svg_content.push_str(&format!(r#"<path d="{path_d}" fill="{color}" />"#));
                continue;
            }

            let glyph_box = match g.ink {
                Some(ink) => super::wordart_envelope_affine::GlyphFitBox {
                    x0: ink.left,
                    x1: ink.right,
                    y0: ink.top,
                    y1: ink.bottom,
                },
                None => super::wordart_envelope_affine::GlyphFitBox {
                    x0: x,
                    x1: x + g.advance.max(1e-3),
                    y0: y_baseline - cap,
                    y1: y_baseline,
                },
            };
            let fit = super::wordart::fit_glyph_envelope_affine(&*warp, glyph_box, h);
            let glyph_str = xml_escape_text(&ch.to_string());

            if let Some(slices) = &fit.slices {
                for (slice_idx, slice) in slices.iter().enumerate() {
                    let clip_id = format!("{safe_id}-clip-{para_idx}-{idx}-{slice_idx}");
                    svg_content.push_str(&format!(
                        r#"<defs><clipPath id="{clip_id}" clipPathUnits="userSpaceOnUse"><rect x="{x0}" y="-100000" width="{ww}" height="200000"/></clipPath></defs>"#,
                        clip_id = clip_id,
                        x0 = slice.clip_x0,
                        ww = (slice.clip_x1 - slice.clip_x0).max(0.0),
                    ));
                    svg_content.push_str(&format!(
                        r#"<text fill="{color}" font-family="{stack}" font-size="{size}" font-weight="{weight}" font-style="{style}" x="{gx}" y="{gy}" transform="{xf}" clip-path="url(#{clip_id})">{glyph}</text>"#,
                        color = color,
                        stack = escaped_stack,
                        size = size,
                        weight = weight,
                        style = style,
                        gx = x,
                        gy = y_baseline,
                        xf = slice.transform,
                        clip_id = clip_id,
                        glyph = glyph_str,
                    ));
                }
            } else {
                svg_content.push_str(&format!(
                    r#"<text fill="{color}" font-family="{stack}" font-size="{size}" font-weight="{weight}" font-style="{style}" x="{gx}" y="{gy}" transform="{xf}">{glyph}</text>"#,
                    color = color,
                    stack = escaped_stack,
                    size = size,
                    weight = weight,
                    style = style,
                    gx = x,
                    gy = y_baseline,
                    xf = fit.transform,
                    glyph = glyph_str,
                ));
            }
        }
    }

    let markup = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" preserveAspectRatio="none" aria-hidden="true" class="pptxv-wordart" style="position:absolute;inset:0;width:100%;height:100%;overflow:visible">{svg_content}</svg>"#
    );
    set_inner_markup_stub(ctx, block, &markup);
    block
}

/// Paint WordArt warp. Path presets use SVG `<textPath>` via SetInnerMarkup
/// (loader only createElementNS's the root `<svg>` tag). Envelope presets use
/// per-glyph affine transforms for accurate envelope mapping.
fn paint_warped_text_body<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    parent: u32,
    text: &str,
    el: &DumpElement,
) -> u32 {
    let block = create_child(ctx, parent, "div");
    set_attr(ctx, block, "class", "pptxv-text pptxv-wordart");
    set_style(ctx, block, "width", "100%");
    set_style(ctx, block, "height", "100%");
    set_style(ctx, block, "overflow", "visible");
    set_style(ctx, block, "position", "relative");

    let preset = el.text_warp_preset.as_deref().unwrap_or("");
    let size = el.font_size_px.unwrap_or(24.0).max(1.0);
    let stack = substitute_font_family_stack(el.font_family.as_deref());
    let color = el.text_color.as_deref().unwrap_or("#000000");
    let weight = if el.font_bold { "bold" } else { "normal" };
    let style = if el.font_italic { "italic" } else { "normal" };

    // Envelope family: per-glyph envelope warp via SVG <text> elements
    if super::wordart::has_glyph_envelope(preset) {
        return paint_warped_text_envelope(
            ctx,
            block,
            text,
            preset,
            el.width.max(1.0),
            el.height.max(1.0),
            el.text_warp_adj,
            el.text_warp_adj2,
            size,
            &stack,
            color,
            weight,
            style,
            el.font_bold,
            el.font_italic,
        );
    }

    // Path family: SVG textPath markup (upstream `renderPathWarp`).
    let w = el.width.max(1.0);
    let h = el.height.max(1.0);
    let lines: Vec<&str> = if text.is_empty() {
        vec![""]
    } else {
        text.split('\n').collect()
    };
    let line_count = lines.len().max(1);
    let safe_id = el
        .id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let mut defs = String::new();
    let mut texts = String::new();
    for (index, line) in lines.iter().enumerate() {
        let path_id = format!("{safe_id}-warp-{index}");
        let d = super::wordart::build_warp_path(
            preset,
            w,
            h,
            index,
            line_count,
            el.text_warp_adj,
            el.text_warp_adj2,
        )
        .unwrap_or_else(|| {
            let y = h
                * (0.2
                    + (if line_count <= 1 {
                        0.5
                    } else {
                        index as f64 / (line_count as f64 - 1.0)
                    }) * 0.6);
            format!("M 0,{y} L {w},{y}")
        });
        let escaped_d = xml_escape_attr(&d);
        let escaped_line = xml_escape_text(line);
        // font-family stacks contain CSS quotes (`"Arial", ...`); must be
        // attribute-escaped or SVG innerHTML splits the attribute (TS uses
        // &quot; via React). Same for fill if a theme colour ever has `&`.
        let escaped_stack = xml_escape_attr(&stack);
        let escaped_color = xml_escape_attr(color);
        defs.push_str(&format!(r#"<path id="{path_id}" d="{escaped_d}"></path>"#));
        let href = format!("#{path_id}");
        texts.push_str(&format!(
            r#"<text fill="{color}" font-family="{stack}" font-size="{size}" font-weight="{weight}" font-style="{style}"><textPath href="{href}" startOffset="50%" text-anchor="middle">{escaped_line}</textPath></text>"#,
            color = escaped_color,
            stack = escaped_stack,
            size = size,
            weight = weight,
            style = style,
            href = href,
            escaped_line = escaped_line,
        ));
    }
    let markup = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" preserveAspectRatio="none" aria-hidden="true" class="pptxv-wordart" style="position:absolute;inset:0;width:100%;height:100%;overflow:visible"><defs>{defs}</defs>{texts}</svg>"#
    );
    set_inner_markup_stub(ctx, block, &markup);
    block
}

fn xml_escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn xml_escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn paint_text_body<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    parent: u32,
    text: &str,
    paint: TextBodyPaint<'_>,
) -> u32 {
    let block = create_child(ctx, parent, "div");
    set_attr(ctx, block, "class", "pptxv-text");
    let wrap_none = paint.layout.is_some_and(|l| l.wrap_none);
    set_style(
        ctx,
        block,
        "white-space",
        if wrap_none { "nowrap" } else { "pre-wrap" },
    );
    if !wrap_none {
        if paint.presentation_kinsoku {
            set_style(ctx, block, "line-break", "strict");
            set_style(ctx, block, "word-break", "normal");
            set_style(ctx, block, "overflow-wrap", "break-word");
        } else {
            set_style(ctx, block, "word-break", "break-word");
        }
    } else if paint.presentation_kinsoku {
        set_style(ctx, block, "line-break", "strict");
        set_style(ctx, block, "word-break", "normal");
    }
    if paint.layout.is_some() {
        // Upstream text bodies use content-box (default): `width: 100%` is the
        // text column inside `a:bodyPr` insets, not including padding.
        set_style(ctx, block, "width", "100%");
        set_style(ctx, block, "height", "100%");
        if paint.layout.is_some_and(|l| l.column_count > 1) {
            set_style(ctx, block, "display", "block");
        } else {
            set_style(ctx, block, "display", "flex");
            set_style(ctx, block, "flex-direction", "column");
        }
    }
    let size = paint.font_size_px.unwrap_or(24.0).max(1.0);
    let per_paragraph_align = paint.paragraphs.is_some_and(|paras| !paras.is_empty());
    let italic_body = paint.font_italic
        || paint.paragraphs.is_some_and(|paras| {
            paras.iter().any(|p| {
                p.level_font_italic == Some(true)
                    || p.runs.iter().any(|r| r.run_font_italic == Some(true))
            })
        });
    let italic_inset_slop = if italic_body { 1.0 } else { 0.0 };
    if let Some(layout) = paint.layout {
        apply_text_body_layout(
            ctx,
            block,
            layout,
            paint.geom_rect_padding,
            per_paragraph_align,
            italic_inset_slop,
        );
        if layout.overflow_clip && layout.v_align == TextBodyVAlign::Top {
            if let Some(box_h) = paint.body_box_height_px.filter(|h| *h > 0.0) {
                let pad_top = layout.inset_top_px + paint.geom_rect_padding.top + italic_inset_slop;
                let pad_bottom =
                    layout.inset_bottom_px + paint.geom_rect_padding.bottom + italic_inset_slop;
                let content_h = box_h - pad_top - pad_bottom;
                // Clip estimate uses the block's line height (upstream `buildTextBlockStyle`), not per-paragraph `lnSpc`.
                let line_h_px = line_height_to_px(size, Some("1.2"));
                if let Some(clip) =
                    resolve_vert_overflow_clip_lines(content_h, pad_bottom, line_h_px)
                {
                    set_style(ctx, block, "clip-path", &clip);
                }
            }
        }
    } else if paint.geom_rect_padding != TextBodyRectPadding::default() {
        apply_geom_rect_padding_only(ctx, block, paint.geom_rect_padding);
    }
    let stack = substitute_font_family_stack(paint.font_family);
    set_style(ctx, block, "font-family", &stack);
    set_style(ctx, block, "font-size", &format!("{}px", format_px(size)));
    // Upstream `buildTextBlockStyle`: compatLnSpc uses legacy 1.0 line-spacing, else 1.2.
    set_style(
        ctx,
        block,
        "line-height",
        if paint.compat_line_spacing {
            "1"
        } else {
            "1.2"
        },
    );
    if paint.font_bold {
        set_style(ctx, block, "font-weight", "700");
    }
    if paint.font_italic {
        set_style(ctx, block, "font-style", "italic");
    }
    if let Some(color) = paint.text_color {
        set_style(ctx, block, "color", color);
    }
    // Upstream `buildTextBlockStyle`: default visible overflow, but `vertOverflow` clip wins.
    if !paint.layout.is_some_and(|l| l.overflow_clip) {
        set_style(ctx, block, "overflow", "visible");
    }
    set_style(ctx, block, "text-spacing-trim", "space-all");
    if let Some(paragraphs) = paint.paragraphs.filter(|p| !p.is_empty()) {
        if paragraphs_have_paintable_runs(paragraphs) {
            if paragraphs.iter().any(|p| p.rtl == Some(true)) {
                set_style(ctx, block, "direction", "rtl");
                set_style(ctx, block, "unicode-bidi", "plaintext");
            }
            if let Some(tab_px) = compute_body_tab_size_px(paragraphs) {
                set_style(ctx, block, "tab-size", &format_layout_px(tab_px));
            }
            paint_rich_paragraphs(ctx, block, paragraphs, &paint, size, &stack);
        }
        // Upstream `renderTextShapeElement`: segments exist but none carry a run or bullet
        // (empty paragraph) paints nothing; `element.text` is only the no-segments fallback.
    } else {
        apply_plain_body_kinsoku(ctx, block, &paint);
        paint_plain_text_runs(ctx, block, text, &paint, size, &stack);
    }
    if let Some(shadow) = paint.text_shadow {
        set_style(ctx, block, "text-shadow", shadow);
    }
    block
}

fn paint_run_typography<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    run: u32,
    paint: &TextBodyPaint<'_>,
    size: f64,
    stack: &str,
) {
    if paint.font_family.is_some() {
        set_style(ctx, run, "font-family", stack);
    }
    set_style(ctx, run, "font-size", &format_layout_px(size));
    set_style(
        ctx,
        run,
        "font-weight",
        if paint.font_bold { "700" } else { "normal" },
    );
    set_style(
        ctx,
        run,
        "font-style",
        if paint.font_italic {
            "italic"
        } else {
            "normal"
        },
    );
    if let Some(color) = paint.text_color {
        set_style(ctx, run, "color", color);
        // Upstream `hollowTextFillStyle`: `a:rPr/a:noFill` clears the glyph
        // interior via `-webkit-text-fill-color` (Chromium fills from that, not
        // `color`). Keep `color: transparent` as the non-webkit fallback.
        if color.eq_ignore_ascii_case("transparent") {
            set_style(ctx, run, "-webkit-text-fill-color", "transparent");
        }
    }
    if let Some(kern) = resolve_font_kerning(paint.font_kern_hundredths, size) {
        set_style(ctx, run, "font-kerning", kern);
    }
    match paint.text_caps {
        Some("all") => set_style(ctx, run, "text-transform", "uppercase"),
        Some("small") => set_style(ctx, run, "font-variant-caps", "small-caps"),
        _ => {}
    }
    if let Some(width) = paint.text_stroke_width_px.filter(|w| *w > 0.0) {
        // When fill is transparent, pin stroke away from currentColor so the
        // outline survives (upstream hollowTextFillStyle).
        let stroke_color = paint.text_stroke_color.unwrap_or_else(|| {
            if paint
                .text_color
                .is_some_and(|c| c.eq_ignore_ascii_case("transparent"))
            {
                "#000000"
            } else {
                "currentColor"
            }
        });
        set_style(
            ctx,
            run,
            "-webkit-text-stroke",
            &format!("{}px {}", format_px(width), stroke_color),
        );
        set_style(ctx, run, "paint-order", "stroke fill");
    }
}

fn paint_rich_paragraphs<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    block: u32,
    paragraphs: &[TextPaintParagraph],
    paint: &TextBodyPaint<'_>,
    size: f64,
    _stack: &str,
) {
    let body_align = paint.layout.and_then(|l| l.text_align);
    // Paragraphs without `a:pPr/@rtl` inherit the shape's resolved direction
    // (upstream element `textStyle.rtl`, often from earlier paragraphs in the body).
    let body_rtl = paragraphs.iter().find_map(|p| p.rtl);
    for para_model in paragraphs {
        let para_size = para_model.level_font_size_px.unwrap_or(size);
        let paragraph_content_size =
            crate::text_layout::paragraph_strut_font_size_px(para_model, size).unwrap_or(para_size);
        let para_paint = TextBodyPaint {
            text_color: para_model.level_text_color.as_deref().or(paint.text_color),
            font_size_px: Some(para_size),
            font_family: para_model
                .level_font_family
                .as_deref()
                .or(paint.font_family),
            font_bold: para_model.level_font_bold.unwrap_or(paint.font_bold),
            font_italic: para_model.level_font_italic.unwrap_or(paint.font_italic),
            // Master/layout `defRPr/@kern` (bug62513 titleStyle kern="1200").
            font_kern_hundredths: para_model
                .level_font_kern_hundredths
                .or(paint.font_kern_hundredths),
            text_caps: para_model.level_text_caps.or(paint.text_caps),
            text_stroke_width_px: paint.text_stroke_width_px,
            text_stroke_color: paint.text_stroke_color,
            layout: paint.layout,
            paragraphs: paint.paragraphs,
            text_shadow: paint.text_shadow,
            geom_rect_padding: paint.geom_rect_padding,
            presentation_kinsoku: paint.presentation_kinsoku,
            body_box_height_px: paint.body_box_height_px,
            compat_line_spacing: paint.compat_line_spacing,
            text_warp_preset: paint.text_warp_preset,
            text_warp_adj: paint.text_warp_adj,
            text_warp_adj2: paint.text_warp_adj2,
        };
        let para_rtl = para_model.rtl.or(body_rtl);
        let is_rtl = para_rtl == Some(true);
        let css_align = resolve_css_text_align(para_model.align.or(body_align), is_rtl);
        let para = create_child(ctx, block, "p");
        set_attr(ctx, para, "class", "pptxv-para");
        set_style(ctx, para, "margin", "0");
        if let Some(strut_px) = crate::text_layout::paragraph_strut_font_size_px(para_model, size) {
            set_style(
                ctx,
                para,
                "font-size",
                &format!("{}px", format_px(strut_px)),
            );
        }
        if let Some(margin_left) = para_model.margin_left_px {
            set_style(
                ctx,
                para,
                "margin-left",
                &format!("{}px", format_px(margin_left)),
            );
        }
        if let Some(text_indent) = para_model.text_indent_px {
            set_style(
                ctx,
                para,
                "text-indent",
                &format!("{}px", format_px(text_indent)),
            );
        }
        if let Some(align) = css_align {
            set_style(ctx, para, "text-align", align.as_css());
        } else {
            set_style(ctx, para, "text-align", "left");
        }
        if let Some(rtl) = para_rtl {
            set_style(ctx, para, "direction", if rtl { "rtl" } else { "ltr" });
            set_style(ctx, para, "unicode-bidi", "embed");
        } else {
            set_style(ctx, para, "direction", "ltr");
            set_style(ctx, para, "unicode-bidi", "embed");
        }
        if let Some(lh) = &para_model.line_height_css {
            set_style(ctx, para, "line-height", lh);
        }
        if let Some(mb) = para_model.margin_bottom_px {
            set_style(ctx, para, "margin-bottom", &format!("{}px", format_px(mb)));
        }
        apply_paragraph_kinsoku_line_break(
            ctx,
            para,
            para_model.east_asian_breaks,
            paint.presentation_kinsoku,
        );
        if let Some(marker) = &para_model.bullet_marker {
            let bullet_typo = para_model
                .runs
                .first()
                .map(|r| merge_run_level_paint(&para_paint, r, para_size));
            let (bullet_paint, bullet_size) = match &bullet_typo {
                Some((p, s)) => (p, *s),
                None => (&para_paint, para_size),
            };
            let bullet_stack = if let Some(face) = para_model.bullet_font_family.as_deref() {
                substitute_font_family_stack(Some(face))
            } else {
                substitute_font_family_stack(bullet_paint.font_family)
            };
            paint_bullet_marker(
                ctx,
                para,
                marker,
                para_model.text_indent_px,
                bullet_paint,
                bullet_size,
                &bullet_stack,
                para_model.bullet_text_color.as_deref(),
            );
        }
        if para_model.runs.is_empty() {
            if let Some(empty_size) = para_model.empty_line_font_size_px {
                set_style(
                    ctx,
                    para,
                    "font-size",
                    &format!("{}px", format_px(empty_size)),
                );
            }
            create_child(ctx, para, "br");
        } else {
            let trimmed_runs = crate::paragraph_trailing_space::trim_paragraph_trailing_runs(
                &para_model.runs,
                css_align,
                para_rtl,
            );
            for (run_idx, run_model) in trimmed_runs.iter().enumerate() {
                let (run_paint, run_size) =
                    merge_run_level_paint(&para_paint, run_model, para_size);
                let run_stack = substitute_font_family_stack(run_paint.font_family);
                let following =
                    crate::east_asian_breaks::following_run_char(&trimmed_runs, run_idx);
                let east_asian = crate::east_asian_breaks::east_asian_breaks_for_run(
                    para_model.east_asian_breaks,
                    run_model.run_lang.as_deref(),
                );
                append_paint_run(
                    ctx,
                    para,
                    run_model,
                    para_rtl,
                    &para_model.tab_stops,
                    para_model.default_tab_size_px.unwrap_or(0.0),
                    &run_paint,
                    run_size,
                    paragraph_content_size,
                    &run_stack,
                    para_model.run_line_height_zero,
                    east_asian,
                    following,
                );
            }
        }
    }
}

fn merge_run_level_paint<'a>(
    base: &'a TextBodyPaint<'a>,
    run: &'a TextPaintRun,
    para_size: f64,
) -> (TextBodyPaint<'a>, f64) {
    let size = run.run_font_size_px.unwrap_or(para_size);
    // Apply super/subscript font-size scale (upstream BASELINE_FONT_SCALE).
    let scaled_size = if run.baseline_shift.is_some() {
        size * BASELINE_FONT_SCALE
    } else {
        size
    };
    (
        TextBodyPaint {
            text_color: run.run_text_color.as_deref().or(base.text_color),
            font_size_px: Some(scaled_size),
            font_family: run.run_font_family.as_deref().or(base.font_family),
            font_bold: run.run_font_bold.unwrap_or(base.font_bold),
            font_italic: run.run_font_italic.unwrap_or(base.font_italic),
            font_kern_hundredths: run.run_font_kern_hundredths.or(base.font_kern_hundredths),
            text_caps: run.run_text_caps.or(base.text_caps),
            text_stroke_width_px: base.text_stroke_width_px,
            text_stroke_color: base.text_stroke_color,
            layout: base.layout,
            paragraphs: base.paragraphs,
            text_shadow: base.text_shadow,
            geom_rect_padding: base.geom_rect_padding,
            presentation_kinsoku: base.presentation_kinsoku,
            body_box_height_px: base.body_box_height_px,
            compat_line_spacing: base.compat_line_spacing,
            text_warp_preset: base.text_warp_preset,
            text_warp_adj: base.text_warp_adj,
            text_warp_adj2: base.text_warp_adj2,
        },
        scaled_size,
    )
}

fn display_run_text<I: StringIntern>(ctx: &PaintContext<'_, I>, run: &TextPaintRun) -> String {
    field_substitution::substitute_field_text(&run.text, run.field_type.as_deref(), &ctx.field_ctx)
}

#[allow(clippy::too_many_arguments)]
fn paint_bullet_marker<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    para: u32,
    marker: &str,
    text_indent_px: Option<f64>,
    paint: &TextBodyPaint<'_>,
    size: f64,
    stack: &str,
    bullet_color: Option<&str>,
) {
    let bullet = create_child(ctx, para, "span");
    set_attr(ctx, bullet, "data-pptx-bullet-marker", "");
    set_style(ctx, bullet, "display", "inline-block");
    set_style(ctx, bullet, "text-indent", "0px");
    if let Some(hang) = text_indent_px.filter(|v| *v < 0.0) {
        set_style(ctx, bullet, "min-width", &format!("{}px", format_px(-hang)));
    } else {
        set_style(ctx, bullet, "margin-inline-end", "0.35em");
    }
    set_text_content(ctx, bullet, marker);
    paint_run_typography(ctx, bullet, paint, size, stack);
    if let Some(color) = bullet_color {
        set_style(ctx, bullet, "color", color);
    }
}

#[allow(clippy::too_many_arguments)]
fn append_paint_run<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    para: u32,
    run_model: &TextPaintRun,
    paragraph_rtl: Option<bool>,
    tab_stops: &[TextPaintTabStop],
    default_tab_px: f64,
    paint: &TextBodyPaint<'_>,
    size: f64,
    paragraph_strut_size: f64,
    stack: &str,
    run_line_height_zero: bool,
    east_asian: Option<EastAsianBreakOptions>,
    following: Option<char>,
) {
    if run_model.line_break {
        create_child(ctx, para, "br");
        return;
    }
    if let Some(mathml) = &run_model.equation_mathml {
        paint_inline_equation_run(
            ctx,
            para,
            run_model,
            paragraph_rtl,
            paint,
            paragraph_strut_size,
            stack,
            run_line_height_zero,
            mathml,
        );
        return;
    }
    let display_text = display_run_text(ctx, run_model);
    // Upstream `buildParagraphRuns`: `if (!text) continue` — empty `<a:t/>` is not
    // painted (poi-with_japanese Rectangle 3 blank lines keep only the `a:br`).
    if display_text.is_empty() {
        return;
    }
    if run_model.hyperlink_click {
        paint_hyperlink_run(
            ctx,
            para,
            run_model,
            paragraph_rtl,
            paint,
            size,
            stack,
            run_line_height_zero,
            east_asian,
            following,
        );
        return;
    }
    if !tab_stops.is_empty() && display_text.contains('\t') {
        if tab_stops.len() == 1 && tab_stops[0].align == crate::text_layout::TextTabAlign::Left {
            paint_css_tab_run(
                ctx,
                para,
                run_model,
                &display_text,
                paragraph_rtl,
                tab_stops[0].position_px,
                paint,
                size,
                stack,
            );
        } else {
            paint_tabbed_run(
                ctx,
                para,
                run_model,
                &display_text,
                paragraph_rtl,
                tab_stops,
                default_tab_px,
                paint,
                size,
                stack,
            );
        }
        return;
    }
    let host = if let Some(ruby) = &run_model.ruby {
        let ruby_el = create_child(ctx, para, "ruby");
        set_style(ctx, ruby_el, "ruby-position", "over");
        set_style(ctx, ruby_el, "ruby-align", "center");
        apply_run_rtl(ctx, ruby_el, run_model.rtl, paragraph_rtl);
        let base = create_child(ctx, ruby_el, "span");
        set_text_content(ctx, base, &display_text);
        if paint.font_family.is_none() {
            set_style(ctx, base, "font-family", VISUAL_HOST_RUN_FONT_STACK);
        }
        paint_run_typography(ctx, base, paint, size, stack);
        if run_line_height_zero {
            set_style(ctx, base, "line-height", "0");
        }
        if run_model.underline {
            apply_run_underline(ctx, base);
        }
        if should_emit_gdi_tracking_for_run(&display_text, run_model.underline) {
            apply_gdi_metric_letter_spacing(
                ctx,
                base,
                &display_text,
                size,
                paint,
                stack,
                paint.font_bold,
                paint.font_italic,
            );
        }
        let open_rp = create_child(ctx, ruby_el, "rp");
        set_text_content(ctx, open_rp, "(");
        paint_ruby_annotation(ctx, ruby_el, ruby, size, stack);
        let close_rp = create_child(ctx, ruby_el, "rp");
        set_text_content(ctx, close_rp, ")");
        ruby_el
    } else if uses_per_piece_gdi_metric(paint, &display_text) {
        let pieces = split_run_for_metric_pieces(&display_text);
        if pieces.len() > 1 {
            for (piece_idx, piece) in pieces.iter().enumerate() {
                let piece_following = if piece_idx + 1 < pieces.len() {
                    pieces[piece_idx + 1].chars().next().or(following)
                } else {
                    following
                };
                paint_run_text_span(
                    ctx,
                    para,
                    piece,
                    run_model,
                    paragraph_rtl,
                    paint,
                    size,
                    stack,
                    run_line_height_zero,
                    east_asian,
                    piece_following,
                    true,
                );
            }
            para
        } else {
            paint_run_text_span(
                ctx,
                para,
                &display_text,
                run_model,
                paragraph_rtl,
                paint,
                size,
                stack,
                run_line_height_zero,
                east_asian,
                following,
                false,
            )
        }
    } else {
        paint_run_text_span(
            ctx,
            para,
            &display_text,
            run_model,
            paragraph_rtl,
            paint,
            size,
            stack,
            run_line_height_zero,
            east_asian,
            following,
            false,
        )
    };
    let _ = host;
}

fn uses_per_piece_gdi_metric(paint: &TextBodyPaint<'_>, text: &str) -> bool {
    if paint.layout.is_some_and(|l| l.wrap_none) {
        return false;
    }
    paint.font_family.is_some() && should_apply_gdi_metric(text)
}

fn is_metric_break_boundary(previous: char, next: char) -> bool {
    let previous_space = previous.is_whitespace();
    let next_space = next.is_whitespace();
    if previous_space != next_space {
        return true;
    }
    previous == '-' && next != '-' && !next_space
}

fn split_run_for_metric_pieces(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 2 {
        return vec![text.to_owned()];
    }
    let mut pieces: Vec<String> = Vec::new();
    let mut current = chars[0].to_string();
    for i in 1..chars.len() {
        if is_metric_break_boundary(chars[i - 1], chars[i]) {
            pieces.push(current);
            current = String::new();
        }
        current.push(chars[i]);
    }
    pieces.push(current);
    if pieces.len() == 1 {
        vec![text.to_owned()]
    } else {
        pieces
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_run_text_span<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    para: u32,
    text: &str,
    run_model: &TextPaintRun,
    paragraph_rtl: Option<bool>,
    paint: &TextBodyPaint<'_>,
    size: f64,
    stack: &str,
    run_line_height_zero: bool,
    east_asian: Option<EastAsianBreakOptions>,
    following: Option<char>,
    gdi_run_was_split: bool,
) -> u32 {
    let wrap_none = paint.layout.is_some_and(|l| l.wrap_none);
    // Upstream `splitStyledRun`: each metric piece carries its own tracking, even when the
    // piece is a single word (the one-piece skip only applies to an unsplit run).
    let emit_gdi_tracking = !wrap_none
        && if gdi_run_was_split {
            should_apply_gdi_metric(text)
        } else {
            should_emit_gdi_tracking_for_run(text, run_model.underline)
        };
    // Catches: CJK-only runs with a:ea ≠ a:latin re-entering paint_run_text_span forever
    // (needs_split stayed true because script_fonts was never cleared on the recursive call).
    if let Some(script_fonts) = &run_model.script_fonts {
        let script_runs = crate::unicode_script::segment_by_script(text);
        let needs_split = script_runs.len() > 1
            || script_runs.iter().any(|seg| {
                crate::unicode_script::resolve_font_for_script(seg.script, script_fonts)
                    != script_fonts.latin
            });
        if needs_split {
            let mut seg_model = run_model.clone();
            seg_model.script_fonts = None;
            let mut last = para;
            for (idx, seg) in script_runs.iter().enumerate() {
                let face = crate::unicode_script::resolve_font_for_script(seg.script, script_fonts);
                let seg_paint = TextBodyPaint {
                    text_color: paint.text_color,
                    font_size_px: paint.font_size_px,
                    font_family: face.as_deref().or(paint.font_family),
                    font_bold: paint.font_bold,
                    font_italic: paint.font_italic,
                    font_kern_hundredths: paint.font_kern_hundredths,
                    text_caps: paint.text_caps,
                    text_stroke_width_px: paint.text_stroke_width_px,
                    text_stroke_color: paint.text_stroke_color,
                    layout: paint.layout,
                    paragraphs: paint.paragraphs,
                    text_shadow: paint.text_shadow,
                    geom_rect_padding: paint.geom_rect_padding,
                    presentation_kinsoku: paint.presentation_kinsoku,
                    body_box_height_px: paint.body_box_height_px,
                    compat_line_spacing: paint.compat_line_spacing,
                    text_warp_preset: paint.text_warp_preset,
                    text_warp_adj: paint.text_warp_adj,
                    text_warp_adj2: paint.text_warp_adj2,
                };
                let seg_stack = substitute_font_family_stack(seg_paint.font_family);
                let seg_following = if idx + 1 < script_runs.len() {
                    script_runs[idx + 1].text.chars().next()
                } else {
                    following
                };
                last = paint_run_text_span(
                    ctx,
                    para,
                    &seg.text,
                    &seg_model,
                    paragraph_rtl,
                    &seg_paint,
                    size,
                    &seg_stack,
                    run_line_height_zero,
                    east_asian,
                    seg_following,
                    false,
                );
            }
            return last;
        }
    }
    let ea_ref = east_asian.as_ref();
    let pieces = split_east_asian_breaks(text, ea_ref, size, following);
    let single_plain = pieces.len() == 1
        && !pieces[0].inline_block_zero_width
        && pieces[0].word_spacing_px.is_none();
    if single_plain {
        let run = create_child(ctx, para, "span");
        set_text_content(ctx, run, &pieces[0].text);
        paint_run_span_styles(
            ctx,
            run,
            run_model,
            paragraph_rtl,
            paint,
            size,
            stack,
            run_line_height_zero,
            east_asian,
            &pieces[0].text,
            emit_gdi_tracking,
        );
        return run;
    }
    let mut last = para;
    for piece in pieces {
        let run = create_child(ctx, para, "span");
        set_text_content(ctx, run, &piece.text);
        if piece.inline_block_zero_width {
            set_style(ctx, run, "display", "inline-block");
            set_style(ctx, run, "inline-size", "0px");
        }
        if let Some(ws) = piece.word_spacing_px {
            if let Some(glyph) = piece.hang_glyph {
                apply_hang_space_word_spacing(
                    ctx,
                    run,
                    glyph,
                    size,
                    stack,
                    paint.font_bold,
                    paint.font_italic,
                    ws,
                );
            } else {
                set_style(ctx, run, "word-spacing", &format_layout_px(ws));
            }
        }
        paint_run_span_styles(
            ctx,
            run,
            run_model,
            paragraph_rtl,
            paint,
            size,
            stack,
            run_line_height_zero,
            east_asian,
            &piece.text,
            emit_gdi_tracking,
        );
        last = run;
    }
    last
}

#[allow(clippy::too_many_arguments)]
fn paint_run_span_styles<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    run: u32,
    run_model: &TextPaintRun,
    paragraph_rtl: Option<bool>,
    paint: &TextBodyPaint<'_>,
    size: f64,
    stack: &str,
    run_line_height_zero: bool,
    east_asian: Option<EastAsianBreakOptions>,
    metric_text: &str,
    emit_gdi_tracking: bool,
) {
    apply_run_rtl(ctx, run, run_model.rtl, paragraph_rtl);
    if paint.font_family.is_some() {
        set_style(ctx, run, "font-family", stack);
    } else {
        set_style(ctx, run, "font-family", VISUAL_HOST_RUN_FONT_STACK);
    }
    paint_run_typography(ctx, run, paint, size, stack);
    if let Some(shift) = run_model.baseline_shift {
        set_style(ctx, run, "vertical-align", shift);
    }
    if run_line_height_zero {
        set_style(ctx, run, "line-height", "0");
    }
    if crate::east_asian_breaks::is_korean_language(run_model.run_lang.as_deref()) {
        let wb = if east_asian.is_some_and(|o| o.latin_line_break) {
            "normal"
        } else {
            "keep-all"
        };
        set_style(ctx, run, "word-break", wb);
    }
    if run_model.underline {
        apply_run_underline(ctx, run);
    }
    if emit_gdi_tracking {
        apply_gdi_metric_letter_spacing(
            ctx,
            run,
            metric_text,
            size,
            paint,
            stack,
            paint.font_bold,
            paint.font_italic,
        );
    }
}

/// Upstream `splitStyledRun`: skip per-run GDI tracking when the run is one metric piece.
/// Upstream `splitStyledRun` keeps the whole-run `resolveMetricTrackingPx` letter-spacing on a
/// one-piece run, so every non-CJK run is tracked, not only multi-piece ones.
fn should_emit_gdi_tracking_for_run(text: &str, _underline: bool) -> bool {
    should_apply_gdi_metric(text)
}

fn should_apply_gdi_metric(text: &str) -> bool {
    let mut chars = 0u32;
    let mut east_asian = 0u32;
    for ch in text.chars() {
        if ch.is_whitespace() || ch == '\u{200B}' || ch == '\u{2060}' {
            continue;
        }
        chars += 1;
        if crate::east_asian_breaks::is_east_asian_char(ch) {
            east_asian += 1;
        }
    }
    chars > 0 && east_asian * 2 < chars
}

fn apply_run_rtl<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    run: u32,
    run_rtl: Option<bool>,
    paragraph_rtl: Option<bool>,
) {
    if let Some(run_rtl) = run_rtl {
        set_style(ctx, run, "direction", if run_rtl { "rtl" } else { "ltr" });
        let bidi = if Some(run_rtl) != paragraph_rtl {
            "bidi-override"
        } else {
            "embed"
        };
        set_style(ctx, run, "unicode-bidi", bidi);
    }
}

fn apply_run_underline<I: StringIntern>(ctx: &mut PaintContext<'_, I>, run: u32) {
    apply_run_underline_with_thickness(ctx, run, true);
}

/// Plain underlines carry `1px`; a hyperlink underline keeps the browser's font-scaled default.
fn apply_run_underline_with_thickness<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    run: u32,
    fixed_thickness: bool,
) {
    set_style(ctx, run, "text-decoration-line", "underline");
    set_style(ctx, run, "text-decoration-style", "solid");
    if fixed_thickness {
        set_style(ctx, run, "text-decoration-thickness", "1px");
    }
    set_style(ctx, run, "text-decoration-skip-ink", "none");
}

const HYPERLINK_COLOR: &str = "#0563C1";

#[allow(clippy::too_many_arguments)]
fn paint_inline_equation_run<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    para: u32,
    run_model: &TextPaintRun,
    paragraph_rtl: Option<bool>,
    paint: &TextBodyPaint<'_>,
    size: f64,
    stack: &str,
    run_line_height_zero: bool,
    mathml: &str,
) {
    let host = create_child(ctx, para, "span");
    set_attr(ctx, host, "class", "pptxv-inline-equation");
    apply_run_rtl(ctx, host, run_model.rtl, paragraph_rtl);
    paint_run_typography(ctx, host, paint, size, stack);
    if run_line_height_zero {
        set_style(ctx, host, "line-height", "0");
    }
    let math = create_child(ctx, host, "span");
    set_attr(ctx, math, "class", "pptxv-equation-math");
    set_style(ctx, math, "display", "inline-block");
    set_style(ctx, math, "vertical-align", "middle");
    set_style(
        ctx,
        math,
        "font-family",
        "'Cambria Math', 'STIX Two Math', serif",
    );
    set_style(ctx, math, "font-size", &format_layout_px(size));
    set_inner_markup_stub(ctx, math, mathml);
}

#[allow(clippy::too_many_arguments)]
fn paint_hyperlink_run<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    para: u32,
    run_model: &TextPaintRun,
    paragraph_rtl: Option<bool>,
    paint: &TextBodyPaint<'_>,
    size: f64,
    stack: &str,
    run_line_height_zero: bool,
    east_asian: Option<EastAsianBreakOptions>,
    following: Option<char>,
) {
    let link = create_child(ctx, para, "a");
    set_attr(ctx, link, "class", "pptxv-link");
    set_attr(ctx, link, "role", "link");
    set_attr(ctx, link, "data-pptx-hyperlink", "true");
    set_attr(ctx, link, "target", "_blank");
    set_attr(ctx, link, "rel", "noopener noreferrer");
    if let Some(tip) = &run_model.hyperlink_tooltip {
        set_attr(ctx, link, "title", tip);
    }
    set_style(ctx, link, "cursor", "pointer");
    apply_run_rtl(ctx, link, run_model.rtl, paragraph_rtl);
    if paint.font_family.is_none() {
        set_style(ctx, link, "font-family", VISUAL_HOST_RUN_FONT_STACK);
    } else {
        set_style(ctx, link, "font-family", stack);
    }
    paint_run_typography(ctx, link, paint, size, stack);
    // Upstream: the resolved run colour wins; `hlink` colour only when the cascade gave none.
    set_style(
        ctx,
        link,
        "color",
        paint.text_color.unwrap_or(HYPERLINK_COLOR),
    );
    apply_run_underline_with_thickness(ctx, link, false);
    if run_line_height_zero {
        set_style(ctx, link, "line-height", "0");
    }
    let display_text = display_run_text(ctx, run_model);
    paint_run_text_span(
        ctx,
        link,
        &display_text,
        run_model,
        paragraph_rtl,
        paint,
        size,
        stack,
        run_line_height_zero,
        east_asian,
        following,
        false,
    );
}

fn paint_ruby_annotation<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    ruby_el: u32,
    ruby: &TextPaintRuby,
    base_size_px: f64,
    stack: &str,
) {
    let rt = create_child(ctx, ruby_el, "rt");
    set_text_content(ctx, rt, &ruby.annotation);
    let rt_size = ruby.font_size_px.unwrap_or(base_size_px * 0.5);
    set_style(ctx, rt, "font-size", &format!("{}px", format_px(rt_size)));
    set_style(ctx, rt, "font-family", stack);
    if let Some(lang) = &ruby.lang {
        set_attr(ctx, rt, "lang", lang);
    }
    if let Some(align) = ruby.text_align {
        set_style(ctx, rt, "text-align", align);
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_css_tab_run<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    para: u32,
    run_model: &TextPaintRun,
    display_text: &str,
    paragraph_rtl: Option<bool>,
    tab_px: f64,
    paint: &TextBodyPaint<'_>,
    size: f64,
    stack: &str,
) {
    let run = create_child(ctx, para, "span");
    set_text_content(ctx, run, display_text);
    apply_run_rtl(ctx, run, run_model.rtl, paragraph_rtl);
    set_style(ctx, run, "white-space", "pre");
    set_style(ctx, run, "tab-size", &format_layout_px(tab_px));
    if paint.font_family.is_none() {
        set_style(ctx, run, "font-family", VISUAL_HOST_RUN_FONT_STACK);
    }
    paint_run_typography(ctx, run, paint, size, stack);
    if run_model.underline {
        apply_run_underline(ctx, run);
    }
    if should_emit_gdi_tracking_for_run(display_text, run_model.underline) {
        apply_gdi_metric_letter_spacing(
            ctx,
            run,
            display_text,
            size,
            paint,
            stack,
            paint.font_bold,
            paint.font_italic,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_tabbed_run<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    para: u32,
    run_model: &TextPaintRun,
    display_text: &str,
    paragraph_rtl: Option<bool>,
    tab_stops: &[TextPaintTabStop],
    default_tab_px: f64,
    paint: &TextBodyPaint<'_>,
    size: f64,
    stack: &str,
) {
    let line_host = create_child(ctx, para, "span");
    set_style(ctx, line_host, "display", "inline-block");
    set_style(ctx, line_host, "white-space", "nowrap");
    apply_run_rtl(ctx, line_host, run_model.rtl, paragraph_rtl);
    let canvas_font = canvas_measure_font(stack, size, paint.font_bold, paint.font_italic);
    let font_id = ctx.intern.intern(&canvas_font);
    let measure = |text: &str| -> f64 {
        if text.is_empty() {
            return 0.0;
        }
        // Distinct intern key from GDI tracking (-3) so tab layout keeps full widths.
        let width_key = ctx.intern.intern(&format!("tabw:{text}"));
        if let Some(w) = ctx.measure.cached_width(font_id, width_key) {
            let tracking_key = ctx.intern.intern(text);
            let tracking = ctx
                .measure
                .cached_width(font_id, tracking_key)
                .unwrap_or(0.0);
            let chars = text.chars().count();
            return if tracking.abs() > 1e-9 && chars > 0 {
                w + tracking * chars as f64
            } else {
                w
            };
        }
        ctx.measure.push(font_id, size as f32, width_key);
        text.len() as f64 * size * 0.5
    };
    let segments: Vec<&str> = display_text.split('\t').collect();
    let pieces = compute_tabbed_line_layout(&segments, tab_stops, measure, default_tab_px);
    for (i, (text, leader_width)) in pieces.iter().enumerate() {
        if i > 0 && *leader_width > 0.5 {
            let leader = create_child(ctx, line_host, "span");
            set_attr(ctx, leader, "aria-hidden", "true");
            set_style(ctx, leader, "display", "inline-block");
            set_style(ctx, leader, "width", &format_layout_px(*leader_width));
            set_style(ctx, leader, "overflow", "hidden");
            set_style(ctx, leader, "white-space", "nowrap");
            set_style(ctx, leader, "vertical-align", "baseline");
        }
        let piece = create_child(ctx, line_host, "span");
        set_text_content(ctx, piece, text);
        set_style(ctx, piece, "display", "inline-block");
        set_style(ctx, piece, "white-space", "pre");
        if paint.font_family.is_none() {
            set_style(ctx, piece, "font-family", VISUAL_HOST_RUN_FONT_STACK);
        }
        paint_run_typography(ctx, piece, paint, size, stack);
        if run_model.underline {
            apply_run_underline(ctx, piece);
        }
        if should_emit_gdi_tracking_for_run(text, run_model.underline) {
            apply_gdi_metric_letter_spacing(
                ctx,
                piece,
                text,
                size,
                paint,
                stack,
                paint.font_bold,
                paint.font_italic,
            );
        }
    }
}

/// One `<p class="pptxv-para">` per body (upstream `renderTextBlock`).
#[allow(dead_code)]
fn append_paragraph_text<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    block: u32,
    text: &str,
    text_align: Option<TextAlignCss>,
    paragraph_rtl: Option<bool>,
) -> (u32, u32) {
    let para = create_child(ctx, block, "p");
    set_attr(ctx, para, "class", "pptxv-para");
    set_style(ctx, para, "margin", "0");
    if let Some(align) = text_align {
        set_style(ctx, para, "text-align", align.as_css());
    }
    if let Some(rtl) = paragraph_rtl {
        set_style(ctx, para, "direction", if rtl { "rtl" } else { "ltr" });
        set_style(ctx, para, "unicode-bidi", "embed");
    } else {
        set_style(ctx, para, "direction", "ltr");
        set_style(ctx, para, "unicode-bidi", "embed");
    }
    let run = create_child(ctx, para, "span");
    set_text_content(ctx, run, text);
    (para, run)
}

fn apply_plain_body_kinsoku<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    block: u32,
    paint: &TextBodyPaint<'_>,
) {
    if paint.presentation_kinsoku {
        set_style(ctx, block, "line-break", "strict");
        set_style(ctx, block, "word-break", "normal");
    }
}

/// Per-paragraph kinsoku CSS (upstream `getKinsokuLineBreakStyles`).
fn apply_paragraph_kinsoku_line_break<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    para: u32,
    east_asian: Option<EastAsianBreakOptions>,
    presentation_kinsoku: bool,
) {
    if east_asian.is_some_and(|o| o.latin_line_break) {
        set_style(ctx, para, "word-break", "break-all");
        set_style(ctx, para, "overflow-wrap", "break-word");
    }
    if east_asian.is_some_and(|o| o.break_anywhere) {
        // Upstream `getKinsokuLineBreakStyles` only sets overflow-wrap here; ZWSP carries breaks.
        set_style(ctx, para, "overflow-wrap", "break-word");
        return;
    }
    if east_asian.is_some() || presentation_kinsoku {
        set_style(ctx, para, "line-break", "strict");
        set_style(ctx, para, "word-break", "normal");
        set_style(ctx, para, "overflow-wrap", "break-word");
    }
}

fn canvas_measure_font(family_stack: &str, size_px: f64, bold: bool, italic: bool) -> String {
    let mut prefix = String::new();
    if italic {
        prefix.push_str("italic ");
    }
    if bold {
        prefix.push_str("bold ");
    }
    format!("{}{}px {}", prefix, format_px(size_px), family_stack)
}

#[allow(clippy::too_many_arguments)]
fn apply_gdi_metric_letter_spacing<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    para: u32,
    text: &str,
    size_px: f64,
    _paint: &TextBodyPaint<'_>,
    painted_stack: &str,
    bold: bool,
    italic: bool,
) {
    if text.is_empty() {
        return;
    }
    // Upstream `resolveMetricTrackingPx` measures the run's own resolved font family.
    let _ = _paint;
    apply_gdi_metric_letter_spacing_with_stack(
        ctx,
        para,
        text,
        size_px,
        painted_stack,
        bold,
        italic,
    );
}

#[allow(clippy::too_many_arguments)]
fn apply_hang_space_word_spacing<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    run: u32,
    glyph: char,
    size_px: f64,
    stack: &str,
    bold: bool,
    italic: bool,
    fallback_px: f64,
) {
    let glyph_s = glyph.to_string();
    let canvas_font = canvas_measure_font(stack, size_px, bold, italic);
    let font_id = ctx.intern.intern(&canvas_font);
    // Distinct intern key so width/GDI caches for the same glyph are not reused.
    let hang_key = ctx.intern.intern(&format!("pptx-hang-space:{glyph_s}"));
    if let Some(ws) = ctx.measure.cached_width(font_id, hang_key) {
        if ws.abs() > 1e-9 {
            set_style(ctx, run, "word-spacing", &format_layout_px(ws));
        }
        return;
    }
    set_style(ctx, run, "word-spacing", &format_layout_px(fallback_px));
    ctx.measure.push(font_id, SIZE_PPTX_HANG_SPACE, hang_key);
}

fn apply_gdi_metric_letter_spacing_with_stack<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    para: u32,
    text: &str,
    size_px: f64,
    stack: &str,
    bold: bool,
    italic: bool,
) {
    if text.is_empty() {
        return;
    }
    let canvas_font = canvas_measure_font(stack, size_px, bold, italic);
    let font_id = ctx.intern.intern(&canvas_font);
    let text_id = ctx.intern.intern(text);
    if let Some(tracking) = ctx.measure.cached_width(font_id, text_id) {
        if tracking.abs() > 1e-9 {
            set_style(ctx, para, "letter-spacing", &format_tracking_px(tracking));
        }
        return;
    }
    ctx.measure
        .push(font_id, SIZE_PPTX_METRIC_TRACKING, text_id);
}

fn format_tracking_px(tracking: f64) -> String {
    if tracking.abs() < 1e-9 {
        return "0".to_owned();
    }
    let limit = 1.0 / 12.0;
    let clamped = tracking.clamp(-limit, limit);
    format!("{:.6}px", clamped)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

fn apply_geom_rect_padding_only<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    block: u32,
    rect: TextBodyRectPadding,
) {
    set_style(ctx, block, "width", "100%");
    set_style(ctx, block, "height", "100%");
    // CSS lengths need a unit; bare numbers are ignored by the browser.
    set_style(
        ctx,
        block,
        "padding-top",
        &format!("{}px", format_px(rect.top)),
    );
    set_style(
        ctx,
        block,
        "padding-bottom",
        &format!("{}px", format_px(rect.bottom)),
    );
    set_style(
        ctx,
        block,
        "padding-left",
        &format!("{}px", format_px(rect.left)),
    );
    set_style(
        ctx,
        block,
        "padding-right",
        &format!("{}px", format_px(rect.right)),
    );
}

fn apply_text_body_layout<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    block: u32,
    layout: &TextBodyLayout,
    rect: TextBodyRectPadding,
    per_paragraph_align: bool,
    italic_inset_slop: f64,
) {
    if layout.column_count > 1 {
        set_style(ctx, block, "display", "block");
        set_style(ctx, block, "column-count", &layout.column_count.to_string());
        set_style(
            ctx,
            block,
            "column-gap",
            &format_layout_px(layout.column_gap_px),
        );
        set_style(ctx, block, "column-fill", "auto");
        let align_content = match layout.v_align {
            TextBodyVAlign::Top => "start",
            TextBodyVAlign::Middle => "center",
            TextBodyVAlign::Bottom => "end",
        };
        set_style(ctx, block, "align-content", align_content);
    } else {
        let justify = match layout.v_align {
            TextBodyVAlign::Top => "flex-start",
            TextBodyVAlign::Middle => "center",
            TextBodyVAlign::Bottom => "flex-end",
        };
        set_style(ctx, block, "justify-content", justify);
    }
    if layout.anchor_center && layout.column_count <= 1 {
        set_style(ctx, block, "width", "fit-content");
        set_style(ctx, block, "max-width", "100%");
        set_style(ctx, block, "margin-left", "auto");
        set_style(ctx, block, "margin-right", "auto");
    }
    if layout.overflow_clip {
        set_style(ctx, block, "overflow", "hidden");
    }
    if layout.wrap_none && !layout.overflow_clip {
        set_style(ctx, block, "white-space", "nowrap");
        set_style(ctx, block, "overflow", "visible");
    }
    if let Some(deg) = layout.body_rotation_deg.filter(|d| *d != 0.0) {
        set_style(
            ctx,
            block,
            "transform",
            &format!("rotate({}deg)", format_px(deg)),
        );
        set_style(ctx, block, "transform-origin", "center");
    }
    if !per_paragraph_align {
        if let Some(align) = layout.text_align {
            set_style(ctx, block, "text-align", align.as_css());
        }
    }
    // Upstream `toPxLengths`: inset + text-rect padding must be `Npx`, not bare `N`.
    set_style(
        ctx,
        block,
        "padding-top",
        &format!(
            "{}px",
            format_px(layout.inset_top_px + rect.top + italic_inset_slop)
        ),
    );
    set_style(
        ctx,
        block,
        "padding-bottom",
        &format!(
            "{}px",
            format_px(layout.inset_bottom_px + rect.bottom + italic_inset_slop)
        ),
    );
    set_style(
        ctx,
        block,
        "padding-left",
        &format!("{}px", format_px(layout.inset_left_px + rect.left)),
    );
    set_style(
        ctx,
        block,
        "padding-right",
        &format!("{}px", format_px(layout.inset_right_px + rect.right)),
    );
}

fn format_layout_px(v: f64) -> String {
    if !v.is_finite() || v.abs() < 1e-9 {
        return "0px".to_owned();
    }
    let s = format!("{:.4}", v)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned();
    format!("{}px", s)
}

fn format_px(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_owned();
    }
    let rounded = v.round();
    if (v - rounded).abs() < 1e-6 {
        format!("{}", rounded as i64)
    } else {
        format!("{}", v)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use nitro_xml_render::measure::{LayoutReadBatch, MeasureBatch, OutlineBatch};
    use nitro_xml_render::CommandBuffer;

    use super::super::context::{PaintContext, StringIntern};
    use super::super::view_ir::ViewIrCollector;
    use super::super::FieldSubstitutionContext;
    use crate::text_body_rect::TextBodyRectPadding;

    use super::{paint_text_body, TextBodyPaint};

    struct Intern(Vec<String>);

    impl StringIntern for Intern {
        fn intern(&mut self, s: &str) -> u32 {
            if let Some(i) = self.0.iter().position(|existing| existing == s) {
                return i as u32;
            }
            self.0.push(s.to_owned());
            (self.0.len() - 1) as u32
        }
    }

    fn paint_with(paint: TextBodyPaint<'_>) -> Intern {
        paint_with_measure(paint, MeasureBatch::default(), None, "Hi")
    }

    fn paint_with_measure(
        paint: TextBodyPaint<'_>,
        mut measure: MeasureBatch,
        intern_seed: Option<Intern>,
        text: &str,
    ) -> Intern {
        // Keep the caller's MeasureBatch (tests seed GDI tracking cache entries).
        let mut buffer = CommandBuffer::new();
        let mut intern = intern_seed.unwrap_or_else(|| Intern(Vec::new()));
        let mut outlines = OutlineBatch::default();
        let mut layout_reads = LayoutReadBatch::default();
        let mut view_ir = ViewIrCollector::default();
        let scheme = BTreeMap::new();
        let mut sp_autofit = crate::render::SpAutofitPaintState::default();
        let slide_autofit = sp_autofit.slide_mut(0);
        let mut ctx = PaintContext::new(
            &mut buffer,
            &mut intern,
            &mut measure,
            &mut outlines,
            &mut layout_reads,
            &mut view_ir,
            1.0,
            960.0,
            540.0,
            &scheme,
            0,
            FieldSubstitutionContext::default(),
            slide_autofit,
        );
        paint_text_body(&mut ctx, 1, text, paint);
        intern
    }

    #[test]
    fn text_outline_without_solid_color_uses_current_color() {
        // Catches: Selamat-like ln with no solid stroke color never gets currentColor text-stroke.
        let intern = paint_with(TextBodyPaint {
            text_color: None,
            font_size_px: Some(16.0),
            font_family: None,
            font_bold: false,
            font_italic: false,
            font_kern_hundredths: None,
            text_caps: None,
            text_stroke_width_px: Some(2.0),
            text_stroke_color: None,
            layout: None,
            paragraphs: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
            presentation_kinsoku: false,
            body_box_height_px: None,
            text_warp_preset: None,
            text_warp_adj: None,
            text_warp_adj2: None,
            compat_line_spacing: false,
        });
        assert!(
            intern.0.iter().any(|s| s == "2px currentColor"),
            "stroke value, got {:?}",
            intern.0
        );
        assert!(intern.0.iter().any(|s| s == "paint-order"));
        assert!(intern.0.iter().any(|s| s == "stroke fill"));
    }

    #[test]
    fn text_outline_with_color_uses_explicit_hex() {
        let intern = paint_with(TextBodyPaint {
            text_color: None,
            font_size_px: Some(16.0),
            font_family: None,
            font_bold: false,
            font_italic: false,
            font_kern_hundredths: None,
            text_caps: None,
            text_stroke_width_px: Some(3.31),
            text_stroke_color: Some("#000000"),
            layout: None,
            paragraphs: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
            presentation_kinsoku: false,
            body_box_height_px: None,
            text_warp_preset: None,
            text_warp_adj: None,
            text_warp_adj2: None,
            compat_line_spacing: false,
        });
        assert!(
            intern.0.iter().any(|s| s == "3.31px #000000"),
            "stroke value, got {:?}",
            intern.0
        );
    }

    #[test]
    fn text_body_default_line_height_matches_upstream() {
        // Catches: missing line-height: 1.2 on pptxv-text blocks (DEFAULT_LINE_HEIGHT).
        let intern = paint_with(TextBodyPaint {
            text_color: None,
            font_size_px: Some(16.0),
            font_family: None,
            font_bold: false,
            font_italic: false,
            font_kern_hundredths: None,
            text_caps: None,
            text_stroke_width_px: None,
            text_stroke_color: None,
            layout: None,
            paragraphs: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
            presentation_kinsoku: false,
            body_box_height_px: None,
            text_warp_preset: None,
            text_warp_adj: None,
            text_warp_adj2: None,
            compat_line_spacing: false,
        });
        assert!(
            intern.0.iter().any(|s| s == "1.2"),
            "line-height, got {:?}",
            intern.0
        );
    }

    #[test]
    fn informal_roman_font_stack_not_calibri_prefixed() {
        // Catches: Informal Roman painted with Calibri/Carlito fallback chain.
        let intern = paint_with(TextBodyPaint {
            text_color: None,
            font_size_px: Some(16.0),
            font_family: Some("Informal Roman"),
            font_bold: false,
            font_italic: false,
            font_kern_hundredths: None,
            text_caps: None,
            text_stroke_width_px: None,
            text_stroke_color: None,
            layout: None,
            paragraphs: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
            presentation_kinsoku: false,
            body_box_height_px: None,
            text_warp_preset: None,
            text_warp_adj: None,
            text_warp_adj2: None,
            compat_line_spacing: false,
        });
        assert!(
            intern
                .0
                .iter()
                .any(|s| s == "\"Informal Roman\", \"Informal\", sans-serif"),
            "font-family stack, got {:?}",
            intern.0
        );
    }

    #[test]
    fn wrap_none_body_skips_gdi_metric_queue() {
        // Catches: wrap="none" long lines still getting per-word GDI tracking and drifting vs TS at DPR1.
        use crate::text_layout::{TextBodyLayout, TextBodyVAlign};
        let layout = TextBodyLayout {
            v_align: TextBodyVAlign::Top,
            text_align: None,
            inset_top_px: 0.0,
            inset_bottom_px: 0.0,
            inset_left_px: 0.0,
            inset_right_px: 0.0,
            column_count: 1,
            column_gap_px: 0.0,
            anchor_center: false,
            overflow_clip: false,
            body_rotation_deg: None,
            wrap_none: true,
        };
        let intern = paint_with(TextBodyPaint {
            text_color: None,
            font_size_px: Some(16.0),
            font_family: Some("Arial"),
            font_bold: false,
            font_italic: false,
            font_kern_hundredths: None,
            text_caps: None,
            text_stroke_width_px: None,
            text_stroke_color: None,
            layout: Some(&layout),
            paragraphs: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
            presentation_kinsoku: false,
            body_box_height_px: None,
            text_warp_preset: None,
            text_warp_adj: None,
            text_warp_adj2: None,
            compat_line_spacing: false,
        });
        assert!(
            intern.0.iter().any(|s| s == "nowrap"),
            "white-space nowrap, got {:?}",
            intern.0
        );
        assert!(
            !intern.0.iter().any(|s| s.contains("letter-spacing")),
            "wrap none must not queue GDI, got {:?}",
            intern.0
        );
    }

    #[test]
    fn kinsoku_uses_overflow_wrap_not_legacy_word_break() {
        // Catches: `word-break: break-word` on the body splitting long Latin labels on different columns than upstream.
        let intern = paint_with(TextBodyPaint {
            text_color: None,
            font_size_px: Some(16.0),
            font_family: None,
            font_bold: false,
            font_italic: false,
            font_kern_hundredths: None,
            text_caps: None,
            text_stroke_width_px: None,
            text_stroke_color: None,
            layout: None,
            paragraphs: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
            presentation_kinsoku: true,
            body_box_height_px: None,
            text_warp_preset: None,
            text_warp_adj: None,
            text_warp_adj2: None,
            compat_line_spacing: false,
        });
        assert!(
            intern.0.iter().any(|s| s == "overflow-wrap"),
            "overflow-wrap prop, got {:?}",
            intern.0
        );
        assert!(
            intern.0.iter().any(|s| s == "break-word"),
            "overflow-wrap value, got {:?}",
            intern.0
        );
        assert!(
            intern
                .0
                .iter()
                .any(|s| s == "word-break" && intern.0.iter().any(|v| v == "normal")),
            "word-break normal, got {:?}",
            intern.0
        );
        let mut legacy_word_break = false;
        for i in 0..intern.0.len().saturating_sub(1) {
            if intern.0[i] == "word-break" && intern.0[i + 1] == "break-word" {
                legacy_word_break = true;
            }
        }
        assert!(
            !legacy_word_break,
            "legacy word-break:break-word must not be used, got {:?}",
            intern.0
        );
    }

    #[test]
    fn plain_text_runs_live_in_span_inside_pptxv_para() {
        // Catches: metric tracking and run typography on <p> instead of an inner <span> like upstream run paint.
        let intern = paint_with(TextBodyPaint {
            text_color: Some("#000000"),
            font_size_px: Some(16.0),
            font_family: None,
            font_bold: false,
            font_italic: false,
            font_kern_hundredths: None,
            text_caps: None,
            text_stroke_width_px: None,
            text_stroke_color: None,
            layout: None,
            paragraphs: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
            presentation_kinsoku: false,
            body_box_height_px: None,
            text_warp_preset: None,
            text_warp_adj: None,
            text_warp_adj2: None,
            compat_line_spacing: false,
        });
        assert!(intern.0.iter().any(|s| s == "span"));
        assert!(intern.0.iter().any(|s| s == "font-weight"));
        assert!(intern.0.iter().any(|s| s == "normal"));
    }

    #[test]
    fn text_body_wraps_plain_text_in_pptxv_para() {
        // Catches: plain text painted as a flex-child text node instead of <p class="pptxv-para">.
        let intern = paint_with(TextBodyPaint {
            text_color: None,
            font_size_px: Some(16.0),
            font_family: None,
            font_bold: false,
            font_italic: false,
            font_kern_hundredths: None,
            text_caps: None,
            text_stroke_width_px: None,
            text_stroke_color: None,
            layout: None,
            paragraphs: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
            presentation_kinsoku: false,
            body_box_height_px: None,
            text_warp_preset: None,
            text_warp_adj: None,
            text_warp_adj2: None,
            compat_line_spacing: false,
        });
        assert!(intern.0.iter().any(|s| s == "pptxv-para"));
        assert!(intern.0.iter().any(|s| s == "p"));
    }

    #[test]
    fn cached_gdi_tracking_becomes_letter_spacing_on_para() {
        // Catches: measure batch queued for pptx text but letter-spacing never applied on paint.
        let mut measure = MeasureBatch::default();
        let mut intern = Intern(Vec::new());
        let canvas_font = format!("24px {}", super::VISUAL_HOST_RUN_FONT_STACK);
        let font_id = intern.intern(&canvas_font);
        let text_id = intern.intern("Hi");
        measure.cache.insert((font_id, text_id), -0.02);
        let intern = paint_with_measure(
            TextBodyPaint {
                text_color: None,
                font_size_px: Some(24.0),
                font_family: Some("Arial"),
                font_bold: false,
                font_italic: false,
                font_kern_hundredths: None,
                text_caps: None,
                text_stroke_width_px: None,
                text_stroke_color: None,
                layout: None,
                paragraphs: None,
                text_shadow: None,
                geom_rect_padding: TextBodyRectPadding::default(),
                presentation_kinsoku: false,
                body_box_height_px: None,
                text_warp_preset: None,
                text_warp_adj: None,
                text_warp_adj2: None,
                compat_line_spacing: false,
            },
            measure,
            Some(intern),
            "Hi there",
        );
        assert!(
            intern.0.iter().any(|s| s == "letter-spacing"),
            "prop, got {:?}",
            intern.0
        );
        assert!(
            intern
                .0
                .iter()
                .any(|s| s.contains("px") && s.starts_with("-0.02")),
            "tracking value, got {:?}",
            intern.0
        );
    }

    #[test]
    fn text_body_layout_omits_border_box_sizing() {
        // Catches: border-box on `.pptxv-text` shrinking the wrap column inside default body insets.
        use crate::text_layout::{TextAlignCss, TextBodyLayout, TextBodyVAlign};
        let layout = TextBodyLayout {
            v_align: TextBodyVAlign::Top,
            text_align: Some(TextAlignCss::Left),
            inset_top_px: 4.8,
            inset_bottom_px: 4.8,
            inset_left_px: 9.6,
            inset_right_px: 9.6,
            ..Default::default()
        };
        let intern = paint_with(TextBodyPaint {
            text_color: None,
            font_size_px: Some(16.0),
            font_family: None,
            font_bold: false,
            font_italic: false,
            font_kern_hundredths: None,
            text_caps: None,
            text_stroke_width_px: None,
            text_stroke_color: None,
            layout: Some(&layout),
            paragraphs: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
            presentation_kinsoku: false,
            body_box_height_px: None,
            text_warp_preset: None,
            text_warp_adj: None,
            text_warp_adj2: None,
            compat_line_spacing: false,
        });
        assert!(
            !intern.0.iter().any(|s| s == "border-box"),
            "text body must use content-box like upstream, got {:?}",
            intern.0
        );
    }

    #[test]
    fn text_body_padding_uses_px_units() {
        // Catches: body insets painted as bare numbers (browser drops them) instead of Npx.
        use crate::text_layout::{TextAlignCss, TextBodyLayout, TextBodyVAlign};
        let layout = TextBodyLayout {
            v_align: TextBodyVAlign::Top,
            text_align: Some(TextAlignCss::Left),
            inset_top_px: 4.8,
            inset_bottom_px: 4.8,
            inset_left_px: 9.6,
            inset_right_px: 9.6,
            ..Default::default()
        };
        let intern = paint_with(TextBodyPaint {
            text_color: None,
            font_size_px: Some(16.0),
            font_family: None,
            font_bold: false,
            font_italic: false,
            font_kern_hundredths: None,
            text_caps: None,
            text_stroke_width_px: None,
            text_stroke_color: None,
            layout: Some(&layout),
            paragraphs: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
            presentation_kinsoku: false,
            body_box_height_px: None,
            text_warp_preset: None,
            text_warp_adj: None,
            text_warp_adj2: None,
            compat_line_spacing: false,
        });
        assert!(
            intern.0.iter().any(|s| s == "9.6px"),
            "horizontal padding with px, got {:?}",
            intern.0
        );
        assert!(
            intern.0.iter().any(|s| s == "4.8px"),
            "vertical padding with px, got {:?}",
            intern.0
        );
        assert!(
            !intern.0.iter().any(|s| s == "9.6" || s == "4.8"),
            "bare inset numbers must not be used as CSS lengths, got {:?}",
            intern.0
        );
    }

    #[test]
    fn default_font_size_is_24px() {
        // Catches: paint defaulting missing font size to 18px instead of upstream 24px (18pt).
        let intern = paint_with(TextBodyPaint {
            text_color: None,
            font_size_px: None,
            font_family: None,
            font_bold: false,
            font_italic: false,
            font_kern_hundredths: None,
            text_caps: None,
            text_stroke_width_px: None,
            text_stroke_color: None,
            layout: None,
            paragraphs: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
            presentation_kinsoku: false,
            body_box_height_px: None,
            text_warp_preset: None,
            text_warp_adj: None,
            text_warp_adj2: None,
            compat_line_spacing: false,
        });
        assert!(
            intern.0.iter().any(|s| s == "24px"),
            "font-size, got {:?}",
            intern.0
        );
    }
}
