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
    DrawingPathCommand,
};
use nitro_xml_opc::xml::{parse_xml_str, Element, Node};

/// `a:ln/@w` on diagram drawing shapes: TS `parseDrawingShape` maps EMU ÷ 12700 to SVG stroke-width.
const EMU_PER_SMARTART_LINE_WIDTH: f64 = 12700.0;

fn smartart_ln_width_emu_to_svg(emu: f64) -> f64 {
    emu / EMU_PER_SMARTART_LINE_WIDTH
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
    /// Authored font size in CSS px when `a:rPr/@sz` is present (`sz/100 * 96/72`).
    font_size_px: Option<f64>,
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

fn shapes_svg(shapes: &[SpBox], w: f64, h: f64) -> String {
    let min_x = shapes.iter().map(|s| s.x).fold(f64::INFINITY, f64::min);
    let min_y = shapes.iter().map(|s| s.y).fold(f64::INFINITY, f64::min);
    let max_x = shapes
        .iter()
        .map(|s| s.x + s.w)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = shapes
        .iter()
        .map(|s| s.y + s.h)
        .fold(f64::NEG_INFINITY, f64::max);
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
                "stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\"",
                escape(&shape.stroke),
                num(stroke_w)
            )
        };
        let cx = shape.x + shape.w / 2.0;
        let cy = shape.y + shape.h / 2.0;
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
                        num(shape.x),
                        num(shape.y),
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
                        Some(t) => format!("{t} translate({} {})", num(shape.x), num(shape.y)),
                        None => format!("translate({} {})", num(shape.x), num(shape.y)),
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
            append_shape_text(&mut body, shape, stroke_w.max(default_stroke));
        }
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"{} {} {} {}\" role=\"img\">{body}</svg>",
        num(min_x),
        num(min_y),
        num(vw),
        num(vh),
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

fn append_shape_text(body: &mut String, shape: &SpBox, stroke: f64) {
    let (text_cx, _, _, base_font) = text_layout_metrics(shape, stroke);
    for line in layout_shape_text(shape, stroke) {
        if line.spans.len() == 1 {
            let (text, font) = &line.spans[0];
            body.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" dominant-baseline=\"central\" font-size=\"{}\" fill=\"{}\">{}</text>",
                num(text_cx),
                num(line.y),
                num(*font),
                escape(&shape.font_color),
                escape(text),
            ));
            continue;
        }
        body.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" dominant-baseline=\"central\" font-size=\"{}\" fill=\"{}\">",
            num(text_cx),
            num(line.y),
            num(base_font),
            escape(&shape.font_color),
        ));
        for (text, font) in &line.spans {
            body.push_str(&format!(
                "<tspan font-size=\"{}\">{}</tspan>",
                num(*font),
                escape(text),
            ));
        }
        body.push_str("</text>");
    }
}

fn num(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
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
    // Match TS `parseDrawingShape`: geometry + labels live in CSS px (EMU / 9525).
    let x = emu_to_pixels(attr_f64(off, "x")? as i64);
    let y = emu_to_pixels(attr_f64(off, "y")? as i64);
    let w = emu_to_pixels(attr_f64(ext, "cx")? as i64);
    let h = emu_to_pixels(attr_f64(ext, "cy")? as i64);
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
    let font_color = font_color_of(sp, scheme, &fill);
    let font_size_px = authored_font_size_px(sp);
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
        font_size_px,
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

fn font_color_of(sp: &Element, scheme: Option<&BTreeMap<String, String>>, fill: &str) -> String {
    if let Some(style) = child_any(sp, "style") {
        if let Some(font_ref) = child_any(style, "fontRef") {
            if let Some(hex) = solid_hex_from_color_child(font_ref, scheme) {
                return hex;
            }
        }
    }
    // Contrast against fill (TS drawingShapeLabelColor): light fill → dark text.
    if fill != "none" && relative_luminance(fill) > 0.55 {
        "#222222".to_owned()
    } else if fill != "none" {
        "#FFFFFF".to_owned()
    } else {
        "#222222".to_owned()
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
        num(w),
        num(w),
        num(h),
        num(h)
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
            svg.contains("viewBox=\"1 2 100 50\"") || svg.contains("viewBox=\"1 2 100.0 50.0\""),
            "{svg}"
        );
        // Default label size clamped to 8..14 px (height*0.2 = 10).
        assert!(svg.contains("font-size=\"10\""), "{svg}");
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
        assert!(
            args[0].path_len > 33,
            "expected closed box outline path_len, got {:?}",
            args[0]
        );
        let svg = smartart_svg(Some(xml), 120.0, 200.0);
        assert!(svg.contains("<path "), "{svg}");
        assert!(svg.contains("fill=\"none\""), "{svg}");
        assert!(
            svg.contains(" Z\""),
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
            .split("<text ")
            .skip(1)
            .filter_map(|chunk| {
                chunk
                    .split("y=\"")
                    .nth(1)
                    .and_then(|rest| rest.split('"').next())
                    .and_then(|y| y.parse().ok())
            })
            .collect();
        assert_eq!(ys.len(), 2, "expected two label lines: {svg}");
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
}
