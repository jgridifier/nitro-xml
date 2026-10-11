// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/docx/print-page-borders.ts
// and src/core/docx/ui/table-render.ts (`cssBorderSide`); changes listed in PROVENANCE.md.

use super::dom::{js_num, Dom, StringIntern};
use crate::layout::LayoutPageBox;

const POINT: f64 = 4.0 / 3.0;

#[derive(Clone, Debug, Default)]
pub struct PageBorderSide {
    pub style: Option<String>,
    pub size_eighth_points: Option<f64>,
    pub color: Option<String>,
    pub space_points: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct PageBorders {
    pub display: Option<String>,
    pub offset_from: Option<String>,
    pub z_order: Option<String>,
    pub top: Option<PageBorderSide>,
    pub left: Option<PageBorderSide>,
    pub bottom: Option<PageBorderSide>,
    pub right: Option<PageBorderSide>,
}

pub fn page_borders_from_section(section: &serde_json::Value) -> Option<PageBorders> {
    let raw = section.get("pageBorders")?;
    Some(PageBorders {
        display: str_field(raw, "display"),
        offset_from: str_field(raw, "offsetFrom"),
        z_order: str_field(raw, "zOrder"),
        top: side_from(raw.get("top")),
        left: side_from(raw.get("left")),
        bottom: side_from(raw.get("bottom")),
        right: side_from(raw.get("right")),
    })
}

fn str_field(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(|s| s.to_string())
}

fn side_from(v: Option<&serde_json::Value>) -> Option<PageBorderSide> {
    let v = v?;
    if v.is_null() {
        return None;
    }
    Some(PageBorderSide {
        style: str_field(v, "style"),
        size_eighth_points: v
            .get("sizeEighthPoints")
            .and_then(|n| n.as_f64().or_else(|| n.as_i64().map(|i| i as f64))),
        color: str_field(v, "color"),
        space_points: v
            .get("spacePoints")
            .and_then(|n| n.as_f64().or_else(|| n.as_i64().map(|i| i as f64))),
    })
}

fn css_border_style(style: Option<&str>) -> String {
    let Some(style) = style.filter(|s| !s.is_empty()) else {
        return "none".into();
    };
    if style == "none" || style == "nil" {
        return "none".into();
    }
    if style == "double" {
        return "double".into();
    }
    let lower = style.to_lowercase();
    if lower.contains("dot") {
        return "dotted".into();
    }
    if lower.contains("dash") {
        return "dashed".into();
    }
    "solid".into()
}

fn css_border_side(side: &PageBorderSide) -> Option<String> {
    let style = side.style.as_deref().unwrap_or("");
    if style.is_empty() || style == "none" || style == "nil" {
        return None;
    }
    let width_px = match side.size_eighth_points {
        Some(eighths) if eighths > 0.0 => ((eighths / 8.0) * POINT).round().max(1.0),
        _ => 1.0,
    };
    let color = side.color.as_deref().unwrap_or("#000000");
    Some(format!(
        "{}px {} {}",
        js_num(width_px),
        css_border_style(Some(style)),
        color
    ))
}

fn space_px(side: &Option<PageBorderSide>) -> f64 {
    side.as_ref().and_then(|s| s.space_points).unwrap_or(24.0) * POINT
}

/// A positioned page-border box, or `None` when `w:display` hides it or no side has a line.
pub fn paint_page_border_box<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    parent: u32,
    page: &LayoutPageBox,
    borders: &PageBorders,
) -> Option<u32> {
    if borders.display.as_deref() == Some("firstPage") && page.page_in_section != 0 {
        return None;
    }
    if borders.display.as_deref() == Some("notFirstPage") && page.page_in_section == 0 {
        return None;
    }
    let css = [
        ("top", css_border_side_opt(&borders.top)),
        ("left", css_border_side_opt(&borders.left)),
        ("bottom", css_border_side_opt(&borders.bottom)),
        ("right", css_border_side_opt(&borders.right)),
    ];
    if css.iter().all(|(_, v)| v.is_none()) {
        return None;
    }
    let from_text = borders.offset_from.as_deref() == Some("text");
    let left = if from_text {
        page.margin_left_px - space_px(&borders.left)
    } else {
        space_px(&borders.left)
    };
    let top = if from_text {
        page.margin_top_px - space_px(&borders.top)
    } else {
        space_px(&borders.top)
    };
    let right = if from_text {
        page.margin_right_px - space_px(&borders.right)
    } else {
        space_px(&borders.right)
    };
    let bottom = if from_text {
        page.margin_bottom_px - space_px(&borders.bottom)
    } else {
        space_px(&borders.bottom)
    };
    let left_c = left.max(0.0);
    let top_c = top.max(0.0);
    let box_id = dom.el(parent, "div");
    dom.class(box_id, "dve-print-page-border");
    dom.attr(box_id, "aria-hidden", "true");
    dom.style(box_id, "position", "absolute");
    dom.style(box_id, "box-sizing", "border-box");
    dom.style(box_id, "pointer-events", "none");
    dom.px(box_id, "left", left_c);
    dom.px(box_id, "top", top_c);
    dom.px(
        box_id,
        "width",
        (page.width_px - left_c - right.max(0.0)).max(0.0),
    );
    dom.px(
        box_id,
        "height",
        (page.height_px - top_c - bottom.max(0.0)).max(0.0),
    );
    let z = if borders.z_order.as_deref() == Some("back") {
        "0"
    } else {
        "2"
    };
    dom.style(box_id, "z-index", z);
    for (side, value) in css {
        if let Some(value) = value {
            let prop = format!("border-{side}");
            dom.style(box_id, &prop, &value);
        }
    }
    Some(box_id)
}

fn css_border_side_opt(side: &Option<PageBorderSide>) -> Option<String> {
    side.as_ref().and_then(css_border_side)
}
