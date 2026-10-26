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

/// Selection box in slide pixels (`rotation` in degrees, clockwise).
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionBox {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub rotation: f64,
}

/// Screen-pixel size of a resize handle and the stem above the top edge to the rotate handle.
pub const HANDLE_PX: f64 = 9.0;
pub const ROTATE_STEM_PX: f64 = 26.0;

/// Resize handles: name, fraction across the box, fraction down the box, CSS cursor.
pub const RESIZE_HANDLES: [(&str, f64, f64, &str); 8] = [
    ("nw", 0.0, 0.0, "nwse-resize"),
    ("n", 0.5, 0.0, "ns-resize"),
    ("ne", 1.0, 0.0, "nesw-resize"),
    ("e", 1.0, 0.5, "ew-resize"),
    ("se", 1.0, 1.0, "nwse-resize"),
    ("s", 0.5, 1.0, "ns-resize"),
    ("sw", 0.0, 1.0, "nesw-resize"),
    ("w", 0.0, 0.5, "ew-resize"),
];

/// Rotate a slide point about `(cx, cy)` by `deg` degrees clockwise (y grows down).
pub fn rotate_about(px: f64, py: f64, cx: f64, cy: f64, deg: f64) -> (f64, f64) {
    let (s, c) = deg.to_radians().sin_cos();
    let (dx, dy) = (px - cx, py - cy);
    (cx + dx * c - dy * s, cy + dx * s + dy * c)
}

/// Slide-space centres of the eight resize handles and the rotate handle (`"rot"`).
pub fn handle_centers(b: &SelectionBox, scale: f64) -> Vec<(&'static str, f64, f64)> {
    let (cx, cy) = (b.x + b.width / 2.0, b.y + b.height / 2.0);
    let mut out = Vec::with_capacity(9);
    for (name, fx, fy, _) in RESIZE_HANDLES {
        let (x, y) = rotate_about(b.x + fx * b.width, b.y + fy * b.height, cx, cy, b.rotation);
        out.push((name, x, y));
    }
    let stem = ROTATE_STEM_PX / scale.max(f64::EPSILON);
    let (x, y) = rotate_about(cx, b.y - stem, cx, cy, b.rotation);
    out.push(("rot", x, y));
    out
}

/// Outline every selected top-level shape. With `handles` and a single selection the box also
/// carries eight resize handles and a rotate handle (`data-handle`), sized in screen pixels.
/// `live` overrides a shape's geometry while it is being dragged.
pub fn paint<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    slide: &Slide,
    selected: &[String],
    live: &[SelectionBox],
    handles: bool,
) {
    if selected.is_empty() {
        return;
    }
    let scale = ctx.scale.max(f64::EPSILON);
    let px = |n: f64| format!("{:.3}px", n / scale);
    for el in &slide.elements {
        if !selected.iter().any(|id| id == &el.id) {
            continue;
        }
        let b = live
            .iter()
            .find(|l| l.id == el.id)
            .cloned()
            .unwrap_or_else(|| SelectionBox {
                id: el.id.clone(),
                x: el.x,
                y: el.y,
                width: el.width,
                height: el.height,
                rotation: el.rotation.unwrap_or(0.0),
            });
        let id = create_child(ctx, STAGE_ROOT_ID, "div");
        position_box(ctx, id, b.x, b.y, b.width, b.height);
        set_attr(ctx, id, "class", "pptx-sel");
        // Blue stroke with a white hairline on both sides so it reads on dark and light fills.
        set_style(ctx, id, "border", &format!("{} solid #1a73e8", px(2.0)));
        set_style(
            ctx,
            id,
            "box-shadow",
            &format!("0 0 0 {0} #fff, inset 0 0 0 {0} #fff", px(1.0)),
        );
        set_style(ctx, id, "background", "transparent");
        set_style(ctx, id, "pointer-events", "none");
        set_style(ctx, id, "z-index", "20");
        if b.rotation.abs() > f64::EPSILON {
            set_style(ctx, id, "transform", &format!("rotate({}deg)", b.rotation));
        }
        if !handles || selected.len() != 1 {
            continue;
        }
        let hs = HANDLE_PX / scale;
        // Children are placed from the padding box, which sits one border inside the outline.
        let inset = 2.0 / scale;
        let dot = |ctx: &mut PaintContext<'_, I>,
                   name: &str,
                   cx: f64,
                   cy: f64,
                   cursor: &str,
                   round: bool| {
            let h = create_child(ctx, id, "div");
            set_attr(ctx, h, "data-handle", name);
            set_attr(ctx, h, "class", "pptx-handle");
            set_style(ctx, h, "position", "absolute");
            set_style(ctx, h, "box-sizing", "border-box");
            set_style(ctx, h, "left", &format!("{:.3}px", cx - inset - hs / 2.0));
            set_style(ctx, h, "top", &format!("{:.3}px", cy - inset - hs / 2.0));
            set_style(ctx, h, "width", &format!("{hs:.3}px"));
            set_style(ctx, h, "height", &format!("{hs:.3}px"));
            set_style(ctx, h, "background", "#fff");
            set_style(ctx, h, "border", &format!("{} solid #1a73e8", px(1.5)));
            if round {
                set_style(ctx, h, "border-radius", "50%");
            }
            set_style(ctx, h, "cursor", cursor);
            set_style(ctx, h, "pointer-events", "auto");
        };
        for (name, fx, fy, cursor) in RESIZE_HANDLES {
            dot(ctx, name, fx * b.width, fy * b.height, cursor, false);
        }
        let stem = ROTATE_STEM_PX / scale;
        let line = create_child(ctx, id, "div");
        set_style(ctx, line, "position", "absolute");
        set_style(
            ctx,
            line,
            "left",
            &format!("{:.3}px", b.width / 2.0 - inset - 0.5 / scale),
        );
        set_style(ctx, line, "top", &format!("{:.3}px", -stem - inset));
        set_style(ctx, line, "width", &px(1.0));
        set_style(ctx, line, "height", &format!("{stem:.3}px"));
        set_style(ctx, line, "background", "#1a73e8");
        set_style(ctx, line, "pointer-events", "none");
        dot(ctx, "rot", b.width / 2.0, -stem, "grab", true);
    }
}
