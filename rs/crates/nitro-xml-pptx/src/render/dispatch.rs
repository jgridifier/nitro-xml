// SPDX-License-Identifier: MIT
//! Element-kind dispatch (never silently skip).

use super::chart;
use super::connector;
use super::context::{PaintContext, StringIntern};
use super::group;
use super::ole;
use super::picture;
use super::placeholder;
use super::shape;
use super::smartart;
use super::table;
use super::text;
use crate::slide::DumpElement;

pub fn paint_element<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    el: &DumpElement,
    parent: u32,
) {
    match el.type_.as_str() {
        "shape" => shape::paint(ctx, el, parent),
        "text" => text::paint(ctx, el, parent),
        "picture" | "image" => picture::paint(ctx, el, parent),
        "group" => group::paint(ctx, el, parent),
        "connector" => connector::paint(ctx, el, parent),
        "table" => table::paint(ctx, el, parent),
        "chart" => chart::paint(ctx, el, parent),
        "smartArt" => smartart::paint(ctx, el, parent),
        "video" | "audio" => placeholder::paint_typed_stub(ctx, &el.type_, el, parent, "[media]"),
        "ole" => ole::paint(ctx, el, parent),
        "model3d" => placeholder::paint_typed_stub(ctx, "model3d", el, parent, "[3D Model]"),
        "ink" => placeholder::paint_typed_stub(ctx, "ink", el, parent, "[Ink]"),
        _ => placeholder::paint_unknown_element(ctx, el, parent),
    }
}
