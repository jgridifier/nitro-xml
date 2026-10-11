// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/text-block.ts; changes in PROVENANCE.md.

use nitro_xml_render::measure::LayoutReadKind;

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_style, set_text_content};
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "text");
    let text = el.text.as_deref().unwrap_or("");
    let block = paint_text_body(ctx, id, text, el.text_color.as_deref(), el.font_size_px);
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
    text_color: Option<&str>,
    font_size_px: Option<f64>,
) -> u32 {
    let block = create_child(ctx, parent, "div");
    set_attr(ctx, block, "class", "pptxv-text");
    set_style(ctx, block, "white-space", "pre-wrap");
    set_style(ctx, block, "word-wrap", "break-word");
    let size = font_size_px.unwrap_or(18.0).max(1.0);
    set_style(
        ctx,
        block,
        "font",
        &format!("{}px Calibri, Carlito, Arial, sans-serif", format_px(size)),
    );
    if let Some(color) = text_color {
        set_style(ctx, block, "color", color);
    }
    set_text_content(ctx, block, text);
    // Batched measureText for GDI-style tracking (upstream text-metric-tracking).
    if !text.is_empty() {
        let font_id = ctx.intern.intern("Calibri");
        let text_id = ctx.intern.intern(text);
        ctx.measure.push(font_id, size as f32, text_id);
    }
    block
}

fn format_px(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}
