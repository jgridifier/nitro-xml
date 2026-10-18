// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/chart-user-shape-overlay.ts;
// changes listed in PROVENANCE.md.

use nitro_xml_opc::chart::{
    flatten_chart_user_shapes, ChartUserShape, ChartUserShapeParagraph, ChartUserShapesChartBox,
    FlattenedChartUserShape,
};

const EMU_PER_PIXEL: f64 = 9525.0;
const PX_PER_PT: f64 = 4.0 / 3.0;
const DEFAULT_CHART_TEXT_PX: f64 = 10.0 * PX_PER_PT;
const CHART_FONT_FAMILY: &str = "Calibri, Carlito, Arial, sans-serif";

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn overlay_transform(
    rotation: Option<f64>,
    flip_h: Option<bool>,
    flip_v: Option<bool>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Option<String> {
    let flip_h = flip_h.unwrap_or(false);
    let flip_v = flip_v.unwrap_or(false);
    let rotation = rotation.unwrap_or(0.0);
    if rotation == 0.0 && !flip_h && !flip_v {
        return None;
    }
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let mut parts = Vec::new();
    if rotation != 0.0 {
        parts.push(format!("rotate({} {} {})", rotation, cx, cy));
    }
    if flip_h || flip_v {
        let sx = if flip_h { -1 } else { 1 };
        let sy = if flip_v { -1 } else { 1 };
        parts.push(format!(
            "translate({} {}) scale({} {}) translate({} {})",
            cx, cy, sx, sy, -cx, -cy
        ));
    }
    Some(parts.join(" "))
}

fn shape_box(shape: &FlattenedChartUserShape, svg_w: f64, svg_h: f64) -> (f64, f64, f64, f64) {
    let x = shape.from.x * svg_w;
    let y = shape.from.y * svg_h;
    if shape.anchor == "rel" {
        if let Some(to) = &shape.to {
            return (
                x,
                y,
                ((to.x - shape.from.x) * svg_w).max(0.0),
                ((to.y - shape.from.y) * svg_h).max(0.0),
            );
        }
    }
    let ext = shape.ext.as_ref();
    let cx = ext.map(|e| e.cx).unwrap_or(0.0);
    let cy = ext.map(|e| e.cy).unwrap_or(0.0);
    let ox = shape
        .abs_group_offset_emu
        .as_ref()
        .map(|o| o.x / EMU_PER_PIXEL)
        .unwrap_or(0.0);
    let oy = shape
        .abs_group_offset_emu
        .as_ref()
        .map(|o| o.y / EMU_PER_PIXEL)
        .unwrap_or(0.0);
    (x + ox, y + oy, cx / EMU_PER_PIXEL, cy / EMU_PER_PIXEL)
}

fn text_markup(
    shape: &FlattenedChartUserShape,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    rotation: Option<f64>,
) -> String {
    let paragraphs = match shape.paragraphs.as_ref() {
        Some(p) if !p.is_empty() => p,
        _ => return String::new(),
    };
    let font_px = |para: &ChartUserShapeParagraph| {
        para.font_size_pt
            .map(|pt| pt * PX_PER_PT)
            .unwrap_or(DEFAULT_CHART_TEXT_PX)
    };
    let line_h = paragraphs
        .iter()
        .map(font_px)
        .map(|px| px * 1.2)
        .fold(12.0, f64::max);
    let total_h = paragraphs.len() as f64 * line_h;
    let mut cursor_y = y + ((h - total_h) / 2.0).max(0.0) + line_h * 0.75;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let mut out = String::new();
    for para in paragraphs {
        if para.text.is_empty() {
            cursor_y += line_h;
            continue;
        }
        let align = para.align.as_deref().unwrap_or("ctr");
        let anchor = if align == "l" {
            "start"
        } else if align == "r" {
            "end"
        } else {
            "middle"
        };
        let tx = if align == "l" {
            x + 2.0
        } else if align == "r" {
            x + w - 2.0
        } else {
            x + w / 2.0
        };
        let fill = para.color.as_deref().unwrap_or("#1e293b");
        let weight = if para.bold.unwrap_or(false) {
            "bold"
        } else {
            "normal"
        };
        let transform = rotation
            .filter(|r| *r != 0.0)
            .map(|r| format!(" transform=\"rotate({} {} {})\"", r, cx, cy));
        out.push_str(&format!(
            "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"{:.2}\" font-family=\"{}\" fill=\"{}\" text-anchor=\"{}\" font-weight=\"{}\"{}>{}</text>",
            tx,
            cursor_y,
            font_px(para),
            esc(CHART_FONT_FAMILY),
            esc(fill),
            anchor,
            weight,
            transform.unwrap_or_default(),
            esc(&para.text)
        ));
        cursor_y += line_h;
    }
    out
}

fn snap(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn project_shape(shape: &FlattenedChartUserShape, svg_w: f64, svg_h: f64) -> String {
    let (x, y, w, h) = shape_box(shape, svg_w, svg_h);
    let (x, y, w, h) = (snap(x), snap(y), snap(w), snap(h));
    let transform = overlay_transform(shape.rotation, shape.flip_h, shape.flip_v, x, y, w, h);
    let tf = transform
        .as_ref()
        .map(|t| format!(" transform=\"{}\"", esc(t)))
        .unwrap_or_default();
    if shape.kind == "cxnSp" {
        let stroke = shape.stroke.as_deref().unwrap_or("#000000");
        let sw = shape.stroke_width.unwrap_or(1.0);
        return format!(
            "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{}\" stroke-width=\"{:.2}\"{} />",
            x,
            y,
            x + w,
            y + h,
            esc(stroke),
            sw,
            tf
        );
    }
    let mut out = String::new();
    if (shape.fill.is_some() || shape.stroke.is_some()) && w > 0.0 && h > 0.0 {
        let fill = shape.fill.as_deref().unwrap_or("none");
        let stroke = shape.stroke.as_deref().unwrap_or("none");
        let sw = shape
            .stroke_width
            .unwrap_or(if shape.stroke.is_some() { 1.0 } else { 0.0 });
        out.push_str(&format!(
            "<polygon points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{:.2}\"{} />",
            x,
            y,
            x + w,
            y,
            x + w,
            y + h,
            x,
            y + h,
            esc(fill),
            esc(stroke),
            sw,
            tf
        ));
    }
    out.push_str(&text_markup(shape, x, y, w, h, shape.rotation));
    out
}

/// Build overlay SVG fragments for chart user shapes (no outer `<svg>` wrapper).
pub fn build_chart_user_shape_overlay_svg(
    user_shapes: Option<&[ChartUserShape]>,
    svg_width: u32,
    svg_height: u32,
) -> String {
    let w = svg_width.max(1) as f64;
    let h = svg_height.max(1) as f64;
    let chart_box = ChartUserShapesChartBox {
        width: w * EMU_PER_PIXEL,
        height: h * EMU_PER_PIXEL,
    };
    let leaves = flatten_chart_user_shapes(user_shapes, Some(&chart_box));
    leaves.iter().map(|s| project_shape(s, w, h)).collect()
}
