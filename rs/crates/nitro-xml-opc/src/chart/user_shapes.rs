// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/utils/chart-user-shapes-parser.ts
// and src/core/pptx/core/types/chart-user-shapes.ts; changes listed in PROVENANCE.md.

use std::collections::HashMap;

use crate::drawingml::{
    parse_drawing_fill, parse_drawing_line, resolve_drawing_color, DrawingColorTheme, DrawingFill,
    HslRounding, ResolveOptions, TransformOrder,
};
use crate::xml::{parse_xml_str, Element, Node};

#[derive(Debug, Clone, PartialEq)]
pub struct ChartUserShapeParagraph {
    pub text: String,
    pub font_size_pt: Option<f64>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub color: Option<String>,
    pub align: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChartUserShapeGroupTransform {
    pub off: EmuPoint,
    pub ext: EmuSize,
    pub ch_off: EmuPoint,
    pub ch_ext: EmuSize,
    pub rotation: Option<f64>,
    pub flip_h: Option<bool>,
    pub flip_v: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmuPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmuSize {
    pub cx: f64,
    pub cy: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChartUserShapeGroupChild {
    pub kind: String,
    pub off: EmuPoint,
    pub ext: EmuSize,
    pub prst: Option<String>,
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: Option<f64>,
    pub paragraphs: Option<Vec<ChartUserShapeParagraph>>,
    pub alt_text: Option<String>,
    pub rotation: Option<f64>,
    pub flip_h: Option<bool>,
    pub flip_v: Option<bool>,
    pub transform: Option<ChartUserShapeGroupTransform>,
    pub children: Option<Vec<ChartUserShapeGroupChild>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChartUserShape {
    pub kind: String,
    pub anchor: String,
    pub from: FracPoint,
    pub to: Option<FracPoint>,
    pub ext: Option<EmuSize>,
    pub prst: Option<String>,
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: Option<f64>,
    pub paragraphs: Option<Vec<ChartUserShapeParagraph>>,
    pub alt_text: Option<String>,
    pub rotation: Option<f64>,
    pub flip_h: Option<bool>,
    pub flip_v: Option<bool>,
    pub transform: Option<ChartUserShapeGroupTransform>,
    pub children: Option<Vec<ChartUserShapeGroupChild>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FracPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlattenedChartUserShape {
    pub kind: String,
    pub anchor: String,
    pub from: FracPoint,
    pub to: Option<FracPoint>,
    pub ext: Option<EmuSize>,
    pub abs_group_offset_emu: Option<EmuPoint>,
    pub prst: Option<String>,
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: Option<f64>,
    pub paragraphs: Option<Vec<ChartUserShapeParagraph>>,
    pub alt_text: Option<String>,
    pub rotation: Option<f64>,
    pub flip_h: Option<bool>,
    pub flip_v: Option<bool>,
}

pub struct ChartUserShapesChartBox {
    pub width: f64,
    pub height: f64,
}

const CHART_USER_SHAPES_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chartUserShapes";

pub fn is_chart_user_shapes_rel(type_uri: &str) -> bool {
    type_uri.trim() == CHART_USER_SHAPES_REL
}

fn child_local<'a>(parent: &'a Element, local: &str) -> Option<&'a Element> {
    parent.children.iter().find_map(|n| match n {
        Node::Element(e) if e.local == local => Some(e),
        _ => None,
    })
}

fn children_local<'a>(parent: &'a Element, local: &str) -> Vec<&'a Element> {
    parent
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(e) if e.local == local => Some(e),
            _ => None,
        })
        .collect()
}

fn attr_val<'a>(el: &'a Element, name: &str) -> Option<&'a str> {
    el.attrs
        .iter()
        .find(|a| a.local == name)
        .map(|a| a.value.as_str())
}

fn parse_num(s: Option<&str>) -> f64 {
    s.and_then(|v| v.parse::<f64>().ok())
        .filter(|n: &f64| n.is_finite())
        .unwrap_or(0.0)
}

fn marker_fraction(marker: Option<&Element>, axis: &str) -> f64 {
    let el = marker.and_then(|m| child_local(m, axis));
    let text = el.map(|e| e.text_content()).unwrap_or_default();
    parse_num(Some(text.trim()))
}

fn color_hex(
    fill: Option<DrawingFill>,
    scheme: Option<&HashMap<String, String>>,
) -> Option<String> {
    fill.and_then(|f| match f {
        DrawingFill::Solid { color } => resolve_drawing_color(
            &color,
            scheme.map(|s| s as &dyn DrawingColorTheme),
            Some(ResolveOptions {
                transform_order: TransformOrder::Document,
                hsl_rounding: HslRounding::HalfDown,
            }),
        )
        .filter(|r| r.alpha == 1.0)
        .map(|r| r.hex),
        _ => None,
    })
}

fn resolve_shape_fill(
    sp_pr: Option<&Element>,
    scheme: Option<&HashMap<String, String>>,
) -> Option<String> {
    if let Some(solid) = color_hex(parse_drawing_fill(sp_pr), scheme) {
        return Some(solid);
    }
    let grad = sp_pr.and_then(|p| child_local(p, "gradFill"));
    if let Some(gs_lst) = grad.and_then(|g| child_local(g, "gsLst")) {
        let first = children_local(gs_lst, "gs").first().copied();
        if let Some(hex) = color_hex(parse_drawing_fill(first), scheme) {
            return Some(hex);
        }
    }
    let patt = sp_pr.and_then(|p| child_local(p, "pattFill"));
    if let Some(fg) = patt.and_then(|p| child_local(p, "fgClr")) {
        if let Some(fill) = parse_drawing_fill(Some(fg)) {
            return color_hex(Some(fill), scheme);
        }
    }
    None
}

fn parse_xfrm_rot_flip(xfrm: Option<&Element>) -> (Option<f64>, Option<bool>, Option<bool>) {
    let Some(xfrm) = xfrm else {
        return (None, None, None);
    };
    let rot = parse_num(attr_val(xfrm, "rot"));
    let rotation = (rot != 0.0).then_some(rot / 60000.0);
    let flip_h =
        attr_val(xfrm, "flipH").is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    let flip_v =
        attr_val(xfrm, "flipV").is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    (rotation, flip_h.then_some(true), flip_v.then_some(true))
}

fn parse_off_ext(off_node: Option<&Element>, ext_node: Option<&Element>) -> (EmuPoint, EmuSize) {
    (
        EmuPoint {
            x: parse_num(off_node.and_then(|o| attr_val(o, "x"))),
            y: parse_num(off_node.and_then(|o| attr_val(o, "y"))),
        },
        EmuSize {
            cx: parse_num(ext_node.and_then(|e| attr_val(e, "cx"))),
            cy: parse_num(ext_node.and_then(|e| attr_val(e, "cy"))),
        },
    )
}

fn parse_shape_paragraphs(
    shape: &Element,
    scheme: Option<&HashMap<String, String>>,
) -> Option<Vec<ChartUserShapeParagraph>> {
    let tx_body = child_local(shape, "txBody")?;
    let mut paragraphs = Vec::new();
    for p in children_local(tx_body, "p") {
        let runs = children_local(p, "r");
        let text: String = runs
            .iter()
            .map(|r| {
                child_local(r, "t")
                    .map(|t| t.text_content())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join("")
            .trim()
            .to_string();
        let p_pr = child_local(p, "pPr");
        let first_rpr = runs.first().and_then(|r| child_local(r, "rPr"));
        let align = p_pr
            .and_then(|pp| attr_val(pp, "algn"))
            .filter(|a| *a == "l" || *a == "ctr" || *a == "r")
            .map(str::to_owned);
        let sz = parse_num(first_rpr.and_then(|r| attr_val(r, "sz")));
        let font_size_pt = (sz > 0.0).then_some(sz / 100.0);
        let bold = first_rpr
            .and_then(|r| attr_val(r, "b"))
            .is_some_and(|b| b == "1" || b.eq_ignore_ascii_case("true"));
        let italic = first_rpr
            .and_then(|r| attr_val(r, "i"))
            .is_some_and(|i| i == "1" || i.eq_ignore_ascii_case("true"));
        let color = first_rpr
            .and_then(|r| child_local(r, "solidFill"))
            .and_then(|sf| color_hex(parse_drawing_fill(Some(sf)), scheme));
        if !text.is_empty() || align.is_some() || font_size_pt.is_some() {
            paragraphs.push(ChartUserShapeParagraph {
                text,
                font_size_pt,
                bold: bold.then_some(true),
                italic: italic.then_some(true),
                color,
                align,
            });
        }
    }
    (!paragraphs.is_empty()).then_some(paragraphs)
}

#[allow(clippy::type_complexity)]
fn parse_shape_visuals(
    shape: &Element,
    kind: &str,
    scheme: Option<&HashMap<String, String>>,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<f64>,
    Option<f64>,
    Option<bool>,
    Option<bool>,
) {
    let sp_pr = child_local(shape, "spPr");
    let prst = sp_pr
        .and_then(|p| child_local(p, "prstGeom"))
        .and_then(|g| attr_val(g, "prst"))
        .map(str::to_owned);
    let fill = resolve_shape_fill(sp_pr, scheme);
    let ln = sp_pr.and_then(|p| child_local(p, "ln"));
    let stroke = parse_drawing_line(ln)
        .and_then(|l| color_hex(l.fill, scheme))
        .or_else(|| {
            if kind == "cxnSp" && ln.is_some() {
                Some("#000000".to_string())
            } else {
                None
            }
        });
    let stroke_width = ln
        .and_then(|l| attr_val(l, "w"))
        .and_then(|w| w.parse::<f64>().ok())
        .filter(|n| n.is_finite())
        .map(|w| w / 12700.0);
    let (rotation, flip_h, flip_v) =
        parse_xfrm_rot_flip(sp_pr.and_then(|p| child_local(p, "xfrm")));
    (prst, fill, stroke, stroke_width, rotation, flip_h, flip_v)
}

fn build_user_shape(
    shape: &Element,
    kind: &str,
    anchor: &str,
    from: FracPoint,
    to: Option<FracPoint>,
    ext: Option<EmuSize>,
    scheme: Option<&HashMap<String, String>>,
) -> ChartUserShape {
    let (prst, fill, stroke, stroke_width, rotation, flip_h, flip_v) =
        parse_shape_visuals(shape, kind, scheme);
    let paragraphs = if kind == "pic" {
        None
    } else {
        parse_shape_paragraphs(shape, scheme)
    };
    let alt_text = if kind == "pic" {
        child_local(shape, "nvPicPr")
            .and_then(|n| child_local(n, "cNvPr"))
            .and_then(|c| attr_val(c, "descr"))
            .filter(|d| !d.is_empty())
            .map(str::to_owned)
    } else {
        None
    };
    ChartUserShape {
        kind: kind.to_string(),
        anchor: anchor.to_string(),
        from,
        to,
        ext,
        prst,
        fill,
        stroke,
        stroke_width,
        paragraphs,
        alt_text,
        rotation,
        flip_h,
        flip_v,
        transform: None,
        children: None,
    }
}

fn parse_group_transform(group: &Element) -> ChartUserShapeGroupTransform {
    let grp_sp_pr = child_local(group, "grpSpPr");
    let xfrm = grp_sp_pr.and_then(|g| child_local(g, "xfrm"));
    let (off, ext) = parse_off_ext(
        xfrm.and_then(|x| child_local(x, "off")),
        xfrm.and_then(|x| child_local(x, "ext")),
    );
    let (ch_off, ch_ext) = parse_off_ext(
        xfrm.and_then(|x| child_local(x, "chOff")),
        xfrm.and_then(|x| child_local(x, "chExt")),
    );
    let (rotation, flip_h, flip_v) = parse_xfrm_rot_flip(xfrm);
    ChartUserShapeGroupTransform {
        off,
        ext,
        ch_off,
        ch_ext,
        rotation,
        flip_h,
        flip_v,
    }
}

fn parse_child_off_ext(
    shape: &Element,
) -> (EmuPoint, EmuSize, Option<f64>, Option<bool>, Option<bool>) {
    let sp_pr = child_local(shape, "spPr");
    let xfrm = sp_pr
        .and_then(|p| child_local(p, "xfrm"))
        .or_else(|| child_local(shape, "xfrm"));
    let (off, ext) = parse_off_ext(
        xfrm.and_then(|x| child_local(x, "off")),
        xfrm.and_then(|x| child_local(x, "ext")),
    );
    let (rotation, flip_h, flip_v) = parse_xfrm_rot_flip(xfrm);
    (off, ext, rotation, flip_h, flip_v)
}

fn build_group_child(
    shape: &Element,
    kind: &str,
    scheme: Option<&HashMap<String, String>>,
) -> ChartUserShapeGroupChild {
    let (prst, fill, stroke, stroke_width, rotation, flip_h, flip_v) =
        parse_shape_visuals(shape, kind, scheme);
    let paragraphs = if kind == "pic" {
        None
    } else {
        parse_shape_paragraphs(shape, scheme)
    };
    let alt_text = if kind == "pic" {
        child_local(shape, "nvPicPr")
            .and_then(|n| child_local(n, "cNvPr"))
            .and_then(|c| attr_val(c, "descr"))
            .filter(|d| !d.is_empty())
            .map(str::to_owned)
    } else {
        None
    };
    let (off, ext, child_rot, child_flip_h, child_flip_v) = parse_child_off_ext(shape);
    ChartUserShapeGroupChild {
        kind: kind.to_string(),
        off,
        ext,
        prst,
        fill,
        stroke,
        stroke_width,
        paragraphs,
        alt_text,
        rotation: child_rot.or(rotation),
        flip_h: child_flip_h.or(flip_h),
        flip_v: child_flip_v.or(flip_v),
        transform: None,
        children: None,
    }
}

fn parse_group_children(
    group: &Element,
    scheme: Option<&HashMap<String, String>>,
) -> Vec<ChartUserShapeGroupChild> {
    let mut children = Vec::new();
    for kind in ["sp", "cxnSp", "pic"] {
        for child in children_local(group, kind) {
            children.push(build_group_child(child, kind, scheme));
        }
    }
    for nested in children_local(group, "grpSp") {
        let transform = parse_group_transform(nested);
        children.push(ChartUserShapeGroupChild {
            kind: "grpSp".to_string(),
            off: transform.off.clone(),
            ext: transform.ext.clone(),
            prst: None,
            fill: None,
            stroke: None,
            stroke_width: None,
            paragraphs: None,
            alt_text: None,
            rotation: transform.rotation,
            flip_h: transform.flip_h,
            flip_v: transform.flip_v,
            transform: Some(transform),
            children: Some(parse_group_children(nested, scheme)),
        });
    }
    for gf in children_local(group, "graphicFrame") {
        let (off, ext, rotation, flip_h, flip_v) = parse_child_off_ext(gf);
        children.push(ChartUserShapeGroupChild {
            kind: "graphicFrame".to_string(),
            off,
            ext,
            prst: None,
            fill: None,
            stroke: None,
            stroke_width: None,
            paragraphs: None,
            alt_text: None,
            rotation,
            flip_h,
            flip_v,
            transform: None,
            children: None,
        });
    }
    children
}

fn parse_anchor_shape(
    anchor: &Element,
    anchor_kind: &str,
    from: FracPoint,
    to: Option<FracPoint>,
    ext: Option<EmuSize>,
    scheme: Option<&HashMap<String, String>>,
) -> Vec<ChartUserShape> {
    for kind in ["sp", "cxnSp", "pic"] {
        if let Some(shape) = child_local(anchor, kind) {
            return vec![build_user_shape(
                shape,
                kind,
                anchor_kind,
                from,
                to,
                ext,
                scheme,
            )];
        }
    }
    if let Some(group) = child_local(anchor, "grpSp") {
        let transform = parse_group_transform(group);
        return vec![ChartUserShape {
            kind: "grpSp".to_string(),
            anchor: anchor_kind.to_string(),
            from,
            to,
            ext,
            prst: None,
            fill: None,
            stroke: None,
            stroke_width: None,
            paragraphs: None,
            alt_text: None,
            rotation: transform.rotation,
            flip_h: transform.flip_h,
            flip_v: transform.flip_v,
            transform: Some(transform),
            children: Some(parse_group_children(group, scheme)),
        }];
    }
    if let Some(gf) = child_local(anchor, "graphicFrame") {
        let (rotation, flip_h, flip_v) = parse_xfrm_rot_flip(child_local(gf, "xfrm"));
        return vec![ChartUserShape {
            kind: "graphicFrame".to_string(),
            anchor: anchor_kind.to_string(),
            from,
            to,
            ext,
            prst: None,
            fill: None,
            stroke: None,
            stroke_width: None,
            paragraphs: None,
            alt_text: None,
            rotation,
            flip_h,
            flip_v,
            transform: None,
            children: None,
        }];
    }
    Vec::new()
}

/// Parse a chart user-shapes drawing part (`c:userShapes` root).
pub fn parse_chart_user_shapes_drawing_xml(
    xml: &str,
    scheme: Option<&HashMap<String, String>>,
) -> Option<Vec<ChartUserShape>> {
    let doc = parse_xml_str(xml).ok()?;
    let root = child_local(&doc.root, "userShapes").unwrap_or(&doc.root);
    let mut shapes = Vec::new();
    for anchor in children_local(root, "relSizeAnchor") {
        let from_el = child_local(anchor, "from");
        let to_el = child_local(anchor, "to");
        let from = FracPoint {
            x: marker_fraction(from_el, "x"),
            y: marker_fraction(from_el, "y"),
        };
        let to = FracPoint {
            x: marker_fraction(to_el, "x"),
            y: marker_fraction(to_el, "y"),
        };
        shapes.extend(parse_anchor_shape(
            anchor,
            "rel",
            from,
            Some(to),
            None,
            scheme,
        ));
    }
    for anchor in children_local(root, "absSizeAnchor") {
        let from_el = child_local(anchor, "from");
        let ext_el = child_local(anchor, "ext");
        let from = FracPoint {
            x: marker_fraction(from_el, "x"),
            y: marker_fraction(from_el, "y"),
        };
        let (_, ext) = parse_off_ext(None, ext_el);
        shapes.extend(parse_anchor_shape(
            anchor,
            "abs",
            from,
            None,
            Some(ext),
            scheme,
        ));
    }
    (!shapes.is_empty()).then_some(shapes)
}

fn child_fraction(
    child_off: &EmuPoint,
    child_ext: &EmuSize,
    ch_off: &EmuPoint,
    ch_ext: &EmuSize,
) -> (f64, f64, f64, f64) {
    let safe_div = |num: f64, den: f64| if den != 0.0 { num / den } else { 0.0 };
    (
        safe_div(child_off.x - ch_off.x, ch_ext.cx),
        safe_div(child_off.y - ch_off.y, ch_ext.cy),
        safe_div(child_ext.cx, ch_ext.cx),
        safe_div(child_ext.cy, ch_ext.cy),
    )
}

fn normalize_rotation_deg(deg: f64) -> f64 {
    let mut mod_ = deg % 360.0;
    if mod_ < 0.0 {
        mod_ += 360.0;
    }
    mod_
}

fn apply_group_rigid_transform(
    box_: (f64, f64, f64, f64),
    rotation_deg: f64,
    flip_h: bool,
    flip_v: bool,
    aspect_w: f64,
    aspect_h: f64,
) -> (f64, f64, f64, f64) {
    let (mut x, mut y, w, h) = box_;
    if flip_h {
        x = 1.0 - x - w;
    }
    if flip_v {
        y = 1.0 - y - h;
    }
    let normalized = normalize_rotation_deg(rotation_deg);
    if normalized == 0.0 {
        return (x, y, w, h);
    }
    let aspect_w = if aspect_w > 0.0 { aspect_w } else { 1.0 };
    let aspect_h = if aspect_h > 0.0 { aspect_h } else { 1.0 };
    let rad = normalized.to_radians();
    let (sin, cos) = rad.sin_cos();
    let dx = (x + w / 2.0 - 0.5) * aspect_w;
    let dy = (y + h / 2.0 - 0.5) * aspect_h;
    let rx = dx * cos - dy * sin;
    let ry = dx * sin + dy * cos;
    let cx = 0.5 + rx / aspect_w;
    let cy = 0.5 + ry / aspect_h;
    (cx - w / 2.0, cy - h / 2.0, w, h)
}

struct LeafPlacement {
    box_: (f64, f64, f64, f64),
    rotation: f64,
    flip_h: bool,
    flip_v: bool,
    child: ChartUserShapeGroupChild,
}

fn resolve_group_leaves(
    children: &[ChartUserShapeGroupChild],
    ch_off: &EmuPoint,
    ch_ext: &EmuSize,
) -> Vec<LeafPlacement> {
    let mut out = Vec::new();
    for child in children {
        let (fx, fy, fw, fh) = child_fraction(&child.off, &child.ext, ch_off, ch_ext);
        if child.kind == "grpSp" {
            let Some(transform) = child.transform.as_ref() else {
                continue;
            };
            let Some(nested_children) = child.children.as_ref() else {
                continue;
            };
            let nested =
                resolve_group_leaves(nested_children, &transform.ch_off, &transform.ch_ext);
            let group_rotation = transform.rotation.unwrap_or(0.0);
            let group_flip_h = transform.flip_h.unwrap_or(false);
            let group_flip_v = transform.flip_v.unwrap_or(false);
            let nested_aspect_w = transform.ext.cx;
            let nested_aspect_h = transform.ext.cy;
            for entry in nested {
                let self_box = apply_group_rigid_transform(
                    entry.box_,
                    group_rotation,
                    group_flip_h,
                    group_flip_v,
                    nested_aspect_w,
                    nested_aspect_h,
                );
                out.push(LeafPlacement {
                    box_: (
                        fx + self_box.0 * fw,
                        fy + self_box.1 * fh,
                        self_box.2 * fw,
                        self_box.3 * fh,
                    ),
                    rotation: normalize_rotation_deg(entry.rotation + group_rotation),
                    flip_h: entry.flip_h != group_flip_h,
                    flip_v: entry.flip_v != group_flip_v,
                    child: entry.child,
                });
            }
        } else {
            out.push(LeafPlacement {
                box_: (fx, fy, fw, fh),
                rotation: normalize_rotation_deg(child.rotation.unwrap_or(0.0)),
                flip_h: child.flip_h.unwrap_or(false),
                flip_v: child.flip_v.unwrap_or(false),
                child: child.clone(),
            });
        }
    }
    out
}

/// Flatten grouped overlay shapes into positioned leaves for rendering.
pub fn flatten_chart_user_shapes(
    shapes: Option<&[ChartUserShape]>,
    chart_box: Option<&ChartUserShapesChartBox>,
) -> Vec<FlattenedChartUserShape> {
    let Some(shapes) = shapes else {
        return Vec::new();
    };
    let mut flat = Vec::new();
    for shape in shapes {
        if shape.kind != "grpSp" {
            flat.push(FlattenedChartUserShape {
                kind: shape.kind.clone(),
                anchor: shape.anchor.clone(),
                from: shape.from.clone(),
                to: shape.to.clone(),
                ext: shape.ext.clone(),
                abs_group_offset_emu: None,
                prst: shape.prst.clone(),
                fill: shape.fill.clone(),
                stroke: shape.stroke.clone(),
                stroke_width: shape.stroke_width,
                paragraphs: shape.paragraphs.clone(),
                alt_text: shape.alt_text.clone(),
                rotation: shape.rotation,
                flip_h: shape.flip_h,
                flip_v: shape.flip_v,
            });
            continue;
        }
        let Some(transform) = shape.transform.as_ref() else {
            continue;
        };
        let Some(children) = shape.children.as_ref() else {
            continue;
        };
        let raw_leaves = resolve_group_leaves(children, &transform.ch_off, &transform.ch_ext);
        let top_rotation = transform.rotation.unwrap_or(0.0);
        let top_flip_h = transform.flip_h.unwrap_or(false);
        let top_flip_v = transform.flip_v.unwrap_or(false);
        let (top_aspect_w, top_aspect_h) = if shape.anchor == "abs" {
            (
                shape.ext.as_ref().map(|e| e.cx).unwrap_or(0.0),
                shape.ext.as_ref().map(|e| e.cy).unwrap_or(0.0),
            )
        } else if let Some(cb) = chart_box {
            let to = shape.to.as_ref().unwrap_or(&shape.from);
            (
                (to.x - shape.from.x) * cb.width,
                (to.y - shape.from.y) * cb.height,
            )
        } else {
            (1.0, 1.0)
        };
        let leaves: Vec<LeafPlacement> = raw_leaves
            .into_iter()
            .map(|entry| {
                let self_box = apply_group_rigid_transform(
                    entry.box_,
                    top_rotation,
                    top_flip_h,
                    top_flip_v,
                    top_aspect_w,
                    top_aspect_h,
                );
                LeafPlacement {
                    box_: self_box,
                    rotation: normalize_rotation_deg(entry.rotation + top_rotation),
                    flip_h: entry.flip_h != top_flip_h,
                    flip_v: entry.flip_v != top_flip_v,
                    child: entry.child,
                }
            })
            .collect();
        for lp in leaves {
            let child = &lp.child;
            let rot_flip = (
                (lp.rotation != 0.0).then_some(lp.rotation),
                lp.flip_h.then_some(true),
                lp.flip_v.then_some(true),
            );
            if shape.anchor == "rel" {
                let width = shape.to.as_ref().map(|t| t.x - shape.from.x).unwrap_or(0.0);
                let height = shape.to.as_ref().map(|t| t.y - shape.from.y).unwrap_or(0.0);
                let from = FracPoint {
                    x: shape.from.x + lp.box_.0 * width,
                    y: shape.from.y + lp.box_.1 * height,
                };
                let to = FracPoint {
                    x: from.x + lp.box_.2 * width,
                    y: from.y + lp.box_.3 * height,
                };
                flat.push(FlattenedChartUserShape {
                    kind: child.kind.clone(),
                    anchor: "rel".to_string(),
                    from,
                    to: Some(to),
                    ext: None,
                    abs_group_offset_emu: None,
                    prst: child.prst.clone(),
                    fill: child.fill.clone(),
                    stroke: child.stroke.clone(),
                    stroke_width: child.stroke_width,
                    paragraphs: child.paragraphs.clone(),
                    alt_text: child.alt_text.clone(),
                    rotation: rot_flip.0,
                    flip_h: rot_flip.1,
                    flip_v: rot_flip.2,
                });
            } else {
                let ext = shape.ext.clone().unwrap_or(EmuSize { cx: 0.0, cy: 0.0 });
                let offset_emu = EmuPoint {
                    x: lp.box_.0 * ext.cx,
                    y: lp.box_.1 * ext.cy,
                };
                let valid_chart_box = chart_box.filter(|cb| cb.width > 0.0 && cb.height > 0.0);
                let from = if let Some(cb) = valid_chart_box {
                    FracPoint {
                        x: shape.from.x + offset_emu.x / cb.width,
                        y: shape.from.y + offset_emu.y / cb.height,
                    }
                } else {
                    shape.from.clone()
                };
                flat.push(FlattenedChartUserShape {
                    kind: child.kind.clone(),
                    anchor: "abs".to_string(),
                    from,
                    to: None,
                    ext: Some(EmuSize {
                        cx: lp.box_.2 * ext.cx,
                        cy: lp.box_.3 * ext.cy,
                    }),
                    abs_group_offset_emu: valid_chart_box.is_none().then_some(offset_emu),
                    prst: child.prst.clone(),
                    fill: child.fill.clone(),
                    stroke: child.stroke.clone(),
                    stroke_width: child.stroke_width,
                    paragraphs: child.paragraphs.clone(),
                    alt_text: child.alt_text.clone(),
                    rotation: rot_flip.0,
                    flip_h: rot_flip.1,
                    flip_v: rot_flip.2,
                });
            }
        }
    }
    flat
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::parse_chart_space_xml;

    // Catches: userShapes rel id parsed from wrong chartSpace child so overlay part is never resolved.
    #[test]
    fn chart_space_user_shapes_rel_id() {
        let chart_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<c:chart/><c:userShapes r:id="rId1"/></c:chartSpace>"#;
        let parsed = parse_chart_space_xml(chart_xml);
        assert_eq!(
            parsed.chart_space.user_shapes_rel_id.as_deref(),
            Some("rId1")
        );
    }

    // Catches: grouped relSizeAnchor overlay shapes dropped at parse so Alpha/Beta never paint.
    #[test]
    fn parses_grouped_rel_size_anchor() {
        let drawing = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<c:userShapes xmlns:cdr="http://schemas.openxmlformats.org/drawingml/2006/chartDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">
<cdr:relSizeAnchor><cdr:from><cdr:x>0.1</cdr:x><cdr:y>0.6</cdr:y></cdr:from><cdr:to><cdr:x>0.9</cdr:x><cdr:y>0.95</cdr:y></cdr:to>
<cdr:grpSp><cdr:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1000000" cy="400000"/><a:chOff x="0" y="0"/><a:chExt cx="1000000" cy="400000"/></a:xfrm></cdr:grpSpPr>
<cdr:sp><cdr:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="500000" cy="400000"/></a:xfrm><a:prstGeom prst="rect"/><a:solidFill><a:srgbClr val="FFFFCC"/></a:solidFill></cdr:spPr><cdr:txBody><a:p><a:r><a:t>Alpha</a:t></a:r></a:p></cdr:txBody></cdr:sp>
</cdr:grpSp></cdr:relSizeAnchor></c:userShapes>"#;
        let shapes = parse_chart_user_shapes_drawing_xml(drawing, None).expect("shapes");
        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].kind, "grpSp");
        let flat = flatten_chart_user_shapes(
            Some(&shapes),
            Some(&ChartUserShapesChartBox {
                width: 952500.0,
                height: 535500.0,
            }),
        );
        assert!(flat
            .iter()
            .any(|s| s.paragraphs.as_ref().is_some_and(|p| p[0].text == "Alpha")));
    }

    // Catches: flatten leaves rel anchors without `to` so overlay boxes collapse to zero width/height.
    #[test]
    fn flatten_fixture_group_has_nonzero_rel_boxes() {
        let drawing_xml = std::process::Command::new("unzip")
            .args([
                "-p",
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../fixtures/upstream/pptx/a0806b09c73d_chart-user-shape-group.pptx"
                ),
                "ppt/drawings/drawing1.xml",
            ])
            .output()
            .expect("unzip")
            .stdout;
        let drawing = String::from_utf8(drawing_xml).expect("utf8");
        let shapes = parse_chart_user_shapes_drawing_xml(&drawing, None).expect("parse");
        let flat = flatten_chart_user_shapes(
            Some(&shapes),
            Some(&ChartUserShapesChartBox {
                width: 960.0 * 9525.0,
                height: 540.0 * 9525.0,
            }),
        );
        let alpha = flat
            .iter()
            .find(|s| s.paragraphs.as_ref().is_some_and(|p| p[0].text == "Alpha"))
            .expect("alpha leaf");
        assert_eq!(alpha.anchor, "rel");
        let to = alpha.to.as_ref().expect("rel to");
        assert!(to.x > alpha.from.x + 0.01, "alpha width fraction");
        assert!(to.y > alpha.from.y + 0.01, "alpha height fraction");
    }
}
