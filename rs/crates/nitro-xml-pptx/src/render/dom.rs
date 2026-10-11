// SPDX-License-Identifier: MIT
//! DOM opcode helpers (CreateElement / SetStyle / SetAttr / SetText).

use nitro_xml_render::CommandBuffer;

use super::context::{PaintContext, StringIntern};

pub fn intern_str<I: StringIntern>(ctx: &mut PaintContext<'_, I>, s: &str) -> u32 {
    ctx.intern.intern(s)
}

pub fn set_style<I: StringIntern>(ctx: &mut PaintContext<'_, I>, id: u32, prop: &str, val: &str) {
    let p = intern_str(ctx, prop);
    let v = intern_str(ctx, val);
    ctx.buffer.set_style(id, p, v);
}

pub fn set_attr<I: StringIntern>(ctx: &mut PaintContext<'_, I>, id: u32, key: &str, val: &str) {
    let k = intern_str(ctx, key);
    let v = intern_str(ctx, val);
    ctx.buffer.set_attr(id, k, v);
}

pub fn create_child<I: StringIntern>(ctx: &mut PaintContext<'_, I>, parent: u32, tag: &str) -> u32 {
    let id = ctx.alloc_id();
    let tag_id = intern_str(ctx, tag);
    ctx.buffer.create_element(id, tag_id, parent);
    id
}

pub fn position_box<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    id: u32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    set_style(ctx, id, "position", "absolute");
    set_style(ctx, id, "left", &format!("{}px", x.round()));
    set_style(ctx, id, "top", &format!("{}px", y.round()));
    set_style(ctx, id, "width", &format!("{}px", w.max(0.0).round()));
    set_style(ctx, id, "height", &format!("{}px", h.max(0.0).round()));
    set_style(ctx, id, "box-sizing", "border-box");
}

pub fn set_text_content<I: StringIntern>(ctx: &mut PaintContext<'_, I>, id: u32, text: &str) {
    let tid = intern_str(ctx, text);
    ctx.buffer.set_text(id, tid);
}

pub fn set_inner_markup_stub<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    parent: u32,
    markup: &str,
) {
    let mid = intern_str(ctx, markup);
    ctx.buffer.set_inner_markup(parent, mid);
}

pub fn stage_root_styles<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    root_id: u32,
    w: f64,
    h: f64,
    scale: f64,
) {
    let div = intern.intern("div");
    buf.create_element(root_id, div, 0);
    let mut set = |prop: &str, val: &str| {
        let p = intern.intern(prop);
        let v = intern.intern(val);
        buf.set_style(root_id, p, v);
    };
    set("position", "relative");
    set("width", &format!("{}px", w.round()));
    set("height", &format!("{}px", h.round()));
    set("transform", &format!("scale({})", format_scale(scale)));
    set("transform-origin", "top left");
    set("overflow", "hidden");
    set("background", "#ffffff");
}

fn format_scale(scale: f64) -> String {
    if (scale - 1.0).abs() < 0.0001 {
        "1".to_owned()
    } else {
        format!("{:.4}", scale)
    }
}
