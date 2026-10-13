// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/smartart-drawing.ts
// (`projectDrawingShapes` / `resolveShapeKind`) and
// src/ui/src/pptx/dom/elements/smartart-svg.ts (cached-drawing assembly);
// changes listed in PROVENANCE.md.
//! S2: shared SmartArt cached-drawing painter.
//!
//! When a diagram drawing part is available, emit SVG for `dsp:sp` / `dsp:pic`
//! using preset and custom geometry paths (pptx P11 `projectDrawingShapes`
//! approach). Textless path/connector shapes are kept; empty noFill placeholder
//! frames are still dropped. Used by pptx; xlsx/docx can call the same entry.

mod circular_arrow;
mod text_wrap_estimate;

use text_wrap_estimate::{drawing_text_base_line_height_ratio, wrap_text_by_estimated_width};

use std::collections::BTreeMap;

use nitro_xml_opc::drawingml::{
    emu_to_pixels, evaluate_preset_shape, ooxml_arc_to_svg, parse_drawing_color_in,
    parse_drawing_geometry, resolve_drawing_color, DrawingColorTheme, DrawingPath,
    DrawingPathCommand, EMU_PER_PIXEL,
};
use nitro_xml_opc::xml::{parse_xml_str, Element, Node};

/// `a:ln/@w` on diagram drawing shapes: TS `parseDrawingShape` maps EMU ÷ 12700 to SVG stroke-width.
const EMU_PER_SMARTART_LINE_WIDTH: f64 = 12700.0;

fn smartart_ln_width_emu_to_svg(emu: f64) -> f64 {
    emu / EMU_PER_SMARTART_LINE_WIDTH
}

/// Match TS `parseDrawingShape`: `Math.round(emu / EMU_PER_PX)` for shape boxes.
fn smartart_drawing_css_px(emu: i64) -> f64 {
    (emu as f64 / EMU_PER_PIXEL as f64).round()
}

fn smartart_drawing_emu_attr_px(el: &Element, name: &str) -> Option<f64> {
    let emu = attr_str(el, name)?.parse::<i64>().ok()?;
    Some(smartart_drawing_css_px(emu))
}

struct SchemeMap<'a>(&'a BTreeMap<String, String>);

impl DrawingColorTheme for SchemeMap<'_> {
    fn scheme(&self, name: &str) -> Option<String> {
        let slot = match name {
            "bg1" => "lt1",
            "tx1" => "dk1",
            "bg2" => "lt2",
            "tx2" => "dk2",
            other => other,
        };
        self.0.get(slot).or_else(|| self.0.get(name)).map(|s| {
            let raw = s.trim().trim_start_matches('#');
            format!("#{raw}")
        })
    }
}

/// SVG markup for a SmartArt frame. `drawing_xml` is the diagram drawing part
/// (`dsp:drawing`) when the package resolved one.
pub fn smartart_svg(drawing_xml: Option<&str>, width: f64, height: f64) -> String {
    smartart_svg_with_scheme(drawing_xml, width, height, None)
}

/// Like [`smartart_svg`], resolving `a:schemeClr` through `scheme` (`accent1` → `#156082`).
///
/// Empty noFill placeholder frames (and textless pictures without a blip) are
/// dropped. Textless shapes with real geometry (preset paths, custGeom
/// connectors, solid fills) are kept.
pub fn smartart_svg_with_scheme(
    drawing_xml: Option<&str>,
    width: f64,
    height: f64,
    scheme: Option<&BTreeMap<String, String>>,
) -> String {
    let w = width.max(1.0).round();
    let h = height.max(1.0).round();
    if let Some(xml) = drawing_xml {
        if let Ok(doc) = parse_xml_str(xml) {
            let mut shapes = collect_shapes(&doc.root, scheme);
            shapes.retain(keep_shape);
            if !shapes.is_empty() {
                return shapes_svg(&shapes, w, h);
            }
        }
    }
    labelled_svg(w, h)
}

/// Compact draw arguments for differential debug (geometry kind + box + paint),
/// not glyph/AA sensitive.
pub fn smartart_draw_args(
    drawing_xml: Option<&str>,
    scheme: Option<&BTreeMap<String, String>>,
) -> Vec<SmartArtDrawArg> {
    let Some(xml) = drawing_xml else {
        return Vec::new();
    };
    let Ok(doc) = parse_xml_str(xml) else {
        return Vec::new();
    };
    let mut shapes = collect_shapes(&doc.root, scheme);
    shapes.retain(keep_shape);
    shapes
        .into_iter()
        .map(|s| SmartArtDrawArg {
            kind: s.kind.tag().to_owned(),
            geom: s.geom_name.clone(),
            x: s.x,
            y: s.y,
            w: s.w,
            h: s.h,
            fill: s.fill.clone(),
            stroke: s.stroke.clone(),
            stroke_width: s.stroke_width,
            font_color: s.font_color.clone(),
            text: s.flat_text(),
            path_len: s.kind.path_d().map(|d| d.len()).unwrap_or(0),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmartArtDrawArg {
    pub kind: String,
    pub geom: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub fill: String,
    pub stroke: String,
    pub stroke_width: f64,
    /// Label fill (`fontRef` / contrast), matching TS `fontColor` on projected shapes.
    pub font_color: String,
    pub text: String,
    pub path_len: usize,
}

#[derive(Clone)]
enum ShapeKind {
    Rect { rx: f64 },
    Ellipse,
    Path { d: String },
}

impl ShapeKind {
    fn tag(&self) -> &'static str {
        match self {
            Self::Rect { .. } => "rect",
            Self::Ellipse => "ellipse",
            Self::Path { .. } => "path",
        }
    }

    fn path_d(&self) -> Option<&str> {
        match self {
            Self::Path { d } => Some(d.as_str()),
            _ => None,
        }
    }
}

#[derive(Clone, Default)]
struct TextInsets {
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
}

/// Independent text frame from `dsp:txXfrm` plus `a:bodyPr` insets (CSS px).
#[derive(Clone)]
struct TextFrameGeom {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    insets: TextInsets,
}

#[derive(Clone)]
struct TextRun {
    text: String,
    /// Parsed for parity with OOXML; SmartArt label paint uses shape-level first-run size (TS).
    #[allow(dead_code)]
    font_size_px: Option<f64>,
}

#[derive(Clone)]
struct TextParagraph {
    runs: Vec<TextRun>,
    /// `a:lnSpc/a:spcPct/@val` as a multiplier (90000 → 0.9).
    line_spacing_ratio: Option<f64>,
    /// `a:spcAft/a:spcPct/@val` as a multiplier added to the line-height ratio.
    space_after_ratio: Option<f64>,
}

struct SpBox {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: String,
    fill_opacity: Option<f64>,
    stroke: String,
    stroke_width: f64,
    paragraphs: Vec<TextParagraph>,
    text_frame: Option<TextFrameGeom>,
    font_color: String,
    /// `dsp:style/a:fontRef` resolved explicitly (skip underlying-fill label pass).
    label_font_from_ref: bool,
    /// Authored font size in CSS px when `a:rPr/@sz` is present (`sz/100 * 96/72`).
    font_size_px: Option<f64>,
    /// `a:rPr/@kern` in hundredths of a point (SmartArt gallery labels use 1200).
    kerning_hundredths_pt: Option<i64>,
    geom_name: String,
    kind: ShapeKind,
    is_pic: bool,
    rotation: Option<f64>,
    flip_h: bool,
    flip_v: bool,
}

impl SpBox {
    fn flat_text(&self) -> String {
        paragraphs_plain_text(&self.paragraphs)
    }

    fn has_text(&self) -> bool {
        self.paragraphs
            .iter()
            .any(|p| p.runs.iter().any(|r| !r.text.trim().is_empty()))
    }
}

fn keep_shape(shape: &SpBox) -> bool {
    if shape.has_text() {
        return true;
    }
    if shape.is_pic {
        return false;
    }
    match &shape.kind {
        ShapeKind::Path { d } => !d.is_empty(),
        ShapeKind::Ellipse => true,
        ShapeKind::Rect { .. } => shape.fill != "none" || shape.stroke_width > 0.0,
    }
}

fn labelled_svg(w: f64, h: f64) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" role=\"img\"><rect width=\"100%\" height=\"100%\" fill=\"#f4f0fa\" stroke=\"#7b5ea7\"/><text x=\"8\" y=\"20\" font-size=\"14\" fill=\"#333\">SmartArt</text></svg>"
    )
}

fn diagram_shape_bounds(shape: &SpBox) -> (f64, f64, f64, f64) {
    let frame = (shape.x, shape.y, shape.x + shape.w, shape.y + shape.h);
    let (tx, ty, tw, th) = shape
        .text_frame
        .as_ref()
        .map(|tf| (tf.x, tf.y, tf.w, tf.h))
        .unwrap_or((shape.x, shape.y, shape.w, shape.h));
    let text_max_x = tx + tw;
    let text_max_y = ty + th;
    (
        frame.0.min(tx),
        frame.1.min(ty),
        frame.2.max(text_max_x),
        frame.3.max(text_max_y),
    )
}

fn shapes_svg(shapes: &[SpBox], _w: f64, _h: f64) -> String {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for shape in shapes {
        let (smin_x, smin_y, smax_x, smax_y) = diagram_shape_bounds(shape);
        min_x = min_x.min(smin_x);
        min_y = min_y.min(smin_y);
        max_x = max_x.max(smax_x);
        max_y = max_y.max(smax_y);
    }
    if !min_x.is_finite() {
        min_x = 0.0;
        min_y = 0.0;
        max_x = 1.0;
        max_y = 1.0;
    }
    let origin_x = min_x;
    let origin_y = min_y;
    let vw = (max_x - min_x).max(1.0);
    let vh = (max_y - min_y).max(1.0);
    let default_stroke = (vw.max(vh) / 200.0).max(1.0);
    let mut body = String::new();
    for shape in shapes {
        let paint_body = shape.fill != "none" || shape.stroke != "none";
        let stroke_w = if shape.stroke_width > 0.0 {
            shape.stroke_width
        } else {
            default_stroke
        };
        let stroke_attr = if shape.stroke == "none" {
            "stroke=\"none\"".to_owned()
        } else {
            format!(
                "stroke=\"{}\" stroke-width=\"{}\"",
                escape(&shape.stroke),
                svg_user_coord(stroke_w)
            )
        };
        let sx = shape.x - origin_x;
        let sy = shape.y - origin_y;
        let cx = sx + shape.w / 2.0;
        let cy = sy + shape.h / 2.0;
        let transform = shape_transform(shape, cx, cy);
        let transform_attr = transform
            .as_ref()
            .map(|t| format!(" transform=\"{}\"", escape(t)))
            .unwrap_or_default();

        if paint_body {
            match &shape.kind {
                ShapeKind::Rect { rx } => {
                    let rx_attr = if *rx > 0.0 {
                        format!(" rx=\"{}\"", num(*rx))
                    } else {
                        String::new()
                    };
                    let opacity = fill_opacity_attr(shape.fill_opacity);
                    body.push_str(&format!(
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rx_attr} fill=\"{}\"{opacity} {stroke_attr}{transform_attr}/>",
                        num(sx),
                        num(sy),
                        num(shape.w.max(1.0)),
                        num(shape.h.max(1.0)),
                        escape(&shape.fill),
                    ));
                }
                ShapeKind::Ellipse => {
                    let opacity = fill_opacity_attr(shape.fill_opacity);
                    body.push_str(&format!(
                        "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"{}\"{opacity} {stroke_attr}{transform_attr}/>",
                        num(cx),
                        num(cy),
                        num((shape.w / 2.0).max(0.5)),
                        num((shape.h / 2.0).max(0.5)),
                        escape(&shape.fill),
                    ));
                }
                ShapeKind::Path { d } => {
                    let path_transform = match &transform {
                        Some(t) => format!("{t} translate({} {})", num(sx), num(sy)),
                        None => format!("translate({} {})", num(sx), num(sy)),
                    };
                    let opacity = fill_opacity_attr(shape.fill_opacity);
                    body.push_str(&format!(
                        "<path d=\"{}\" fill=\"{}\"{opacity} {stroke_attr} transform=\"{}\"/>",
                        escape(d),
                        escape(&shape.fill),
                        escape(&path_transform),
                    ));
                }
            }
        }

        if shape.has_text() {
            append_shape_text(
                &mut body,
                shape,
                stroke_w.max(default_stroke),
                origin_x,
                origin_y,
            );
        }
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" preserveAspectRatio=\"xMidYMid meet\" style=\"width:100%;height:100%;pointer-events:none;display:block\" role=\"img\">{body}</svg>",
        svg_user_coord(vw),
        svg_user_coord(vh),
    )
}

fn shape_transform(shape: &SpBox, cx: f64, cy: f64) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(rot) = shape.rotation {
        if rot.abs() > 1e-6 {
            parts.push(format!("rotate({} {} {})", num(rot), num(cx), num(cy)));
        }
    }
    if shape.flip_h || shape.flip_v {
        let sx = if shape.flip_h { -1.0 } else { 1.0 };
        let sy = if shape.flip_v { -1.0 } else { 1.0 };
        parts.push(format!(
            "translate({} {}) scale({} {}) translate({} {})",
            num(cx),
            num(cy),
            num(sx),
            num(sy),
            num(-cx),
            num(-cy),
        ));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

/// Font size that stays inside the shape. `f64::clamp` panics when the
/// stroke-derived minimum exceeds the shape height (wide diagrams with a
/// short connector), which aborts wasm and leaves the handle borrowed.
fn fit_font(h: f64, stroke: f64) -> f64 {
    let preferred = h / 4.0;
    let min_size = stroke * 8.0;
    let max_size = h * 0.6;
    if !preferred.is_finite() || !min_size.is_finite() || !max_size.is_finite() {
        return 12.0;
    }
    let max_size = max_size.max(1.0);
    if min_size > max_size {
        return max_size;
    }
    preferred.clamp(min_size.max(1.0), max_size).max(1.0)
}

const TEXT_WIDTH_FRACTION: f64 = 0.82;

struct PositionedTextLine {
    y: f64,
    spans: Vec<(String, f64)>,
}

fn default_label_font(shape: &SpBox, stroke: f64) -> f64 {
    let frame_h = shape.text_frame.as_ref().map(|f| f.h).unwrap_or(shape.h);
    match shape.font_size_px {
        Some(sz) if sz > 0.0 => sz.min(frame_h.max(1.0)).max(1.0),
        _ => (frame_h * 0.2)
            .clamp(8.0, 14.0)
            .min(fit_font(frame_h, stroke))
            .max(1.0),
    }
}

fn text_layout_metrics(shape: &SpBox, stroke: f64) -> (f64, f64, f64, f64) {
    let base_font = default_label_font(shape, stroke);
    if let Some(tf) = shape.text_frame.as_ref() {
        if tf.w > 0.0 && tf.h > 0.0 {
            let content_w = (tf.w - tf.insets.left - tf.insets.right).max(1.0);
            let content_h = (tf.h - tf.insets.top - tf.insets.bottom).max(1.0);
            let text_cx = tf.x + tf.insets.left + content_w / 2.0;
            let text_cy = tf.y + tf.insets.top + content_h / 2.0;
            return (text_cx, text_cy, content_w, base_font);
        }
    }
    let text_cx = shape.x + shape.w / 2.0;
    let text_cy = shape.y + shape.h / 2.0;
    let max_width = (shape.w * TEXT_WIDTH_FRACTION).max(base_font);
    (text_cx, text_cy, max_width, base_font)
}

fn paragraph_to_lines(
    para: &TextParagraph,
    label_font: f64,
    wrap_font: f64,
    max_width: f64,
) -> Vec<Vec<(String, f64)>> {
    if para.runs.is_empty() {
        return Vec::new();
    }
    if para.runs.len() == 1 {
        let run = &para.runs[0];
        let wrapped = wrap_text_by_estimated_width(&run.text, max_width, wrap_font);
        if wrapped.is_empty() {
            return vec![vec![(run.text.clone(), label_font)]];
        }
        return wrapped
            .into_iter()
            .map(|line| vec![(line, label_font)])
            .collect();
    }
    let joined = para
        .runs
        .iter()
        .map(|r| r.text.as_str())
        .collect::<String>();
    let wrapped = wrap_text_by_estimated_width(&joined, max_width, wrap_font);
    if wrapped.len() <= 1 {
        return vec![para
            .runs
            .iter()
            .map(|r| (r.text.clone(), label_font))
            .collect()];
    }
    wrapped
        .into_iter()
        .map(|line| vec![(line, label_font)])
        .collect()
}

/// First authored run size (`extractDrawingShapeTextStyle` / `projectDrawingShapes`).
fn shape_label_font(shape: &SpBox, stroke: f64) -> f64 {
    shape
        .font_size_px
        .unwrap_or_else(|| default_label_font(shape, stroke))
}

/// Line box ratio from the first paragraph only (TS `projectDrawingShapes` shape fields).
fn shape_line_height_ratio(shape: &SpBox) -> f64 {
    let shape_text = shape.flat_text();
    let para = shape.paragraphs.first();
    let ln = para.and_then(|p| p.line_spacing_ratio).unwrap_or(1.0);
    let aft = para.and_then(|p| p.space_after_ratio).unwrap_or(0.0);
    drawing_text_base_line_height_ratio(&shape_text) * ln + aft
}

fn layout_shape_text(shape: &SpBox, stroke: f64) -> Vec<PositionedTextLine> {
    let label_font = shape_label_font(shape, stroke);
    let (_text_cx, text_cy, max_width, _) = text_layout_metrics(shape, stroke);
    let line_height_ratio = shape_line_height_ratio(shape);
    let mut logical_lines: Vec<(Vec<(String, f64)>, f64)> = Vec::new();
    for para in &shape.paragraphs {
        for line in paragraph_to_lines(para, label_font, label_font, max_width) {
            logical_lines.push((line, line_height_ratio));
        }
    }
    if logical_lines.is_empty() {
        return Vec::new();
    }
    let line_heights: Vec<f64> = logical_lines
        .iter()
        .map(|(_, ratio)| label_font * ratio)
        .collect();
    let total_h = line_heights.iter().sum::<f64>();
    let mut y = text_cy - total_h / 2.0;
    logical_lines
        .into_iter()
        .zip(line_heights)
        .map(|((spans, _), lh)| {
            y += lh / 2.0;
            let line = PositionedTextLine { y, spans };
            y += lh / 2.0;
            line
        })
        .collect()
}

fn spacing_pct_from_p_pr(p_pr: &Element, local: &str) -> Option<f64> {
    let spacing = child_any(p_pr, local)?;
    let spc_pct = child_any(spacing, "spcPct")?;
    let val = attr_f64(spc_pct, "val")?;
    if val > 0.0 {
        Some(val / 100_000.0)
    } else {
        None
    }
}

fn append_shape_text(body: &mut String, shape: &SpBox, stroke: f64, origin_x: f64, origin_y: f64) {
    let lines = layout_shape_text(shape, stroke);
    if lines.is_empty() {
        return;
    }
    let (text_cx, _, _, _) = text_layout_metrics(shape, stroke);
    let text_cx = text_cx - origin_x;
    let label_font = shape_label_font(shape, stroke);
    let kerning_attr = match resolve_font_kerning(shape.kerning_hundredths_pt, label_font) {
        Some(kw) => format!(" style=\"font-kerning:{}\"", kw),
        None => String::new(),
    };
    body.push_str(&format!(
        "<text x=\"{}\" text-anchor=\"middle\" dominant-baseline=\"central\" font-size=\"{}\" fill=\"{}\"{kerning_attr}>",
        svg_user_coord(text_cx),
        svg_user_coord(label_font),
        escape(&shape.font_color),
    ));
    for line in lines {
        let line_y = line.y - origin_y;
        if line.spans.len() == 1 {
            let (text, _) = &line.spans[0];
            body.push_str(&format!(
                "<tspan x=\"{}\" y=\"{}\">{}</tspan>",
                svg_user_coord(text_cx),
                svg_user_coord(line_y),
                escape(text),
            ));
            continue;
        }
        for (i, (text, font)) in line.spans.iter().enumerate() {
            if i == 0 {
                body.push_str(&format!(
                    "<tspan x=\"{}\" y=\"{}\" font-size=\"{}\">{}</tspan>",
                    svg_user_coord(text_cx),
                    svg_user_coord(line_y),
                    svg_user_coord(*font),
                    escape(text),
                ));
            } else {
                body.push_str(&format!(
                    "<tspan font-size=\"{}\">{}</tspan>",
                    svg_user_coord(*font),
                    escape(text),
                ));
            }
        }
    }
    body.push_str("</text>");
}

fn num(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

/// Match TS SVG attribute number serialization (no `num()` 2dp rounding).
fn svg_user_coord(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_owned();
    }
    let rounded = v.round();
    if (v - rounded).abs() < 1e-9 {
        return format!("{}", rounded as i64);
    }
    let s = format!("{:.12}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn collect_shapes(root: &Element, scheme: Option<&BTreeMap<String, String>>) -> Vec<SpBox> {
    let mut out = Vec::new();
    walk_shapes(root, scheme, &mut out);
    resolve_label_font_colors(&mut out);
    out
}

fn walk_shapes(el: &Element, scheme: Option<&BTreeMap<String, String>>, out: &mut Vec<SpBox>) {
    // Iterative: recursive walks overflow the WASM stack at MAX_XML_DEPTH (#178).
    let mut stack: Vec<&Element> = vec![el];
    while let Some(node) = stack.pop() {
        if node.local == "sp" || node.local == "pic" {
            if let Some(box_) = sp_box(node, scheme, node.local == "pic") {
                out.push(box_);
            }
            continue;
        }
        for child in node.children.iter().rev() {
            if let Node::Element(child) = child {
                stack.push(child);
            }
        }
    }
}

fn sp_box(sp: &Element, scheme: Option<&BTreeMap<String, String>>, is_pic: bool) -> Option<SpBox> {
    let sp_pr = child_any(sp, "spPr")?;
    let xfrm = child_any(sp_pr, "xfrm")?;
    let off = child_any(xfrm, "off")?;
    let ext = child_any(xfrm, "ext")?;
    // Match TS `parseDrawingShape`: geometry + labels live in rounded CSS px.
    let x = smartart_drawing_emu_attr_px(off, "x")?;
    let y = smartart_drawing_emu_attr_px(off, "y")?;
    let w = smartart_drawing_emu_attr_px(ext, "cx")?;
    let h = smartart_drawing_emu_attr_px(ext, "cy")?;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let no_fill = child_any(sp_pr, "noFill").is_some();
    let (fill, fill_opacity) = if no_fill {
        ("none".to_owned(), None)
    } else {
        let fill = solid_hex(sp_pr, scheme).unwrap_or_else(|| "#D6DCE4".to_owned());
        (fill, solid_fill_alpha(sp_pr))
    };
    let (stroke, stroke_width_emu) = stroke_of(sp_pr, scheme);
    let stroke_width = if stroke_width_emu > 0.0 {
        smartart_ln_width_emu_to_svg(stroke_width_emu)
    } else {
        0.0
    };
    let paragraphs = text_paragraphs_of(sp);
    let text_frame = text_frame_of(sp);
    let (font_color, label_font_from_ref) = match font_ref_color(sp, scheme) {
        Some(c) => (c, true),
        None => (String::new(), false),
    };
    let font_size_px = authored_font_size_px(sp);
    let kerning_hundredths_pt = authored_kern_hundredths_pt(sp);
    let rotation = attr_f64(xfrm, "rot").map(|r| r / 60_000.0);
    let flip_h =
        attr_str(xfrm, "flipH").is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    let flip_v =
        attr_str(xfrm, "flipV").is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));

    let parsed = parse_drawing_geometry(Some(sp_pr));
    let geom_name = parsed.geometry.clone();
    let kind = resolve_kind(
        &parsed.geometry,
        w,
        h,
        parsed.adjustments.as_ref(),
        parsed.paths.as_deref(),
    );
    let kind =
        smartart_custgeom_paint_kind(&geom_name, &paragraphs, no_fill, stroke_width, w, h, kind);

    Some(SpBox {
        x,
        y,
        w,
        h,
        fill,
        fill_opacity,
        stroke,
        stroke_width,
        paragraphs,
        text_frame,
        font_color,
        label_font_from_ref,
        font_size_px,
        kerning_hundredths_pt,
        geom_name,
        kind,
        is_pic,
        rotation,
        flip_h,
        flip_v,
    })
}

fn fill_opacity_attr(opacity: Option<f64>) -> String {
    match opacity {
        Some(o) if o < 0.999 => format!(" fill-opacity=\"{}\"", num(o.clamp(0.0, 1.0))),
        _ => String::new(),
    }
}

fn solid_fill_alpha(container: &Element) -> Option<f64> {
    let fill = child_any(container, "solidFill")?;
    let color = child_any(fill, "srgbClr").or_else(|| child_any(fill, "schemeClr"))?;
    let alpha = child_any(color, "alpha").and_then(|e| attr_str(e, "val"))?;
    let v: f64 = alpha.parse().ok()?;
    Some((v / 100_000.0).clamp(0.0, 1.0))
}

fn font_ref_color(sp: &Element, scheme: Option<&BTreeMap<String, String>>) -> Option<String> {
    let style = child_any(sp, "style")?;
    let font_ref = child_any(style, "fontRef")?;
    solid_hex_from_color_child(font_ref, scheme)
}

fn shape_label_center(shape: &SpBox) -> (f64, f64) {
    if let Some(tf) = shape.text_frame.as_ref() {
        return (tf.x + tf.w / 2.0, tf.y + tf.h / 2.0);
    }
    (shape.x + shape.w / 2.0, shape.y + shape.h / 2.0)
}

/// Paint-order search for a filled shape under this label (TS `underlyingFill`).
fn underlying_fill(shape: &SpBox, shapes: &[SpBox], index: usize) -> Option<String> {
    let (cx, cy) = shape_label_center(shape);
    for below in (0..index).rev() {
        let cand = &shapes[below];
        if cand.fill == "none" || cand.is_pic {
            continue;
        }
        if cx >= cand.x && cx <= cand.x + cand.w && cy >= cand.y && cy <= cand.y + cand.h {
            return Some(blend_onto_white(&cand.fill, cand.fill_opacity));
        }
    }
    None
}

fn blend_onto_white(hex: &str, opacity: Option<f64>) -> String {
    let opacity = opacity.filter(|o| *o < 0.999).unwrap_or(1.0);
    if opacity >= 1.0 || !hex.starts_with('#') {
        return hex.to_owned();
    }
    let raw = hex.trim_start_matches('#');
    if raw.len() != 6 {
        return hex.to_owned();
    }
    let parse = |s: &str| u8::from_str_radix(s, 16).unwrap_or(128);
    let r = parse(&raw[0..2]);
    let g = parse(&raw[2..4]);
    let b = parse(&raw[4..6]);
    let blend = |c: u8| ((f64::from(c) * opacity) + 255.0 * (1.0 - opacity)).round() as u8;
    format!("#{:02X}{:02X}{:02X}", blend(r), blend(g), blend(b))
}

fn contrast_text_color(fill_basis: &str) -> String {
    if relative_luminance(fill_basis) > 0.55 {
        "#1a1a1a".to_owned()
    } else {
        "#FFFFFF".to_owned()
    }
}

fn label_color_for_shape(shapes: &[SpBox], index: usize) -> String {
    let shape = &shapes[index];
    let basis = if shape.fill != "none" {
        Some(blend_onto_white(&shape.fill, shape.fill_opacity))
    } else {
        underlying_fill(shape, shapes, index)
    };
    basis
        .map(|b| contrast_text_color(&b))
        .unwrap_or_else(|| "#1a1a1a".to_owned())
}

fn resolve_label_font_colors(shapes: &mut [SpBox]) {
    let colors: Vec<String> = shapes
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if s.label_font_from_ref {
                s.font_color.clone()
            } else {
                label_color_for_shape(shapes, i)
            }
        })
        .collect();
    for (shape, color) in shapes.iter_mut().zip(colors) {
        if !shape.label_font_from_ref {
            shape.font_color = color;
        }
    }
}

fn authored_font_size_px(sp: &Element) -> Option<f64> {
    // Match TS smartart-drawing-shape-style: `(sz/100) * (96/72)` CSS px.
    let mut stack: Vec<&Element> = vec![sp];
    while let Some(node) = stack.pop() {
        if node.local == "rPr" || node.local == "defRPr" {
            if let Some(sz) = attr_f64(node, "sz") {
                if sz > 0.0 {
                    return Some((sz / 100.0) * (96.0 / 72.0));
                }
            }
        }
        for child in node.children.iter().rev() {
            if let Node::Element(child) = child {
                stack.push(child);
            }
        }
    }
    None
}

fn authored_kern_hundredths_pt(sp: &Element) -> Option<i64> {
    let mut stack: Vec<&Element> = vec![sp];
    while let Some(node) = stack.pop() {
        if node.local == "rPr" {
            if let Some(kern) = attr_str(node, "kern") {
                if let Ok(v) = kern.parse::<i64>() {
                    return Some(v);
                }
            }
        }
        for child in node.children.iter().rev() {
            if let Node::Element(child) = child {
                stack.push(child);
            }
        }
    }
    None
}

/// Match upstream `resolveFontKerning` (`text-run-style.ts`).
fn resolve_font_kerning(
    kern_hundredths_pt: Option<i64>,
    font_size_px: f64,
) -> Option<&'static str> {
    let kern = kern_hundredths_pt?;
    if kern == 0 {
        return Some("none");
    }
    let font_pt = font_size_px * (72.0 / 96.0);
    if font_pt >= kern as f64 / 100.0 {
        Some("normal")
    } else {
        Some("none")
    }
}

fn relative_luminance(hex: &str) -> f64 {
    let raw = hex.trim_start_matches('#');
    if raw.len() != 6 {
        return 0.5;
    }
    let Ok(r) = u8::from_str_radix(&raw[0..2], 16) else {
        return 0.5;
    };
    let Ok(g) = u8::from_str_radix(&raw[2..4], 16) else {
        return 0.5;
    };
    let Ok(b) = u8::from_str_radix(&raw[4..6], 16) else {
        return 0.5;
    };
    (0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b)) / 255.0
}

/// Match TS `projectDrawingShapes` + `getPresetShapeVectorGeometry` for cached
/// `shapeType: "custom"`: preset clip-path resolution fails, so `outlinePathData`
/// strokes the shape box as a closed rectangle instead of the authored polyline.
fn smartart_custgeom_paint_kind(
    geom_name: &str,
    paragraphs: &[TextParagraph],
    no_fill: bool,
    stroke_width: f64,
    w: f64,
    h: f64,
    kind: ShapeKind,
) -> ShapeKind {
    if geom_name != "custom" || !no_fill || stroke_width <= 0.0 {
        return kind;
    }
    let has_text = paragraphs
        .iter()
        .any(|p| p.runs.iter().any(|r| !r.text.trim().is_empty()));
    if has_text {
        return kind;
    }
    ShapeKind::Path {
        d: outline_rect_path_data(w, h),
    }
}

fn outline_rect_path_data(width: f64, height: f64) -> String {
    let w = width.max(1.0);
    let h = height.max(1.0);
    format!(
        "M 0 0 L {} 0 L {} {} L 0 {} Z",
        svg_user_coord(w),
        svg_user_coord(w),
        svg_user_coord(h),
        svg_user_coord(h)
    )
}

fn resolve_kind(
    geometry: &str,
    w: f64,
    h: f64,
    adjustments: Option<&std::collections::HashMap<String, i64>>,
    custom_paths: Option<&[DrawingPath]>,
) -> ShapeKind {
    let normalized = geometry.trim().to_ascii_lowercase();
    if normalized == "custom" {
        if let Some(paths) = custom_paths {
            if let Some(d) = custom_paths_to_svg(paths, w, h) {
                return ShapeKind::Path { d };
            }
        }
        return ShapeKind::Rect { rx: 0.0 };
    }
    if normalized == "ellipse" || normalized == "oval" {
        return ShapeKind::Ellipse;
    }
    let identity = matches!(
        normalized.as_str(),
        "rect" | "flowchartprocess" | "" | "roundrect"
    );
    if identity {
        let rx = if normalized == "roundrect" {
            w.min(h) * 0.1
        } else {
            0.0
        };
        return ShapeKind::Rect { rx };
    }
    let adj = adjustments.map(|m| {
        m.iter()
            .map(|(k, v)| (k.clone(), *v as f64))
            .collect::<std::collections::HashMap<_, _>>()
    });
    // Match TS SmartArt paint: adjustment-aware circularArrow polygon before ECMA evaluator.
    if normalized == "circulararrow" {
        if let Some(d) = circular_arrow::circular_arrow_path_d(w, h, adj.as_ref()) {
            return ShapeKind::Path { d };
        }
    }
    if let Some(eval) = evaluate_preset_shape(geometry, w, h, adj.as_ref()) {
        if !eval.svg_path.is_empty() {
            return ShapeKind::Path { d: eval.svg_path };
        }
    }
    ShapeKind::Rect { rx: 0.0 }
}

fn custom_paths_to_svg(paths: &[DrawingPath], shape_w: f64, shape_h: f64) -> Option<String> {
    let mut parts = Vec::new();
    for path in paths {
        let space_w = path.width.filter(|v| *v > 0.0);
        let space_h = path.height.filter(|v| *v > 0.0);
        let in_path_space = space_w.is_some() || space_h.is_some();
        let scale_x = space_w.map(|sw| shape_w / sw).unwrap_or(1.0);
        let scale_y = space_h.map(|sh| shape_h / sh).unwrap_or(1.0);
        let sx = |v: f64| {
            if in_path_space {
                v * scale_x
            } else {
                emu_to_pixels(v as i64)
            }
        };
        let sy = |v: f64| {
            if in_path_space {
                v * scale_y
            } else {
                emu_to_pixels(v as i64)
            }
        };
        let mut pen_x = 0.0;
        let mut pen_y = 0.0;
        for cmd in &path.commands {
            match cmd {
                DrawingPathCommand::M { x, y } => {
                    let x = sx(*x);
                    let y = sy(*y);
                    parts.push(format!("M {} {}", num(x), num(y)));
                    pen_x = x;
                    pen_y = y;
                }
                DrawingPathCommand::L { x, y } => {
                    let x = sx(*x);
                    let y = sy(*y);
                    parts.push(format!("L {} {}", num(x), num(y)));
                    pen_x = x;
                    pen_y = y;
                }
                DrawingPathCommand::C {
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                } => {
                    let x1 = sx(*x1);
                    let y1 = sy(*y1);
                    let x2 = sx(*x2);
                    let y2 = sy(*y2);
                    let x = sx(*x);
                    let y = sy(*y);
                    parts.push(format!(
                        "C {} {} {} {} {} {}",
                        num(x1),
                        num(y1),
                        num(x2),
                        num(y2),
                        num(x),
                        num(y)
                    ));
                    pen_x = x;
                    pen_y = y;
                }
                DrawingPathCommand::Q { x1, y1, x, y } => {
                    let x1 = sx(*x1);
                    let y1 = sy(*y1);
                    let x = sx(*x);
                    let y = sy(*y);
                    parts.push(format!("Q {} {} {} {}", num(x1), num(y1), num(x), num(y)));
                    pen_x = x;
                    pen_y = y;
                }
                DrawingPathCommand::A {
                    w_r,
                    h_r,
                    start_angle,
                    swing_angle,
                } => {
                    // geometry_parse stores degrees; ooxml_arc_to_svg expects OOXML angle units
                    // (degrees * 60000) via guide_eval::angle_to_radians.
                    let st = *start_angle * 60_000.0;
                    let sw = *swing_angle * 60_000.0;
                    if let Some(result) = ooxml_arc_to_svg(sx(*w_r), sy(*h_r), st, sw, pen_x, pen_y)
                    {
                        parts.push(result.svg);
                        pen_x = result.end_x;
                        pen_y = result.end_y;
                    }
                }
                DrawingPathCommand::Z => parts.push("Z".to_owned()),
            }
        }
    }
    let d = parts.join(" ");
    if d.is_empty() {
        None
    } else {
        Some(d)
    }
}

fn stroke_of(sp_pr: &Element, scheme: Option<&BTreeMap<String, String>>) -> (String, f64) {
    let Some(ln) = child_any(sp_pr, "ln") else {
        return ("none".to_owned(), 0.0);
    };
    if child_any(ln, "noFill").is_some() {
        return ("none".to_owned(), 0.0);
    }
    let width = attr_f64(ln, "w").unwrap_or(0.0);
    let color = solid_hex(ln, scheme).unwrap_or_else(|| "#666666".to_owned());
    (color, width)
}

fn solid_hex(container: &Element, scheme: Option<&BTreeMap<String, String>>) -> Option<String> {
    let fill = child_any(container, "solidFill")?;
    solid_hex_from_color_child(fill, scheme)
}

fn solid_hex_from_color_child(
    container: &Element,
    scheme: Option<&BTreeMap<String, String>>,
) -> Option<String> {
    let color = parse_drawing_color_in(Some(container))?;
    let theme = scheme.map(SchemeMap);
    let theme_ref = theme.as_ref().map(|t| t as &dyn DrawingColorTheme);
    let resolved = resolve_drawing_color(&color, theme_ref, None)?;
    if resolved.hex.is_empty() {
        return None;
    }
    Some(resolved.hex)
}

fn paragraphs_plain_text(paragraphs: &[TextParagraph]) -> String {
    paragraphs
        .iter()
        .map(|para| {
            para.runs
                .iter()
                .map(|r| r.text.as_str())
                .collect::<String>()
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn emu_attr_px(el: &Element, name: &str) -> Option<f64> {
    attr_f64(el, name).map(|v| emu_to_pixels(v as i64))
}

fn text_frame_of(sp: &Element) -> Option<TextFrameGeom> {
    let tx_xfrm = child_any(sp, "txXfrm")?;
    let off = child_any(tx_xfrm, "off")?;
    let ext = child_any(tx_xfrm, "ext")?;
    let x = emu_attr_px(off, "x")?;
    let y = emu_attr_px(off, "y")?;
    let w = emu_attr_px(ext, "cx")?;
    let h = emu_attr_px(ext, "cy")?;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let mut insets = TextInsets::default();
    if let Some(tx_body) = child_any(sp, "txBody") {
        if let Some(body_pr) = child_any(tx_body, "bodyPr") {
            insets.left = emu_attr_px(body_pr, "lIns").unwrap_or(0.0);
            insets.right = emu_attr_px(body_pr, "rIns").unwrap_or(0.0);
            insets.top = emu_attr_px(body_pr, "tIns").unwrap_or(0.0);
            insets.bottom = emu_attr_px(body_pr, "bIns").unwrap_or(0.0);
        }
    }
    Some(TextFrameGeom { x, y, w, h, insets })
}

fn run_font_size_px(r_pr: &Element) -> Option<f64> {
    attr_f64(r_pr, "sz").and_then(|sz| {
        if sz > 0.0 {
            Some((sz / 100.0) * (96.0 / 72.0))
        } else {
            None
        }
    })
}

fn text_paragraphs_of(sp: &Element) -> Vec<TextParagraph> {
    let tx_body = child_any(sp, "txBody");
    let mut paragraphs = Vec::new();
    if let Some(tx_body) = tx_body {
        collect_tx_paragraphs(tx_body, &mut paragraphs);
    }
    if paragraphs.is_empty() {
        let mut flat = Vec::new();
        collect_text(sp, &mut flat);
        let joined = flat.join("").trim().to_owned();
        if !joined.is_empty() {
            paragraphs.push(TextParagraph {
                runs: vec![TextRun {
                    text: joined,
                    font_size_px: None,
                }],
                line_spacing_ratio: None,
                space_after_ratio: None,
            });
        }
    }
    paragraphs
}

fn collect_tx_paragraphs(tx_body: &Element, out: &mut Vec<TextParagraph>) {
    let mut stack: Vec<&Element> = vec![tx_body];
    while let Some(node) = stack.pop() {
        if node.local == "p" {
            let runs = paragraph_runs(node);
            if runs.iter().any(|r| !r.text.is_empty()) {
                let p_pr = child_any(node, "pPr");
                out.push(TextParagraph {
                    runs,
                    line_spacing_ratio: p_pr.and_then(|p| spacing_pct_from_p_pr(p, "lnSpc")),
                    space_after_ratio: p_pr.and_then(|p| spacing_pct_from_p_pr(p, "spcAft")),
                });
            }
            continue;
        }
        for child in node.children.iter().rev() {
            if let Node::Element(child) = child {
                stack.push(child);
            }
        }
    }
}

fn paragraph_runs(paragraph: &Element) -> Vec<TextRun> {
    let mut runs = Vec::new();
    for child in &paragraph.children {
        let node = match child {
            Node::Element(e) => e,
            _ => continue,
        };
        if node.local != "r" {
            continue;
        }
        let r_pr = child_any(node, "rPr");
        let font_size_px = r_pr.and_then(run_font_size_px);
        let mut text_parts = Vec::new();
        collect_text(node, &mut text_parts);
        let text = text_parts.join("");
        if !text.is_empty() {
            runs.push(TextRun { text, font_size_px });
        }
    }
    if runs.is_empty() {
        let mut text_parts = Vec::new();
        collect_text(paragraph, &mut text_parts);
        let text = text_parts.join("").trim().to_owned();
        if !text.is_empty() {
            runs.push(TextRun {
                text,
                font_size_px: None,
            });
        }
    }
    runs
}

fn collect_text(el: &Element, parts: &mut Vec<String>) {
    // Iterative: recursive walks overflow the WASM stack at MAX_XML_DEPTH (#178).
    let mut stack: Vec<&Element> = vec![el];
    while let Some(node) = stack.pop() {
        if node.local == "t" {
            let t = node.text_content();
            if !t.is_empty() {
                parts.push(t);
            }
            continue;
        }
        for child in node.children.iter().rev() {
            if let Node::Element(child) = child {
                stack.push(child);
            }
        }
    }
}

fn child_any<'a>(el: &'a Element, local: &str) -> Option<&'a Element> {
    el.children.iter().find_map(|n| match n {
        Node::Element(e) if e.local == local => Some(e),
        _ => None,
    })
}

fn attr_str<'a>(el: &'a Element, local: &str) -> Option<&'a str> {
    el.attrs
        .iter()
        .find(|a| a.local == local && a.ns.is_none())
        .map(|a| a.value.as_str())
}

fn attr_f64(el: &Element, local: &str) -> Option<f64> {
    attr_str(el, local)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use nitro_xml_opc::xml::parse_xml_str;

    use super::{
        collect_shapes, collect_text, smartart_draw_args, smartart_svg, smartart_svg_with_scheme,
    };

    // Catches: a cached dsp:sp drawing collapsed to the labelled SmartArt stub.
    #[test]
    fn cached_drawing_emits_sp_rect_and_text() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="914400" cy="457200"/></a:xfrm>
                <a:solidFill><a:srgbClr val="FF0000"/></a:solidFill>
              </dsp:spPr>
              <dsp:txBody><a:p><a:r><a:t>Box</a:t></a:r></a:p></dsp:txBody>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let svg = smartart_svg(Some(xml), 320.0, 180.0);
        assert!(svg.contains("Box"), "{svg}");
        assert!(svg.contains("#FF0000"), "{svg}");
        assert!(svg.contains("<rect "), "{svg}");
        assert!(!svg.contains(">SmartArt<"), "{svg}");
    }

    // Catches: a short labelled shape beside a wide one panics in f64::clamp (wasm abort, then "recursive use" on the stuck handle) and a textless frame paints as a pale placeholder.
    #[test]
    fn thin_labelled_shape_keeps_scheme_fill_without_empty_frames() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="8000000" cy="2000000"/></a:xfrm>
                <a:solidFill><a:schemeClr val="accent1"/></a:solidFill>
              </dsp:spPr>
              <dsp:txBody><a:p><a:r><a:t>Manager</a:t></a:r></a:p></dsp:txBody>
            </dsp:sp>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="2100000"/><a:ext cx="400000" cy="90000"/></a:xfrm>
                <a:solidFill><a:srgbClr val="00AA88"/></a:solidFill>
              </dsp:spPr>
              <dsp:txBody><a:p><a:r><a:t>Thin</a:t></a:r></a:p></dsp:txBody>
            </dsp:sp>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="1000" y="1000"/><a:ext cx="3000000" cy="90000"/></a:xfrm>
                <a:noFill/>
              </dsp:spPr>
            </dsp:sp>
            <dsp:pic>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="900000" cy="400000"/></a:xfrm>
                <a:solidFill><a:srgbClr val="EEEEEE"/></a:solidFill>
              </dsp:spPr>
            </dsp:pic>
          </dsp:spTree>
        </dsp:drawing>"#;
        let mut scheme = BTreeMap::new();
        scheme.insert("accent1".to_owned(), "#156082".to_owned());
        let svg = smartart_svg_with_scheme(Some(xml), 400.0, 220.0, Some(&scheme));
        assert!(svg.contains("Manager"), "{svg}");
        assert!(svg.contains("Thin"), "{svg}");
        assert!(svg.contains("#156082"), "{svg}");
        assert!(svg.contains("#00AA88"), "{svg}");
        assert!(!svg.contains("#D6DCE4"), "{svg}");
        assert!(!svg.contains("#EEEEEE"), "{svg}");
        assert!(!svg.contains(">SmartArt<"), "{svg}");
    }

    // Catches: dark chevron labels painted #222 on accent1 (invisible) instead of fontRef lt1 white.
    #[test]
    fn font_ref_lt1_paints_white_label_on_dark_fill() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="2000000" cy="1000000"/></a:xfrm>
                <a:prstGeom prst="chevron"><a:avLst/></a:prstGeom>
                <a:solidFill><a:schemeClr val="accent1"/></a:solidFill>
              </dsp:spPr>
              <dsp:style><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></dsp:style>
              <dsp:txBody><a:p><a:r><a:rPr sz="3100"/><a:t>Node One</a:t></a:r></a:p></dsp:txBody>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let mut scheme = BTreeMap::new();
        scheme.insert("accent1".to_owned(), "#156082".to_owned());
        scheme.insert("lt1".to_owned(), "#FFFFFF".to_owned());
        let svg = smartart_svg_with_scheme(Some(xml), 400.0, 200.0, Some(&scheme));
        assert!(svg.contains(">Node<") && svg.contains(">One<"), "{svg}");
        assert!(svg.contains("fill=\"#FFFFFF\""), "{svg}");
        assert!(svg.contains("#156082"), "{svg}");
        // Catches: EMU font-size clamped by Chromium (~10000px) → invisible labels.
        assert!(
            svg.contains("font-size=\"41.33"),
            "authored 31pt → ~41.33px; got {svg}"
        );
    }

    // Catches: SmartArt tint inverted (40% kept as 40% toward white) so chevron children stay teal not grey.
    #[test]
    fn scheme_tint_keeps_fraction_of_accent_toward_white() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="1000000" cy="500000"/></a:xfrm>
                <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                <a:solidFill><a:schemeClr val="accent1"><a:tint val="40000"/></a:schemeClr></a:solidFill>
              </dsp:spPr>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let mut scheme = BTreeMap::new();
        scheme.insert("accent1".to_owned(), "#156082".to_owned());
        let svg = smartart_svg_with_scheme(Some(xml), 200.0, 100.0, Some(&scheme));
        // Linear-light tint 40% of #156082 (not the inverted sRGB lerp #73A0B4).
        assert!(
            !svg.contains("#73A0B4") && !svg.contains("#73a0b4"),
            "inverted tint: {svg}"
        );
        assert!(svg.contains("fill=\"#"), "{svg}");
    }

    // Catches: EMU viewBox + EMU font-size collapses under Chromium's CSS font-size clamp.
    #[test]
    fn geometry_and_font_are_in_css_pixels() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="9525" y="19050"/><a:ext cx="952500" cy="476250"/></a:xfrm>
                <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                <a:solidFill><a:srgbClr val="112233"/></a:solidFill>
              </dsp:spPr>
              <dsp:txBody><a:p><a:r><a:t>Px</a:t></a:r></a:p></dsp:txBody>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let svg = smartart_svg(Some(xml), 200.0, 100.0);
        assert!(
            svg.contains("viewBox=\"0 0 100 50\"") || svg.contains("viewBox=\"0 0 100.0 50.0\""),
            "{svg}"
        );
        // Default label size clamped to 8..14 px (height*0.2 = 10).
        assert!(svg.contains("font-size=\"10\""), "{svg}");
    }

    // Catches: SmartArt xfrm extents stay fractional (139910 EMU → 14.69px) so connector
    // outline rects and node boxes drift from TS Math.round(EMU/9525).
    #[test]
    fn drawing_shape_extents_round_like_parse_drawing_shape() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="6381238" y="669943"/><a:ext cx="139910" cy="612939"/></a:xfrm>
                <a:custGeom><a:pathLst><a:path><a:moveTo><a:pt x="139910" y="0"/></a:moveTo></a:path></a:pathLst></a:custGeom>
                <a:noFill/>
                <a:ln w="19050"><a:solidFill><a:srgbClr val="34599C"/></a:solidFill></a:ln>
              </dsp:spPr>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let args = smartart_draw_args(Some(xml), None);
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].w, 15.0);
        assert_eq!(args[0].h, 64.0);
        let svg = smartart_svg(Some(xml), 200.0, 200.0);
        assert!(
            svg.contains("L 15 0 L 15 64"),
            "outline rect must use rounded box: {svg}"
        );
        assert!(
            !svg.contains("14.69"),
            "fractional extent leaks into path: {svg}"
        );
    }

    // Catches: noFill text frames inventing a grey stroke rectangle around SmartArt labels.
    #[test]
    fn text_only_nofill_frame_emits_text_without_body() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="1000000" cy="1000000"/></a:xfrm>
                <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                <a:noFill/>
                <a:ln><a:noFill/></a:ln>
              </dsp:spPr>
              <dsp:txBody><a:p><a:r><a:t>Label</a:t></a:r></a:p></dsp:txBody>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let svg = smartart_svg(Some(xml), 200.0, 200.0);
        assert!(svg.contains("Label"), "{svg}");
        assert!(!svg.contains("<rect "), "{svg}");
        assert!(!svg.contains("<path "), "{svg}");
    }

    // Catches: text-cycle SmartArt painting circularArrow presets as axis-aligned rects / dropping them when textless.
    #[test]
    fn circular_arrow_preset_emits_path_draw_arg() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="1000000" cy="1000000"/></a:xfrm>
                <a:prstGeom prst="circularArrow"><a:avLst/></a:prstGeom>
                <a:solidFill><a:srgbClr val="156082"/></a:solidFill>
              </dsp:spPr>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let args = smartart_draw_args(Some(xml), None);
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].kind, "path");
        assert_eq!(args[0].geom, "circularArrow");
        assert!(args[0].path_len > 200, "{args:?}");
        let svg = smartart_svg(Some(xml), 200.0, 200.0);
        assert!(svg.contains("<path "), "{svg}");
        assert!(!svg.contains("<rect "), "{svg}");
        assert!(
            !svg.contains(" A "),
            "TS paint uses adjustment-aware polygon, not arcTo: {svg}"
        );
    }

    // Catches: orgchart fan connectors (textless custGeom + noFill + ln) painted as polylines instead of TS outlinePathData box rects.
    #[test]
    fn custgeom_stroke_connector_kept() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="183641" cy="804522"/></a:xfrm>
                <a:custGeom>
                  <a:pathLst>
                    <a:path>
                      <a:moveTo><a:pt x="183641" y="0"/></a:moveTo>
                      <a:lnTo><a:pt x="183641" y="804522"/></a:lnTo>
                      <a:lnTo><a:pt x="0" y="804522"/></a:lnTo>
                    </a:path>
                  </a:pathLst>
                </a:custGeom>
                <a:noFill/>
                <a:ln w="19050"><a:solidFill><a:srgbClr val="156082"/></a:solidFill></a:ln>
              </dsp:spPr>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let args = smartart_draw_args(Some(xml), None);
        assert_eq!(args.len(), 1, "{args:?}");
        assert_eq!(args[0].kind, "path");
        assert_eq!(args[0].fill, "none");
        assert_eq!(args[0].stroke, "#156082");
        assert!(
            (args[0].stroke_width - 1.5).abs() < 0.01,
            "expected EMU/12700 stroke width, got {:?}",
            args[0].stroke_width
        );
        assert_eq!(args[0].w, 19.0);
        assert_eq!(args[0].h, 84.0);
        let svg = smartart_svg(Some(xml), 120.0, 200.0);
        assert!(svg.contains("<path "), "{svg}");
        assert!(svg.contains("fill=\"none\""), "{svg}");
        assert!(
            svg.contains("L 19 0 L 19 84"),
            "expected closed box outline path: {svg}"
        );
        assert!(
            svg.contains("stroke-width=\"1.5\""),
            "TS parseDrawingShape uses 19050/12700 = 1.5px: {svg}"
        );
    }

    // Catches: missing drawing XML painting an empty frame instead of a label.
    #[test]
    fn missing_drawing_is_labelled_smartart() {
        let svg = smartart_svg(None, 200.0, 80.0);
        assert!(svg.contains(">SmartArt<"), "{svg}");
        assert!(svg.contains("width=\"200\""), "{svg}");
    }

    // Catches: SmartArt labels emitted as one <text> per wrapped line so Chromium recenters each line independently vs TS appendSvgTextLines tspans.
    #[test]
    fn label_lines_use_single_text_with_positioned_tspans() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="2000000" cy="800000"/></a:xfrm>
                <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                <a:solidFill><a:srgbClr val="156082"/></a:solidFill>
              </dsp:spPr>
              <dsp:txBody>
                <a:p><a:r><a:rPr sz="2400" kern="1200"/><a:t>Alpha</a:t></a:r></a:p>
                <a:p><a:r><a:rPr sz="2400" kern="1200"/><a:t>Gamma</a:t></a:r></a:p>
              </dsp:txBody>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let svg = smartart_svg(Some(xml), 300.0, 120.0);
        assert_eq!(
            svg.matches("<text ").count(),
            1,
            "one text block per shape: {svg}"
        );
        assert_eq!(
            svg.matches("<tspan ").count(),
            2,
            "one tspan per line: {svg}"
        );
        assert!(
            svg.contains("font-kerning:normal"),
            "kern=1200 at 24pt enables kerning: {svg}"
        );
        assert!(
            svg.contains("x=\"105\"") && svg.contains("<tspan x=\"105\""),
            "tspan repeats parent anchor x: {svg}"
        );
    }

    // Catches: chevron labels centred on shape box and wrapped at 0.82×w instead of dsp:txXfrm minus bodyPr insets.
    #[test]
    fn tx_xfrm_and_body_insets_drive_text_wrap_and_center() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="1905000" cy="952500"/></a:xfrm>
                <a:prstGeom prst="chevron"><a:avLst/></a:prstGeom>
                <a:solidFill><a:srgbClr val="156082"/></a:solidFill>
              </dsp:spPr>
              <dsp:txXfrm>
                <a:off x="0" y="238125"/><a:ext cx="476250" cy="476250"/>
              </dsp:txXfrm>
              <dsp:txBody>
                <a:bodyPr lIns="45720" rIns="45720" tIns="45720" bIns="45720"/>
                <a:p><a:r><a:t>Left</a:t></a:r></a:p>
              </dsp:txBody>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let svg = smartart_svg(Some(xml), 220.0, 120.0);
        // Text frame is the left quarter of the shape (~50px wide), not centred at x=100.
        assert!(
            svg.contains("x=\"25\"") || svg.contains("x=\"25.0\""),
            "expected label centred in txXfrm inset box, got {svg}"
        );
        assert!(
            !svg.contains("x=\"100\""),
            "shape-centred label should not appear: {svg}"
        );
    }

    // Catches: text-cycle flattening multiple a:p into one wrapped line; label font is first-run sz for all lines (TS projectDrawingShapes).
    #[test]
    fn paragraph_breaks_and_per_run_font_sizes_preserved() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="1000000" cy="1000000"/></a:xfrm>
                <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                <a:solidFill><a:srgbClr val="CCCCCC"/></a:solidFill>
              </dsp:spPr>
              <dsp:txBody>
                <a:p><a:r><a:rPr sz="2000"/><a:t>Small</a:t></a:r></a:p>
                <a:p><a:r><a:rPr sz="4000"/><a:t>Big</a:t></a:r></a:p>
              </dsp:txBody>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let svg = smartart_svg(Some(xml), 120.0, 120.0);
        assert!(svg.contains(">Small<"), "{svg}");
        assert!(svg.contains(">Big<"), "{svg}");
        assert!(
            svg.contains("font-size=\"26.67") || svg.contains("font-size=\"26.6"),
            "both paragraphs use first-run font size: {svg}"
        );
        assert!(
            !svg.contains("font-size=\"53.33") && !svg.contains("font-size=\"53.3"),
            "lvl-1 run sz must not override shape label font: {svg}"
        );
        assert!(
            !svg.contains(">Small Big<") && !svg.contains(">Small<Big<"),
            "paragraphs must not merge: {svg}"
        );
    }

    // Catches: SmartArt labels ignoring a:lnSpc/a:spcAft and always using 1.2× line height.
    #[test]
    fn paragraph_ln_spc_and_spc_aft_adjust_label_line_spacing() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="4000000" cy="2000000"/></a:xfrm>
                <a:solidFill><a:srgbClr val="156082"/></a:solidFill>
              </dsp:spPr>
              <dsp:txBody>
                <a:p>
                  <a:pPr>
                    <a:lnSpc><a:spcPct val="50000"/></a:lnSpc>
                    <a:spcAft><a:spcPct val="10000"/></a:spcAft>
                  </a:pPr>
                  <a:r><a:rPr sz="2400"/><a:t>Alpha</a:t></a:r>
                </a:p>
                <a:p><a:r><a:rPr sz="2400"/><a:t>Beta</a:t></a:r></a:p>
              </dsp:txBody>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let svg = smartart_svg(Some(xml), 400.0, 200.0);
        let ys: Vec<f64> = svg
            .split("<tspan ")
            .skip(1)
            .filter_map(|chunk| {
                chunk
                    .split("y=\"")
                    .nth(1)
                    .and_then(|rest| rest.split('"').next())
                    .and_then(|y| y.parse().ok())
            })
            .collect();
        assert_eq!(ys.len(), 2, "expected two label tspans: {svg}");
        let font_px = 24.0 * (96.0 / 72.0);
        // TS uses first-paragraph lnSpc/spcAft for every line in the shape label block.
        let expected_gap = font_px * (1.2 * 0.5 + 0.1);
        let actual_gap = (ys[1] - ys[0]).abs();
        assert!(
            (actual_gap - expected_gap).abs() < 0.5,
            "line gap {actual_gap} should reflect first-paragraph lnSpc+spcAft (~{expected_gap}): {svg}"
        );
    }

    // Catches: custGeom paths without a:path/@w|@h left in EMU so connectors collapse in viewBox.
    #[test]
    fn custgeom_without_path_space_scales_emu_to_shape_pixels() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="183641" cy="804522"/></a:xfrm>
                <a:custGeom>
                  <a:pathLst>
                    <a:path>
                      <a:moveTo><a:pt x="0" y="0"/></a:moveTo>
                      <a:lnTo><a:pt x="183641" y="804522"/></a:lnTo>
                    </a:path>
                  </a:pathLst>
                </a:custGeom>
                <a:noFill/>
                <a:ln w="19050"><a:solidFill><a:srgbClr val="156082"/></a:solidFill></a:ln>
              </dsp:spPr>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let svg = smartart_svg(Some(xml), 120.0, 200.0);
        assert!(svg.contains("<path "), "{svg}");
        assert!(
            !svg.contains("183641"),
            "path coords should be CSS px not EMU: {svg}"
        );
        assert!(
            svg.contains("L 19.28 84.46") || svg.contains("L 19.3 84.5") || svg.contains("L 19 84"),
            "{svg}"
        );
    }

    // Catches: SmartArt shape/text walks recursing once per nesting level and overflowing the WASM
    // stack at MAX_XML_DEPTH (4096).
    #[test]
    fn deep_nesting_shape_walk_does_not_overflow_stack() {
        let xml = format!("{}{}", "<a>".repeat(4096), "</a>".repeat(4096));
        let root = parse_xml_str(&xml).expect("parses").root;
        let shapes = collect_shapes(&root, None);
        assert!(shapes.is_empty());
        let mut parts = Vec::new();
        collect_text(&root, &mut parts);
        assert!(parts.is_empty());
    }
    // Catches: connector `a:ln` `schemeClr`+`shade` left full accent1 so orgchart lines miss TS stroke colour.
    #[test]
    fn connector_scheme_shade_on_ln_stroke() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="100000" cy="100000"/></a:xfrm>
                <a:custGeom><a:pathLst><a:path><a:moveTo><a:pt x="100" y="0"/></a:moveTo><a:lnTo><a:pt x="100" y="100"/></a:lnTo></a:path></a:pathLst></a:custGeom>
                <a:noFill/>
                <a:ln w="19050"><a:solidFill><a:schemeClr val="accent1"><a:shade val="60000"/><a:hueOff val="0"/><a:satOff val="0"/><a:lumOff val="0"/><a:alphaOff val="0"/></a:schemeClr></a:solidFill></a:ln>
              </dsp:spPr>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let mut scheme = BTreeMap::new();
        scheme.insert("accent1".to_owned(), "#156082".to_owned());
        let args = smartart_draw_args(Some(xml), Some(&scheme));
        assert_eq!(args[0].stroke, "#0E4B66", "got {:?}", args[0].stroke);
    }

    // Catches: noFill SmartArt label boxes keeping #222222 instead of contrast on the painted shape below.
    #[test]
    fn nofill_label_reads_contrast_from_shape_below() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="2000000" cy="1000000"/></a:xfrm>
                <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                <a:solidFill><a:srgbClr val="1f3864"/></a:solidFill>
              </dsp:spPr>
            </dsp:sp>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="100000" y="100000"/><a:ext cx="1800000" cy="800000"/></a:xfrm>
                <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                <a:noFill/>
              </dsp:spPr>
              <dsp:txBody><a:p><a:r><a:t>Heading</a:t></a:r></a:p></dsp:txBody>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let args = smartart_draw_args(Some(xml), None);
        assert_eq!(args.len(), 2);
        assert_eq!(args[1].font_color, "#FFFFFF", "{:?}", args);
    }

    // Catches: node labels ignoring `dsp:style/a:fontRef` `lt1` and using dark contrast on accent1 fill.
    #[test]
    fn draw_arg_carries_font_ref_label_colour() {
        let xml = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <dsp:spTree>
            <dsp:sp>
              <dsp:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="2000000" cy="1000000"/></a:xfrm>
                <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                <a:solidFill><a:schemeClr val="accent1"/></a:solidFill>
              </dsp:spPr>
              <dsp:style><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></dsp:style>
              <dsp:txBody><a:p><a:r><a:t>Team</a:t></a:r></a:p></dsp:txBody>
            </dsp:sp>
          </dsp:spTree>
        </dsp:drawing>"#;
        let mut scheme = BTreeMap::new();
        scheme.insert("accent1".to_owned(), "#156082".to_owned());
        scheme.insert("lt1".to_owned(), "#FFFFFF".to_owned());
        let args = smartart_draw_args(Some(xml), Some(&scheme));
        assert_eq!(args[0].font_color, "#FFFFFF", "{:?}", args);
    }
}
