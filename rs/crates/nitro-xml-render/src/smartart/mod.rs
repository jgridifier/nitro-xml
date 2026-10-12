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

use std::collections::BTreeMap;

use nitro_xml_opc::drawingml::{
    evaluate_preset_shape, ooxml_arc_to_svg, parse_drawing_geometry, DrawingPath,
    DrawingPathCommand,
};
use nitro_xml_opc::xml::{parse_xml_str, Element, Node};

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
            text: s.text.clone(),
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

struct SpBox {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: String,
    fill_opacity: Option<f64>,
    stroke: String,
    stroke_width: f64,
    text: String,
    font_color: String,
    /// Authored font size in EMUs when `a:rPr/@sz` is present.
    font_size_emu: Option<f64>,
    geom_name: String,
    kind: ShapeKind,
    is_pic: bool,
    rotation: Option<f64>,
    flip_h: bool,
    flip_v: bool,
}

fn keep_shape(shape: &SpBox) -> bool {
    if !shape.text.is_empty() {
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

        if !shape.text.is_empty() {
            // Prefer authored `sz` (hundredths of a point → EMU); else ~20% of height.
            let font = shape
                .font_size_emu
                .unwrap_or(shape.h * 0.2)
                .min(fit_font(shape.h, stroke_w.max(default_stroke)))
                .clamp(shape.h * 0.08, shape.h * 0.35)
                .max(1.0);
            let line_height = font * 1.2;
            let max_width = (shape.w * 0.82).max(font);
            let lines = wrap_smartart_text(&shape.text, font, max_width);
            let total_h = line_height * lines.len().max(1) as f64;
            let mut y = cy - total_h / 2.0 + line_height / 2.0;
            for line in &lines {
                body.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" dominant-baseline=\"central\" font-size=\"{}\" fill=\"{}\">{}</text>",
                    num(cx),
                    num(y),
                    num(font),
                    escape(&shape.font_color),
                    escape(line),
                ));
                y += line_height;
            }
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

fn wrap_smartart_text(text: &str, font: f64, max_width: f64) -> Vec<String> {
    let char_w = font * 0.55;
    if char_w <= 0.0 || max_width <= 0.0 {
        return vec![text.to_owned()];
    }
    let max_chars = (max_width / char_w).floor().max(1.0) as usize;
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current = word.to_owned();
            continue;
        }
        if current.len() + 1 + word.len() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current = word.to_owned();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(text.to_owned());
    }
    lines
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
    let x = attr_f64(off, "x")?;
    let y = attr_f64(off, "y")?;
    let w = attr_f64(ext, "cx")?;
    let h = attr_f64(ext, "cy")?;
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
    let (stroke, stroke_width) = stroke_of(sp_pr, scheme);
    let text = text_of(sp);
    let font_color = font_color_of(sp, scheme, &fill);
    let font_size_emu = authored_font_size_emu(sp);
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

    Some(SpBox {
        x,
        y,
        w,
        h,
        fill,
        fill_opacity,
        stroke,
        stroke_width,
        text,
        font_color,
        font_size_emu,
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

fn authored_font_size_emu(sp: &Element) -> Option<f64> {
    // `sz` is hundredths of a point; 1pt = 12700 EMU.
    let mut stack: Vec<&Element> = vec![sp];
    while let Some(node) = stack.pop() {
        if node.local == "rPr" || node.local == "defRPr" {
            if let Some(sz) = attr_f64(node, "sz") {
                if sz > 0.0 {
                    return Some(sz / 100.0 * 12_700.0);
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

fn solid_hex_from_color_child(
    container: &Element,
    scheme: Option<&BTreeMap<String, String>>,
) -> Option<String> {
    if let Some(srgb) = child_any(container, "srgbClr") {
        let mut hex = hex6(attr_str(srgb, "val")?)?;
        apply_simple_tint_shade(srgb, &mut hex);
        return Some(hex);
    }
    let scheme_el = child_any(container, "schemeClr")?;
    let name = attr_str(scheme_el, "val")?;
    let mut hex = scheme_hex(scheme?, name)?;
    apply_simple_tint_shade(scheme_el, &mut hex);
    Some(hex)
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
        let scale_x = space_w.map(|sw| shape_w / sw).unwrap_or(1.0);
        let scale_y = space_h.map(|sh| shape_h / sh).unwrap_or(1.0);
        let sx = |v: f64| v * scale_x;
        let sy = |v: f64| v * scale_y;
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
    if let Some(srgb) = child_any(fill, "srgbClr") {
        let mut hex = hex6(attr_str(srgb, "val")?)?;
        apply_simple_tint_shade(srgb, &mut hex);
        return Some(hex);
    }
    let scheme_el = child_any(fill, "schemeClr")?;
    let name = attr_str(scheme_el, "val")?;
    let mut hex = scheme_hex(scheme?, name)?;
    apply_simple_tint_shade(scheme_el, &mut hex);
    Some(hex)
}

/// Apply DrawingML `tint` / `shade` (100000ths) onto a `#RRGGBB` fill for SmartArt.
fn apply_simple_tint_shade(color_el: &Element, hex: &mut String) {
    let tint = child_any(color_el, "tint")
        .and_then(|e| attr_str(e, "val"))
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| (v / 100_000.0).clamp(0.0, 1.0));
    let shade = child_any(color_el, "shade")
        .and_then(|e| attr_str(e, "val"))
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| (v / 100_000.0).clamp(0.0, 1.0));
    if tint.is_none() && shade.is_none() {
        return;
    }
    let raw = hex.trim_start_matches('#');
    if raw.len() != 6 {
        return;
    }
    let Ok(r) = u8::from_str_radix(&raw[0..2], 16) else {
        return;
    };
    let Ok(g) = u8::from_str_radix(&raw[2..4], 16) else {
        return;
    };
    let Ok(b) = u8::from_str_radix(&raw[4..6], 16) else {
        return;
    };
    let mut rf = f64::from(r);
    let mut gf = f64::from(g);
    let mut bf = f64::from(b);
    if let Some(t) = tint {
        rf += (255.0 - rf) * t;
        gf += (255.0 - gf) * t;
        bf += (255.0 - bf) * t;
    }
    if let Some(s) = shade {
        rf *= s;
        gf *= s;
        bf *= s;
    }
    *hex = format!(
        "#{:02X}{:02X}{:02X}",
        rf.round().clamp(0.0, 255.0) as u8,
        gf.round().clamp(0.0, 255.0) as u8,
        bf.round().clamp(0.0, 255.0) as u8
    );
}

fn scheme_hex(scheme: &BTreeMap<String, String>, name: &str) -> Option<String> {
    let slot = match name {
        "bg1" => "lt1",
        "tx1" => "dk1",
        "bg2" => "lt2",
        "tx2" => "dk2",
        other => other,
    };
    let raw = scheme.get(slot).or_else(|| scheme.get(name))?;
    hex6(raw.trim().trim_start_matches('#'))
}

fn hex6(val: &str) -> Option<String> {
    if val.len() == 6 && val.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(format!("#{}", val.to_ascii_uppercase()))
    } else {
        None
    }
}

fn text_of(el: &Element) -> String {
    let mut parts = Vec::new();
    collect_text_paras(el, &mut parts);
    parts
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn collect_text_paras(el: &Element, parts: &mut Vec<String>) {
    let mut stack: Vec<&Element> = vec![el];
    while let Some(node) = stack.pop() {
        if node.local == "p" {
            let mut para = Vec::new();
            collect_text(node, &mut para);
            let joined = para.join("").trim().to_owned();
            if !joined.is_empty() {
                parts.push(joined);
            }
            continue;
        }
        for child in node.children.iter().rev() {
            if let Node::Element(child) = child {
                stack.push(child);
            }
        }
    }
    if parts.is_empty() {
        collect_text(el, parts);
    }
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
        assert!(svg.contains("Node One"), "{svg}");
        assert!(svg.contains("fill=\"#FFFFFF\""), "{svg}");
        assert!(svg.contains("#156082"), "{svg}");
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

    // Catches: orgchart fan connectors (textless custGeom + noFill + ln) dropped from the SVG.
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
        let svg = smartart_svg(Some(xml), 120.0, 200.0);
        assert!(svg.contains("<path "), "{svg}");
        assert!(svg.contains("fill=\"none\""), "{svg}");
    }

    // Catches: missing drawing XML painting an empty frame instead of a label.
    #[test]
    fn missing_drawing_is_labelled_smartart() {
        let svg = smartart_svg(None, 200.0, 80.0);
        assert!(svg.contains(">SmartArt<"), "{svg}");
        assert!(svg.contains("width=\"200\""), "{svg}");
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
