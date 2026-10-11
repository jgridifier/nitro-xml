// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/slide-stage.ts and
// src/ui/src/pptx/render/slide-background.ts; changes in PROVENANCE.md.

use super::context::{PaintContext, StringIntern};
use super::dispatch::paint_element;
use super::dom::stage_root_styles;
use super::placeholder;
use crate::presentation::Slide;

const STAGE_ROOT_ID: u32 = 1;

pub fn paint_slide_stage<I: StringIntern>(ctx: &mut PaintContext<'_, I>, slide: &Slide) {
    stage_root_styles(
        ctx.buffer,
        ctx.intern,
        STAGE_ROOT_ID,
        ctx.canvas_w,
        ctx.canvas_h,
        ctx.scale,
        slide.background_color.as_deref(),
        slide.background_gradient.as_deref(),
    );
    for el in &slide.elements {
        paint_element(ctx, el, STAGE_ROOT_ID);
    }
    for (i, kind) in slide.other_kinds.iter().enumerate() {
        placeholder::paint_other_kind(ctx, kind, i, STAGE_ROOT_ID, ctx.canvas_w, ctx.canvas_h);
    }
}
