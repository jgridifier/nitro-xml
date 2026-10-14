// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/connector.ts; changes in PROVENANCE.md.

use super::connector_geometry::connector_stroke_geometry;
use super::connector_markers::{attach_markers, build_marker_def};
use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_inner_markup_stub, set_style};
use super::text::{paint_text_body, TextBodyPaint};
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    // Upstream connector.ts: zero-height `cxnSp` wrappers stay overflow-visible so
    // the absolute SVG stroke (max(dim,1)) is not clipped away.
    set_style(ctx, id, "overflow", "visible");
    set_style(ctx, id, "pointer-events", "none");
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "connector");
    if let Some(rot) = el.rotation {
        if rot.abs() > 0.01 {
            set_style(ctx, id, "transform", &format!("rotate({}deg)", rot));
        }
    }
    let stroke = el.stroke_color.as_deref().unwrap_or("#333333");
    // Upstream connector.ts defaults strokeWidth to 2 when unset.
    let stroke_width = el.stroke_width_px.unwrap_or(2.0);
    // Upstream: `Math.max(element.width/height, 1)` for the SVG viewBox.
    let w_geom = el.width.max(1.0);
    let h_geom = el.height.max(1.0);
    let w_view = w_geom.round() as i64;
    let h_view = h_geom.round() as i64;
    let geom = connector_stroke_geometry(
        el.shape_type.as_deref(),
        w_geom,
        h_geom,
        el.flip_horizontal,
        el.flip_vertical,
        stroke,
        stroke_width,
        el.stroke_dash.as_deref(),
    );
    let marker_seed = el
        .id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let start_id = el
        .connector_start_arrow
        .as_deref()
        .map(|_| format!("{}-start", marker_seed));
    let end_id = el
        .connector_end_arrow
        .as_deref()
        .map(|_| format!("{}-end", marker_seed));
    let shadow_id = el
        .box_shadow
        .as_deref()
        .and_then(parse_css_box_shadow)
        .map(|_| format!("{}-line-shadow", marker_seed));
    let mut inner = String::new();
    inner.push_str("<defs>");
    if let (Some(sid), Some(shadow)) = (
        shadow_id.as_deref(),
        el.box_shadow.as_deref().and_then(parse_css_box_shadow),
    ) {
        // Upstream connector.ts `feDropShadow` (effectRef outerShdw).
        inner.push_str(&format!(
            "<filter id=\"{sid}\" x=\"-50%\" y=\"-50%\" width=\"200%\" height=\"200%\">\
<feDropShadow dx=\"{}\" dy=\"{}\" stdDeviation=\"{}\" flood-color=\"{}\" flood-opacity=\"{}\"/></filter>",
            shadow.dx,
            shadow.dy,
            shadow.blur / 2.0,
            xml_escape_attr(&shadow.color),
            shadow.opacity
        ));
    }
    if let (Some(id), Some(ty)) = (start_id.as_deref(), el.connector_start_arrow.as_deref()) {
        inner.push_str(&build_marker_def(id, ty, stroke));
    }
    if let (Some(id), Some(ty)) = (end_id.as_deref(), el.connector_end_arrow.as_deref()) {
        inner.push_str(&build_marker_def(id, ty, stroke));
    }
    inner.push_str("</defs>");
    let mut stroke_markup = attach_markers(&geom.markup, start_id.as_deref(), end_id.as_deref());
    if let Some(sid) = shadow_id.as_deref() {
        stroke_markup = inject_filter_attr(&stroke_markup, sid);
    }
    inner.push_str(&stroke_markup);
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w_view}\" height=\"{h_view}\" viewBox=\"0 0 {w_view} {h_view}\" style=\"overflow:visible;display:block\">{}</svg>",
        inner
    );
    set_inner_markup_stub(ctx, id, &svg);
    let text = el.text.as_deref().unwrap_or("");
    if !text.trim().is_empty() {
        // Upstream `appendConnectorLabel`: vertically centre the label on the stroke.
        let label = create_child(ctx, id, "div");
        set_attr(ctx, label, "class", "pptxv-connector-label");
        set_style(ctx, label, "position", "absolute");
        set_style(ctx, label, "left", "10%");
        set_style(ctx, label, "top", "50%");
        set_style(ctx, label, "width", "80%");
        set_style(ctx, label, "height", "auto");
        set_style(ctx, label, "transform", "translateY(-50%)");
        set_style(ctx, label, "text-align", "center");
        set_style(ctx, label, "pointer-events", "none");
        paint_text_body(ctx, label, text, TextBodyPaint::from_element(el));
    }
    ctx.view_ir.push_painted("connector", el, None);
}

struct CssBoxShadow {
    dx: f64,
    dy: f64,
    blur: f64,
    color: String,
    opacity: f64,
}

/// Parse a single CSS `box-shadow` like `0px 2px 2px rgba(0, 0, 0, 0.38)`.
fn parse_css_box_shadow(css: &str) -> Option<CssBoxShadow> {
    let css = css.trim();
    if css.is_empty() {
        return None;
    }
    // rgba(...) may contain spaces; peel color from the end.
    let (nums, color_raw) = if let Some(idx) = css.find("rgba(").or_else(|| css.find("rgb(")) {
        (css[..idx].trim(), css[idx..].trim())
    } else {
        let idx = css.rfind('#')?;
        (css[..idx].trim(), css[idx..].trim())
    };
    let parts: Vec<f64> = nums
        .split_whitespace()
        .filter_map(|p| p.trim_end_matches("px").parse().ok())
        .collect();
    if parts.len() < 2 {
        return None;
    }
    let dx = parts[0];
    let dy = parts[1];
    let blur = parts.get(2).copied().unwrap_or(0.0);
    let (color, opacity) = parse_css_color_opacity(color_raw);
    Some(CssBoxShadow {
        dx,
        dy,
        blur,
        color,
        opacity,
    })
}

fn parse_css_color_opacity(raw: &str) -> (String, f64) {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix("rgba(").and_then(|s| s.strip_suffix(')')) {
        let vals: Vec<&str> = rest.split(',').map(str::trim).collect();
        if vals.len() == 4 {
            if let (Ok(r), Ok(g), Ok(b), Ok(a)) = (
                vals[0].parse::<u8>(),
                vals[1].parse::<u8>(),
                vals[2].parse::<u8>(),
                vals[3].parse::<f64>(),
            ) {
                return (format!("#{r:02X}{g:02X}{b:02X}"), a.clamp(0.0, 1.0));
            }
        }
    }
    if raw.starts_with('#') {
        return (raw.to_owned(), 1.0);
    }
    ("#000000".into(), 0.38)
}

fn inject_filter_attr(markup: &str, filter_id: &str) -> String {
    let needle = "/>";
    if let Some(pos) = markup.find(needle) {
        let insert = format!(" filter=\"url(#{})\"", xml_escape_attr(filter_id));
        let mut s = String::with_capacity(markup.len() + insert.len());
        s.push_str(&markup[..pos]);
        s.push_str(&insert);
        s.push_str(&markup[pos..]);
        s
    } else {
        markup.to_owned()
    }
}

fn xml_escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

#[cfg(test)]
mod tests {
    use super::parse_css_box_shadow;

    // Catches: effectRef box-shadow CSS never becoming SVG feDropShadow on cxnSp strokes.
    #[test]
    fn parses_theme_outer_shadow_css() {
        let s = parse_css_box_shadow("0px 2px 2px rgba(0, 0, 0, 0.38)").expect("parse");
        assert!((s.dx - 0.0).abs() < 1e-9);
        assert!((s.dy - 2.0).abs() < 1e-9);
        assert!((s.blur - 2.0).abs() < 1e-9);
        assert_eq!(s.color, "#000000");
        assert!((s.opacity - 0.38).abs() < 1e-9);
    }
}
