// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/utils/chart-user-shapes-parser.ts
// (parseChartUserShapesDrawing, flattenChartUserShapes) and
// src/ui/src/pptx/render/chart-user-shape-overlay.ts; changes in PROVENANCE.md.
//! `c:userShapes` drawing overlay of a chart, projected to SVG primitives.

use std::collections::HashMap;
use std::fmt::Write as _;

use nitro_xml_opc::drawingml::{parse_drawing_color_in, resolve_drawing_color};
use nitro_xml_opc::xml::{parse_xml_str, Element};

use crate::inheritance::{attr, child_local, children_local};

const CDR: &str = "http://schemas.openxmlformats.org/drawingml/2006/chartDrawing";
const A: &str = nitro_xml_opc::xml::ns::A;
const EMU_PER_PIXEL: f64 = 9525.0;
const PX_PER_PT: f64 = 4.0 / 3.0;

#[derive(Clone, Default)]
struct Para {
    text: String,
    align: Option<String>,
    font_size: Option<f64>,
    bold: bool,
    color: Option<String>,
}

#[derive(Clone, Default)]
struct Visuals {
    fill: Option<String>,
    stroke: Option<String>,
    stroke_width: Option<f64>,
    rotation: f64,
    flip_h: bool,
    flip_v: bool,
    paragraphs: Vec<Para>,
    connector: bool,
}

#[derive(Clone, Copy, Default)]
struct Xfrm {
    off: (f64, f64),
    ext: (f64, f64),
}

#[derive(Clone)]
struct GroupTransform {
    ext: (f64, f64),
    ch_off: (f64, f64),
    ch_ext: (f64, f64),
    rotation: f64,
    flip_h: bool,
    flip_v: bool,
}

enum Child {
    Leaf(Xfrm, Visuals),
    Group(Xfrm, GroupTransform, Vec<Child>),
}

enum Anchor {
    Rel { from: (f64, f64), to: (f64, f64) },
    Abs { from: (f64, f64), ext: (f64, f64) },
}

enum Shape {
    Leaf(Visuals),
    Group(GroupTransform, Vec<Child>),
}

struct Parsed {
    anchor: Anchor,
    shape: Shape,
}

#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

type Scheme = HashMap<String, String>;

fn num(el: Option<&Element>, name: &str) -> f64 {
    el.and_then(|e| attr(e, name))
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(0.0)
}

fn on(v: Option<&str>) -> bool {
    matches!(v, Some("1") | Some("true"))
}

fn color(container: Option<&Element>, scheme: &Scheme) -> Option<String> {
    let c = parse_drawing_color_in(container)?;
    let r = resolve_drawing_color(&c, Some(scheme), None)?;
    Some(r.hex)
}

fn rot_flip(xfrm: Option<&Element>) -> (f64, bool, bool) {
    let Some(x) = xfrm else {
        return (0.0, false, false);
    };
    let rot = num(Some(x), "rot");
    (
        if rot != 0.0 { rot / 60000.0 } else { 0.0 },
        on(attr(x, "flipH")),
        on(attr(x, "flipV")),
    )
}

fn paragraphs(shape: &Element, scheme: &Scheme) -> Vec<Para> {
    let Some(body) = child_local(shape, "txBody", CDR).or_else(|| child_local(shape, "txBody", A))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for p in children_local(body, "p", A) {
        let runs = children_local(p, "r", A);
        let text: String = runs
            .iter()
            .map(|r| {
                child_local(r, "t", A)
                    .map(|t| t.text_content())
                    .unwrap_or_default()
            })
            .collect::<String>()
            .trim()
            .to_string();
        let first_rpr = runs.first().and_then(|r| child_local(r, "rPr", A));
        let mut para = Para {
            text,
            ..Default::default()
        };
        if let Some(a) = child_local(p, "pPr", A).and_then(|pp| attr(pp, "algn")) {
            if matches!(a, "l" | "ctr" | "r") {
                para.align = Some(a.to_string());
            }
        }
        if let Some(sz) = first_rpr
            .and_then(|r| attr(r, "sz"))
            .and_then(|v| v.parse::<f64>().ok())
        {
            para.font_size = Some(sz / 100.0);
        }
        para.bold = on(first_rpr.and_then(|r| attr(r, "b")));
        para.color = color(
            first_rpr.and_then(|r| child_local(r, "solidFill", A)),
            scheme,
        );
        if !para.text.is_empty() || para.align.is_some() || para.font_size.is_some() {
            out.push(para);
        }
    }
    out
}

fn sp_pr(shape: &Element) -> Option<&Element> {
    child_local(shape, "spPr", CDR).or_else(|| child_local(shape, "spPr", A))
}

fn visuals(shape: &Element, connector: bool, scheme: &Scheme) -> Visuals {
    let sp = sp_pr(shape);
    let mut v = Visuals {
        connector,
        ..Default::default()
    };
    // resolveShapeFill: solid, else first gradient stop, else pattern foreground.
    v.fill = sp
        .and_then(|s| child_local(s, "solidFill", A))
        .and_then(|f| color(Some(f), scheme))
        .or_else(|| {
            let grad = sp.and_then(|s| child_local(s, "gradFill", A))?;
            let stop = child_local(grad, "gsLst", A)
                .and_then(|g| children_local(g, "gs", A).first().copied())?;
            color(Some(stop), scheme)
        })
        .or_else(|| {
            let patt = sp.and_then(|s| child_local(s, "pattFill", A))?;
            color(child_local(patt, "fgClr", A), scheme)
        });
    let ln = sp.and_then(|s| child_local(s, "ln", A));
    v.stroke = ln
        .and_then(|l| child_local(l, "solidFill", A))
        .and_then(|f| color(Some(f), scheme));
    if v.stroke.is_none() && connector && ln.is_some() {
        v.stroke = Some("#000000".into());
    }
    if let Some(w) = ln
        .and_then(|l| attr(l, "w"))
        .and_then(|w| w.parse::<f64>().ok())
    {
        v.stroke_width = Some(w / 12700.0);
    }
    let (r, fh, fv) = rot_flip(sp.and_then(|s| child_local(s, "xfrm", A)));
    v.rotation = r;
    v.flip_h = fh;
    v.flip_v = fv;
    if !connector {
        v.paragraphs = paragraphs(shape, scheme);
    }
    v
}

fn off_ext(xfrm: Option<&Element>, off: &str, ext: &str) -> ((f64, f64), (f64, f64)) {
    let o = xfrm.and_then(|x| child_local(x, off, A));
    let e = xfrm.and_then(|x| child_local(x, ext, A));
    ((num(o, "x"), num(o, "y")), (num(e, "cx"), num(e, "cy")))
}

fn group_transform(group: &Element) -> GroupTransform {
    let xfrm = child_local(group, "grpSpPr", CDR)
        .or_else(|| child_local(group, "grpSpPr", A))
        .and_then(|g| child_local(g, "xfrm", A));
    let (_, ext) = off_ext(xfrm, "off", "ext");
    let (ch_off, ch_ext) = off_ext(xfrm, "chOff", "chExt");
    let (rotation, flip_h, flip_v) = rot_flip(xfrm);
    GroupTransform {
        ext,
        ch_off,
        ch_ext,
        rotation,
        flip_h,
        flip_v,
    }
}

fn child_xfrm(shape: &Element) -> Xfrm {
    let xfrm = sp_pr(shape)
        .and_then(|s| child_local(s, "xfrm", A))
        .or_else(|| child_local(shape, "xfrm", A));
    let (off, ext) = off_ext(xfrm, "off", "ext");
    Xfrm { off, ext }
}

fn group_children(group: &Element, scheme: &Scheme) -> Vec<Child> {
    let mut out = Vec::new();
    for kind in ["sp", "cxnSp", "pic"] {
        for c in children_local(group, kind, CDR) {
            out.push(Child::Leaf(
                child_xfrm(c),
                visuals(c, kind == "cxnSp", scheme),
            ));
        }
    }
    for g in children_local(group, "grpSp", CDR) {
        let t = group_transform(g);
        let xf = child_xfrm_group(g);
        out.push(Child::Group(xf, t, group_children(g, scheme)));
    }
    out
}

fn child_xfrm_group(group: &Element) -> Xfrm {
    let xfrm = child_local(group, "grpSpPr", CDR)
        .or_else(|| child_local(group, "grpSpPr", A))
        .and_then(|g| child_local(g, "xfrm", A));
    let (off, ext) = off_ext(xfrm, "off", "ext");
    Xfrm { off, ext }
}

fn marker(el: Option<&Element>) -> (f64, f64) {
    let f = |name: &str| {
        el.and_then(|e| child_local(e, name, CDR))
            .and_then(|c| c.text_content().trim().parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(0.0)
    };
    (f("x"), f("y"))
}

fn parse_anchor(anchor: &Element, base: Anchor, scheme: &Scheme) -> Option<Parsed> {
    for kind in ["sp", "cxnSp", "pic"] {
        if let Some(s) = child_local(anchor, kind, CDR) {
            return Some(Parsed {
                anchor: base,
                shape: Shape::Leaf(visuals(s, kind == "cxnSp", scheme)),
            });
        }
    }
    let g = child_local(anchor, "grpSp", CDR)?;
    Some(Parsed {
        anchor: base,
        shape: Shape::Group(group_transform(g), group_children(g, scheme)),
    })
}

fn parse_drawing(xml: &str, scheme: &Scheme) -> Vec<Parsed> {
    let Ok(doc) = parse_xml_str(xml) else {
        return Vec::new();
    };
    let root = &doc.root;
    let mut out = Vec::new();
    for a in children_local(root, "relSizeAnchor", CDR) {
        let base = Anchor::Rel {
            from: marker(child_local(a, "from", CDR)),
            to: marker(child_local(a, "to", CDR)),
        };
        out.extend(parse_anchor(a, base, scheme));
    }
    for a in children_local(root, "absSizeAnchor", CDR) {
        let e = child_local(a, "ext", CDR);
        let base = Anchor::Abs {
            from: marker(child_local(a, "from", CDR)),
            ext: (num(e, "cx"), num(e, "cy")),
        };
        out.extend(parse_anchor(a, base, scheme));
    }
    out
}

fn normalize_rotation(deg: f64) -> f64 {
    let m = deg % 360.0;
    if m < 0.0 {
        m + 360.0
    } else {
        m
    }
}

fn rigid(b: Rect, rot: f64, flip_h: bool, flip_v: bool, aspect: (f64, f64)) -> Rect {
    let (mut x, mut y) = (b.x, b.y);
    if flip_h {
        x = 1.0 - x - b.w;
    }
    if flip_v {
        y = 1.0 - y - b.h;
    }
    let n = normalize_rotation(rot);
    if n == 0.0 {
        return Rect { x, y, ..b };
    }
    let aw = if aspect.0 > 0.0 { aspect.0 } else { 1.0 };
    let ah = if aspect.1 > 0.0 { aspect.1 } else { 1.0 };
    let rad = n * std::f64::consts::PI / 180.0;
    let (cos, sin) = (rad.cos(), rad.sin());
    let dx = (x + b.w / 2.0 - 0.5) * aw;
    let dy = (y + b.h / 2.0 - 0.5) * ah;
    let (rx, ry) = (dx * cos - dy * sin, dx * sin + dy * cos);
    Rect {
        x: 0.5 + rx / aw - b.w / 2.0,
        y: 0.5 + ry / ah - b.h / 2.0,
        w: b.w,
        h: b.h,
    }
}

fn fraction(off: (f64, f64), ext: (f64, f64), ch_off: (f64, f64), ch_ext: (f64, f64)) -> Rect {
    let d = |n: f64, den: f64| if den != 0.0 { n / den } else { 0.0 };
    Rect {
        x: d(off.0 - ch_off.0, ch_ext.0),
        y: d(off.1 - ch_off.1, ch_ext.1),
        w: d(ext.0, ch_ext.0),
        h: d(ext.1, ch_ext.1),
    }
}

struct Leaf {
    b: Rect,
    rotation: f64,
    flip_h: bool,
    flip_v: bool,
    v: Visuals,
}

fn resolve_leaves(children: &[Child], ch_off: (f64, f64), ch_ext: (f64, f64)) -> Vec<Leaf> {
    let mut out = Vec::new();
    for child in children {
        match child {
            Child::Leaf(x, v) => out.push(Leaf {
                b: fraction(x.off, x.ext, ch_off, ch_ext),
                rotation: normalize_rotation(v.rotation),
                flip_h: v.flip_h,
                flip_v: v.flip_v,
                v: v.clone(),
            }),
            Child::Group(x, t, kids) => {
                let frac = fraction(x.off, x.ext, ch_off, ch_ext);
                for e in resolve_leaves(kids, t.ch_off, t.ch_ext) {
                    let s = rigid(e.b, t.rotation, t.flip_h, t.flip_v, t.ext);
                    out.push(Leaf {
                        b: Rect {
                            x: frac.x + s.x * frac.w,
                            y: frac.y + s.y * frac.h,
                            w: s.w * frac.w,
                            h: s.h * frac.h,
                        },
                        rotation: normalize_rotation(e.rotation + t.rotation),
                        flip_h: e.flip_h != t.flip_h,
                        flip_v: e.flip_v != t.flip_v,
                        v: e.v,
                    });
                }
            }
        }
    }
    out
}

/// One leaf in the chart's fractional space (`from`/`to`) or absolute EMU extent.
struct Placed {
    from: (f64, f64),
    to: Option<(f64, f64)>,
    ext: (f64, f64),
    rel: bool,
    v: Visuals,
}

fn flatten(parsed: Vec<Parsed>, chart_w_emu: f64, chart_h_emu: f64) -> Vec<Placed> {
    let mut out = Vec::new();
    for p in parsed {
        match p.shape {
            Shape::Leaf(v) => match p.anchor {
                Anchor::Rel { from, to } => out.push(Placed {
                    from,
                    to: Some(to),
                    ext: (0.0, 0.0),
                    rel: true,
                    v,
                }),
                Anchor::Abs { from, ext } => out.push(Placed {
                    from,
                    to: None,
                    ext,
                    rel: false,
                    v,
                }),
            },
            Shape::Group(t, kids) => {
                let raw = resolve_leaves(&kids, t.ch_off, t.ch_ext);
                let aspect = match &p.anchor {
                    Anchor::Abs { ext, .. } => *ext,
                    Anchor::Rel { from, to } => {
                        ((to.0 - from.0) * chart_w_emu, (to.1 - from.1) * chart_h_emu)
                    }
                };
                for e in raw {
                    let b = rigid(e.b, t.rotation, t.flip_h, t.flip_v, aspect);
                    let mut v = e.v;
                    v.rotation = normalize_rotation(e.rotation + t.rotation);
                    v.flip_h = e.flip_h != t.flip_h;
                    v.flip_v = e.flip_v != t.flip_v;
                    match &p.anchor {
                        Anchor::Rel { from, to } => {
                            let (w, h) = (to.0 - from.0, to.1 - from.1);
                            let f = (from.0 + b.x * w, from.1 + b.y * h);
                            out.push(Placed {
                                from: f,
                                to: Some((f.0 + b.w * w, f.1 + b.h * h)),
                                ext: (0.0, 0.0),
                                rel: true,
                                v,
                            });
                        }
                        Anchor::Abs { from, ext } => {
                            let off = (b.x * ext.0, b.y * ext.1);
                            out.push(Placed {
                                from: (from.0 + off.0 / chart_w_emu, from.1 + off.1 / chart_h_emu),
                                to: None,
                                ext: (b.w * ext.0, b.h * ext.1),
                                rel: false,
                                v,
                            });
                        }
                    }
                }
            }
        }
    }
    out
}

fn n(v: f64) -> String {
    format!("{v}")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn overlay_transform(v: &Visuals, b: Rect) -> Option<String> {
    if v.rotation == 0.0 && !v.flip_h && !v.flip_v {
        return None;
    }
    let (cx, cy) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
    let mut parts = Vec::new();
    if v.rotation != 0.0 {
        parts.push(format!("rotate({} {} {})", n(v.rotation), n(cx), n(cy)));
    }
    if v.flip_h || v.flip_v {
        parts.push(format!(
            "translate({} {}) scale({} {}) translate({} {})",
            n(cx),
            n(cy),
            if v.flip_h { -1 } else { 1 },
            if v.flip_v { -1 } else { 1 },
            n(-cx),
            n(-cy)
        ));
    }
    Some(parts.join(" "))
}

/// SVG markup for the overlay, appended after the chart's data marks.
pub(crate) fn overlay_svg(xml: &str, svg_w: f64, svg_h: f64, scheme: &Scheme) -> String {
    let leaves = flatten(
        parse_drawing(xml, scheme),
        svg_w * EMU_PER_PIXEL,
        svg_h * EMU_PER_PIXEL,
    );
    let mut o = String::new();
    for p in leaves {
        let (x, y) = (p.from.0 * svg_w, p.from.1 * svg_h);
        let b = match (p.rel, p.to) {
            (true, Some(to)) => Rect {
                x,
                y,
                w: ((to.0 - p.from.0) * svg_w).max(0.0),
                h: ((to.1 - p.from.1) * svg_h).max(0.0),
            },
            _ => Rect {
                x,
                y,
                w: p.ext.0 / EMU_PER_PIXEL,
                h: p.ext.1 / EMU_PER_PIXEL,
            },
        };
        let v = &p.v;
        let transform = overlay_transform(v, b);
        let tattr = transform
            .as_ref()
            .map(|t| format!(" transform=\"{t}\""))
            .unwrap_or_default();
        if v.connector {
            let _ = write!(
                o,
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\" opacity=\"1\"{tattr}></line>",
                n(b.x),
                n(b.y),
                n(b.x + b.w),
                n(b.y + b.h),
                v.stroke.as_deref().unwrap_or("#000000"),
                n(v.stroke_width.unwrap_or(1.0))
            );
            continue;
        }
        if (v.fill.is_some() || v.stroke.is_some()) && b.w > 0.0 && b.h > 0.0 {
            let _ = write!(
                o,
                "<polygon points=\"{},{} {},{} {},{} {},{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\" opacity=\"1\"{tattr}></polygon>",
                n(b.x),
                n(b.y),
                n(b.x + b.w),
                n(b.y),
                n(b.x + b.w),
                n(b.y + b.h),
                n(b.x),
                n(b.y + b.h),
                v.fill.as_deref().unwrap_or("none"),
                v.stroke.as_deref().unwrap_or("none"),
                n(v.stroke_width.unwrap_or(if v.stroke.is_some() { 1.0 } else { 0.0 }))
            );
        }
        text_primitives(&mut o, v, b);
    }
    o
}

fn text_primitives(o: &mut String, v: &Visuals, b: Rect) {
    if v.paragraphs.is_empty() {
        return;
    }
    let font_px = |p: &Para| p.font_size.map_or(10.0 * PX_PER_PT, |s| s * PX_PER_PT);
    let line_h = v
        .paragraphs
        .iter()
        .map(|p| font_px(p) * 1.2)
        .fold(12.0, f64::max);
    let total = v.paragraphs.len() as f64 * line_h;
    let mut cursor = b.y + ((b.h - total) / 2.0).max(0.0) + line_h * 0.75;
    let (cx, cy) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
    for p in &v.paragraphs {
        if p.text.is_empty() {
            cursor += line_h;
            continue;
        }
        let align = p.align.as_deref().unwrap_or("ctr");
        let (anchor, tx) = match align {
            "l" => ("start", b.x + 2.0),
            "r" => ("end", b.x + b.w - 2.0),
            _ => ("middle", b.x + b.w / 2.0),
        };
        let rot = if v.rotation != 0.0 {
            format!(
                " transform=\"rotate({} {} {})\"",
                n(v.rotation),
                n(cx),
                n(cy)
            )
        } else {
            String::new()
        };
        let _ = write!(
            o,
            "<text x=\"{}\" y=\"{}\" text-anchor=\"{anchor}\" font-size=\"{}\" fill=\"{}\" font-weight=\"{}\" font-style=\"normal\" opacity=\"1\"{rot}>{}</text>",
            n(tx),
            n(cursor),
            n(font_px(p)),
            p.color.as_deref().unwrap_or("#1e293b"),
            if p.bold { "bold" } else { "normal" },
            esc(&p.text)
        );
        cursor += line_h;
    }
}
