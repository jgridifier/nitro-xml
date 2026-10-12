// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/smartart-svg.ts
// (cached-drawing assembly only); changes listed in PROVENANCE.md.
//! S2: shared SmartArt cached-drawing painter.
//!
//! M1 bootstrap: when a diagram drawing part is available, emit an SVG of
//! `dsp:sp` boxes (rectangles + text). Otherwise emit a labelled SmartArt
//! frame at the caller-supplied box. Full layout fallback and 3D (P11/P12)
//! stay follow-ups. Used by pptx; xlsx/docx can call the same entry.

use std::collections::BTreeMap;

use nitro_xml_opc::xml::{parse_xml_str, Element, Node};

/// SVG markup for a SmartArt frame. `drawing_xml` is the diagram drawing part
/// (`dsp:drawing`) when the package resolved one.
pub fn smartart_svg(drawing_xml: Option<&str>, width: f64, height: f64) -> String {
    smartart_svg_with_scheme(drawing_xml, width, height, None)
}

/// Like [`smartart_svg`], resolving `a:schemeClr` through `scheme` (`accent1` → `#156082`).
///
/// Textless `dsp:sp` / `dsp:pic` frames are dropped. A drawing that is only those
/// frames stays the labelled stub, so empty placeholders do not replace it.
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
            shapes.retain(|shape| !shape.text.is_empty());
            if !shapes.is_empty() {
                return shapes_svg(&shapes, w, h);
            }
        }
    }
    labelled_svg(w, h)
}

struct SpBox {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: String,
    text: String,
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
    let stroke = (vw.max(vh) / 200.0).max(1.0);
    let mut body = String::new();
    for shape in shapes {
        let font = fit_font(shape.h, stroke);
        let tx = shape.x + shape.w / 2.0;
        let ty = shape.y + shape.h / 2.0;
        body.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" stroke=\"#666666\" stroke-width=\"{stroke}\"/>",
            num(shape.x),
            num(shape.y),
            num(shape.w.max(1.0)),
            num(shape.h.max(1.0)),
            escape(&shape.fill),
        ));
        if !shape.text.is_empty() {
            body.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" dominant-baseline=\"central\" font-size=\"{}\" fill=\"#222222\">{}</text>",
                num(tx),
                num(ty),
                num(font),
                escape(&shape.text),
            ));
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
    if el.local == "sp" || el.local == "pic" {
        if let Some(box_) = sp_box(el, scheme) {
            out.push(box_);
        }
        return;
    }
    for child in &el.children {
        if let Node::Element(child) = child {
            walk_shapes(child, scheme, out);
        }
    }
}

fn sp_box(sp: &Element, scheme: Option<&BTreeMap<String, String>>) -> Option<SpBox> {
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
    let fill = solid_hex(sp_pr, scheme).unwrap_or_else(|| "#D6DCE4".to_owned());
    let text = text_of(sp);
    Some(SpBox {
        x,
        y,
        w,
        h,
        fill,
        text,
    })
}

fn solid_hex(sp_pr: &Element, scheme: Option<&BTreeMap<String, String>>) -> Option<String> {
    let fill = child_any(sp_pr, "solidFill")?;
    if let Some(srgb) = child_any(fill, "srgbClr") {
        return hex6(attr_str(srgb, "val")?);
    }
    let scheme_el = child_any(fill, "schemeClr")?;
    let name = attr_str(scheme_el, "val")?;
    scheme_hex(scheme?, name)
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
    collect_text(el, &mut parts);
    parts.join("").trim().to_owned()
}

fn collect_text(el: &Element, parts: &mut Vec<String>) {
    if el.local == "t" {
        let t = el.text_content();
        if !t.is_empty() {
            parts.push(t);
        }
        return;
    }
    for child in &el.children {
        if let Node::Element(child) = child {
            collect_text(child, parts);
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

    use super::{smartart_svg, smartart_svg_with_scheme};

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

    // Catches: missing drawing XML painting an empty frame instead of a label.
    #[test]
    fn missing_drawing_is_labelled_smartart() {
        let svg = smartart_svg(None, 200.0, 80.0);
        assert!(svg.contains(">SmartArt<"), "{svg}");
        assert!(svg.contains("width=\"200\""), "{svg}");
    }
}
