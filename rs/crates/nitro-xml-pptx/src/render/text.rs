// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/text-block.ts,
// src/ui/src/pptx/render/text-effects.ts; changes in PROVENANCE.md.

use nitro_xml_render::measure::LayoutReadKind;

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_style, set_text_content};
use super::font_substitution::substitute_font_family_stack;
use super::shape_autofit_resize::queue_sp_autofit_scroll_height;
use crate::slide::DumpElement;
use crate::text_body_rect::{resolve_text_body_rect_padding, TextBodyRectPadding};
use crate::text_layout::{TextBodyLayout, TextBodyVAlign};

/// Loader `fillMeasureWidths` sentinel: returns GDI letter-spacing px/char (upstream `text-metric-tracking`).
const SIZE_PPTX_METRIC_TRACKING: f32 = -3.0;

#[derive(Clone, Copy)]
pub struct TextBodyPaint<'a> {
    pub text_color: Option<&'a str>,
    pub font_size_px: Option<f64>,
    pub font_family: Option<&'a str>,
    pub font_bold: bool,
    pub text_stroke_width_px: Option<f64>,
    pub text_stroke_color: Option<&'a str>,
    pub layout: Option<&'a TextBodyLayout>,
    pub text_shadow: Option<&'a str>,
    pub geom_rect_padding: TextBodyRectPadding,
}

impl<'a> TextBodyPaint<'a> {
    pub fn from_element(el: &'a DumpElement) -> Self {
        Self {
            text_color: el.text_color.as_deref(),
            font_size_px: el.font_size_px,
            font_family: el.font_family.as_deref(),
            font_bold: el.font_bold,
            text_stroke_width_px: el.text_stroke_width_px,
            text_stroke_color: el.text_stroke_color.as_deref(),
            layout: el.text_body_layout.as_ref(),
            text_shadow: el.text_shadow.as_deref(),
            geom_rect_padding: resolve_text_body_rect_padding(
                el.shape_type.as_deref(),
                el.width,
                el.height,
                None,
            ),
        }
    }
}

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let painted_h = ctx.sp_autofit.painted_height(&el.id, el.height);
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, painted_h);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "text");
    let text = el.text.as_deref().unwrap_or("");
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

pub fn paint_text_body<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    parent: u32,
    text: &str,
    paint: TextBodyPaint<'_>,
) -> u32 {
    let block = create_child(ctx, parent, "div");
    set_attr(ctx, block, "class", "pptxv-text");
    set_style(ctx, block, "white-space", "pre-wrap");
    set_style(ctx, block, "word-break", "break-word");
    if paint.layout.is_some() {
        set_style(ctx, block, "box-sizing", "border-box");
        set_style(ctx, block, "width", "100%");
        set_style(ctx, block, "height", "100%");
        set_style(ctx, block, "display", "flex");
        set_style(ctx, block, "flex-direction", "column");
    }
    if let Some(layout) = paint.layout {
        apply_text_body_layout(ctx, block, layout, paint.geom_rect_padding);
    } else if paint.geom_rect_padding != TextBodyRectPadding::default() {
        apply_geom_rect_padding_only(ctx, block, paint.geom_rect_padding);
    }
    let size = paint.font_size_px.unwrap_or(24.0).max(1.0);
    let stack = substitute_font_family_stack(paint.font_family);
    set_style(ctx, block, "font-family", &stack);
    set_style(ctx, block, "font-size", &format!("{}px", format_px(size)));
    set_style(ctx, block, "line-height", "1.2");
    if paint.font_bold {
        set_style(ctx, block, "font-weight", "700");
    }
    if let Some(color) = paint.text_color {
        set_style(ctx, block, "color", color);
    }
    if let Some(width) = paint.text_stroke_width_px.filter(|w| *w > 0.0) {
        let stroke_color = paint.text_stroke_color.unwrap_or("currentColor");
        set_style(
            ctx,
            block,
            "-webkit-text-stroke",
            &format!("{}px {}", format_px(width), stroke_color),
        );
        set_style(ctx, block, "paint-order", "stroke fill");
    }
    if let Some(shadow) = paint.text_shadow {
        set_style(ctx, block, "text-shadow", shadow);
    }
    set_style(ctx, block, "overflow", "visible");
    set_style(ctx, block, "text-spacing-trim", "space-all");
    let para = append_paragraph_text(ctx, block, text);
    apply_gdi_metric_letter_spacing(ctx, para, text, size, paint.font_family, paint.font_bold);
    block
}

/// One `<p class="pptxv-para">` per body (upstream `renderTextBlock`).
fn append_paragraph_text<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    block: u32,
    text: &str,
) -> u32 {
    let para = create_child(ctx, block, "p");
    set_attr(ctx, para, "class", "pptxv-para");
    set_style(ctx, para, "margin", "0");
    set_text_content(ctx, para, text);
    para
}

fn canvas_measure_font(family_stack: &str, size_px: f64, bold: bool) -> String {
    if bold {
        format!("bold {}px {}", format_px(size_px), family_stack)
    } else {
        format!("{}px {}", format_px(size_px), family_stack)
    }
}

fn apply_gdi_metric_letter_spacing<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    para: u32,
    text: &str,
    size_px: f64,
    font_family: Option<&str>,
    bold: bool,
) {
    if text.is_empty() {
        return;
    }
    let stack = substitute_font_family_stack(font_family);
    let canvas_font = canvas_measure_font(&stack, size_px, bold);
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
    set_style(ctx, block, "box-sizing", "border-box");
    set_style(ctx, block, "width", "100%");
    set_style(ctx, block, "height", "100%");
    set_style(ctx, block, "padding-top", &format_px(rect.top));
    set_style(ctx, block, "padding-bottom", &format_px(rect.bottom));
    set_style(ctx, block, "padding-left", &format_px(rect.left));
    set_style(ctx, block, "padding-right", &format_px(rect.right));
}

fn apply_text_body_layout<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    block: u32,
    layout: &TextBodyLayout,
    rect: TextBodyRectPadding,
) {
    let justify = match layout.v_align {
        TextBodyVAlign::Top => "flex-start",
        TextBodyVAlign::Middle => "center",
        TextBodyVAlign::Bottom => "flex-end",
    };
    set_style(ctx, block, "justify-content", justify);
    if let Some(align) = layout.text_align {
        set_style(ctx, block, "text-align", align.as_css());
    }
    set_style(
        ctx,
        block,
        "padding-top",
        &format_px(layout.inset_top_px + rect.top),
    );
    set_style(
        ctx,
        block,
        "padding-bottom",
        &format_px(layout.inset_bottom_px + rect.bottom),
    );
    set_style(
        ctx,
        block,
        "padding-left",
        &format_px(layout.inset_left_px + rect.left),
    );
    set_style(
        ctx,
        block,
        "padding-right",
        &format_px(layout.inset_right_px + rect.right),
    );
}

fn format_px(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use nitro_xml_render::measure::{LayoutReadBatch, MeasureBatch};
    use nitro_xml_render::CommandBuffer;

    use super::super::context::{PaintContext, StringIntern};
    use super::super::view_ir::ViewIrCollector;
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
        paint_with_measure(paint, MeasureBatch::default(), None)
    }

    fn paint_with_measure(
        paint: TextBodyPaint<'_>,
        mut measure: MeasureBatch,
        intern_seed: Option<Intern>,
    ) -> Intern {
        let mut buffer = CommandBuffer::new();
        let mut intern = intern_seed.unwrap_or_else(|| Intern(Vec::new()));
        let mut layout_reads = LayoutReadBatch::default();
        let mut view_ir = ViewIrCollector::default();
        let scheme = BTreeMap::new();
        let mut sp_autofit = crate::render::SpAutofitPaintState::default();
        let slide_autofit = sp_autofit.slide_mut(0);
        let mut ctx = PaintContext::new(
            &mut buffer,
            &mut intern,
            &mut measure,
            &mut layout_reads,
            &mut view_ir,
            1.0,
            960.0,
            540.0,
            &scheme,
            0,
            slide_autofit,
        );
        paint_text_body(&mut ctx, 1, "Hi", paint);
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
            text_stroke_width_px: Some(2.0),
            text_stroke_color: None,
            layout: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
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
            text_stroke_width_px: Some(3.31),
            text_stroke_color: Some("#000000"),
            layout: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
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
            text_stroke_width_px: None,
            text_stroke_color: None,
            layout: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
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
            text_stroke_width_px: None,
            text_stroke_color: None,
            layout: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
        });
        assert!(
            intern
                .0
                .iter()
                .any(|s| s == "\"Informal Roman\", sans-serif"),
            "font-family stack, got {:?}",
            intern.0
        );
    }

    #[test]
    fn text_body_wraps_plain_text_in_pptxv_para() {
        // Catches: plain text painted as a flex-child text node instead of <p class="pptxv-para">.
        let intern = paint_with(TextBodyPaint {
            text_color: None,
            font_size_px: Some(16.0),
            font_family: None,
            font_bold: false,
            text_stroke_width_px: None,
            text_stroke_color: None,
            layout: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
        });
        assert!(intern.0.iter().any(|s| s == "pptxv-para"));
        assert!(intern.0.iter().any(|s| s == "p"));
    }

    #[test]
    fn cached_gdi_tracking_becomes_letter_spacing_on_para() {
        // Catches: measure batch queued for pptx text but letter-spacing never applied on paint.
        let mut measure = MeasureBatch::default();
        let mut intern = Intern(Vec::new());
        let stack = super::super::font_substitution::substitute_font_family_stack(None);
        let canvas_font = format!("24px {}", stack);
        let font_id = intern.intern(&canvas_font);
        let text_id = intern.intern("Hi");
        measure.cache.insert((font_id, text_id), -0.02);
        let intern = paint_with_measure(
            TextBodyPaint {
                text_color: None,
                font_size_px: Some(24.0),
                font_family: None,
                font_bold: false,
                text_stroke_width_px: None,
                text_stroke_color: None,
                layout: None,
                text_shadow: None,
                geom_rect_padding: TextBodyRectPadding::default(),
            },
            measure,
            Some(intern),
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
    fn default_font_size_is_24px() {
        // Catches: paint defaulting missing font size to 18px instead of upstream 24px (18pt).
        let intern = paint_with(TextBodyPaint {
            text_color: None,
            font_size_px: None,
            font_family: None,
            font_bold: false,
            text_stroke_width_px: None,
            text_stroke_color: None,
            layout: None,
            text_shadow: None,
            geom_rect_padding: TextBodyRectPadding::default(),
        });
        assert!(
            intern.0.iter().any(|s| s == "24px"),
            "font-size, got {:?}",
            intern.0
        );
    }
}
