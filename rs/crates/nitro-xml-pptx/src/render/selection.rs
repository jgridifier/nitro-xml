// SPDX-License-Identifier: MIT
//! Top-level shape hit testing, copy text, and the selection outline (#43).

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_style};
use crate::presentation::Slide;
use crate::slide::DumpElement;

const STAGE_ROOT_ID: u32 = 1;

pub fn hit_top_level(slide: &Slide, x: f64, y: f64) -> Option<String> {
    let mut found = None;
    for el in &slide.elements {
        if x >= el.x && y >= el.y && x <= el.x + el.width && y <= el.y + el.height {
            found = Some(el.id.clone());
        }
    }
    found
}

pub fn top_level_ids(slide: &Slide) -> Vec<String> {
    slide.elements.iter().map(|el| el.id.clone()).collect()
}

pub fn contains_id(slide: &Slide, id: &str) -> bool {
    slide.elements.iter().any(|el| el.id == id)
}

/// Selected elements' text in paint order, joined with newlines.
pub fn copy_text(slide: &Slide, ids: &[String]) -> String {
    let mut parts = Vec::new();
    for el in &slide.elements {
        if ids.iter().any(|id| id == &el.id) {
            let text = element_text(el);
            if !text.is_empty() {
                parts.push(text);
            }
        }
    }
    parts.join("\n")
}

fn element_text(el: &DumpElement) -> String {
    if let Some(text) = el.text.as_deref() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    let mut parts = Vec::new();
    if let Some(children) = &el.children {
        for child in children {
            let text = element_text(child);
            if !text.is_empty() {
                parts.push(text);
            }
        }
    }
    parts.join("\n")
}

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, slide: &Slide, selected: &[String]) {
    if selected.is_empty() {
        return;
    }
    for el in &slide.elements {
        if !selected.iter().any(|id| id == &el.id) {
            continue;
        }
        let id = create_child(ctx, STAGE_ROOT_ID, "div");
        position_box(ctx, id, el.x, el.y, el.width, el.height);
        set_attr(ctx, id, "class", "pptx-sel");
        set_style(ctx, id, "border", "2px solid #1a73e8");
        set_style(ctx, id, "background", "transparent");
        set_style(ctx, id, "pointer-events", "none");
        set_style(ctx, id, "z-index", "20");
    }
}
