// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/text-block.ts,
// src/ui/src/pptx/render/text-effects.ts; changes in PROVENANCE.md.

use nitro_xml_render::measure::LayoutReadKind;

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_style, set_text_content};
use super::font_substitution::substitute_font_family_stack;
use crate::slide::DumpElement;
use crate::text_layout::{TextBodyLayout, TextBodyVAlign};

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
        }
    }
}

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "text");
    let text = el.text.as_deref().unwrap_or("");
    let block = paint_text_body(ctx, id, text, TextBodyPaint::from_element(el));
    // Autofit uses post-apply ScrollHeight + ComputedStyle (docs/boundary.md #127).
    ctx.layout_reads
        .push(block, LayoutReadKind::ScrollHeight, 0);
    let font_size_prop = ctx.intern.intern("font-size");
    ctx.layout_reads
        .push(block, LayoutReadKind::ComputedStyle, font_size_prop);
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
        apply_text_body_layout(ctx, block, layout);
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
    set_text_content(ctx, block, text);
    // Batched measureText for GDI-style tracking (upstream text-metric-tracking).
    if !text.is_empty() {
        let family = paint
            .font_family
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Calibri");
        let font_id = ctx.intern.intern(family);
        let text_id = ctx.intern.intern(text);
        ctx.measure.push(font_id, size as f32, text_id);
    }
    block
}

fn apply_text_body_layout<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    block: u32,
    layout: &TextBodyLayout,
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
    set_style(ctx, block, "padding-top", &format_px(layout.inset_top_px));
    set_style(
        ctx,
        block,
        "padding-bottom",
        &format_px(layout.inset_bottom_px),
    );
    set_style(ctx, block, "padding-left", &format_px(layout.inset_left_px));
    set_style(
        ctx,
        block,
        "padding-right",
        &format_px(layout.inset_right_px),
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
        let mut buffer = CommandBuffer::new();
        let mut intern = Intern(Vec::new());
        let mut measure = MeasureBatch::default();
        let mut layout_reads = LayoutReadBatch::default();
        let mut view_ir = ViewIrCollector::default();
        let scheme = BTreeMap::new();
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
        });
        assert!(
            intern.0.iter().any(|s| s == "24px"),
            "font-size, got {:?}",
            intern.0
        );
    }
}
