// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/connector-markers.ts; changes in PROVENANCE.md.

/// SVG `<marker>` path for a connector arrowhead (`viewBox="0 0 10 10"`).
pub fn marker_path_d(arrow_type: &str) -> (&'static str, bool) {
    match arrow_type {
        "diamond" => ("M5 0 L10 5 L5 10 L0 5 Z", false),
        "oval" => ("M5 2 A3 3 0 1 1 5 8 A3 3 0 1 1 5 2", false),
        "stealth" => ("M0 0 L10 5 L0 10 L3 5 Z", false),
        "arrow" => ("M1 1 L9 5 L1 9", true),
        _ => ("M0 0 L10 5 L0 10 Z", false),
    }
}

pub fn build_marker_def(id: &str, arrow_type: &str, stroke: &str) -> String {
    let (d, stroke_only) = marker_path_d(arrow_type);
    let marker_width = 4.0;
    let marker_height = 4.0;
    let path_attrs = if stroke_only {
        format!(
            "d=\"{}\" fill=\"none\" stroke=\"{}\"",
            xml_escape_attr(d),
            xml_escape_attr(stroke)
        )
    } else {
        format!(
            "d=\"{}\" fill=\"{}\"",
            xml_escape_attr(d),
            xml_escape_attr(stroke)
        )
    };
    if arrow_type == "oval" {
        return format!(
            "<marker id=\"{}\" viewBox=\"0 0 10 10\" refX=\"5\" refY=\"5\" markerWidth=\"{}\" markerHeight=\"{}\" orient=\"auto-start-reverse\" markerUnits=\"strokeWidth\">\
<circle cx=\"5\" cy=\"5\" r=\"4\" fill=\"{}\"/></marker>",
            xml_escape_attr(id),
            marker_width,
            marker_height,
            xml_escape_attr(stroke)
        );
    }
    format!(
        "<marker id=\"{}\" viewBox=\"0 0 10 10\" refX=\"5\" refY=\"5\" markerWidth=\"{}\" markerHeight=\"{}\" orient=\"auto-start-reverse\" markerUnits=\"strokeWidth\">\
<path {}/></marker>",
        xml_escape_attr(id),
        marker_width,
        marker_height,
        path_attrs
    )
}

pub fn attach_markers(
    markup: &str,
    marker_start: Option<&str>,
    marker_end: Option<&str>,
) -> String {
    let mut out = markup.to_owned();
    if let Some(id) = marker_start {
        out = inject_marker_attr(&out, "marker-start", id);
    }
    if let Some(id) = marker_end {
        out = inject_marker_attr(&out, "marker-end", id);
    }
    out
}

fn inject_marker_attr(markup: &str, attr: &str, id: &str) -> String {
    let needle = "/>";
    if let Some(pos) = markup.find(needle) {
        let insert = format!(" {}=\"url(#{})\"", attr, xml_escape_attr(id));
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
