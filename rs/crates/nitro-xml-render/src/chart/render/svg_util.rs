// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-svg-util.ts

pub const TEXT_COLOR: &str = "#595959";
pub const GRID_COLOR: &str = "#D9D9D9";
pub const AXIS_COLOR: &str = "#BFBFBF";
pub const FONT_SIZE: f64 = 10.0;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn n(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    rounded.to_string()
}

pub fn text_width(text: &str, size: f64) -> f64 {
    text.len() as f64 * size * 0.55
}

#[derive(Clone, Debug, Default)]
pub struct TextAttrs {
    pub anchor: Option<&'static str>,
    pub size: Option<f64>,
    pub weight: Option<&'static str>,
    pub baseline: Option<&'static str>,
    pub fill: Option<String>,
    pub family: Option<String>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub filter: Option<String>,
}

pub fn text(x: f64, y: f64, content: &str, attrs: &TextAttrs) -> String {
    let size = attrs.size.unwrap_or(FONT_SIZE);
    let fill = esc(attrs.fill.as_deref().unwrap_or(TEXT_COLOR));
    let mut parts = vec![format!("x=\"{}\"", n(x)), format!("y=\"{}\"", n(y))];
    parts.push(format!("font-size=\"{}\"", size));
    parts.push(format!("fill=\"{}\"", fill));
    if let Some(anchor) = attrs.anchor {
        if anchor != "start" {
            parts.push(format!("text-anchor=\"{}\"", anchor));
        }
    }
    if let Some(weight) = attrs.weight {
        parts.push(format!("font-weight=\"{}\"", weight));
    }
    if let Some(baseline) = attrs.baseline {
        parts.push(format!("dominant-baseline=\"{}\"", baseline));
    }
    if let Some(family) = &attrs.family {
        parts.push(format!("font-family=\"{}\"", esc(family)));
    }
    if let Some(filter) = &attrs.filter {
        parts.push(format!("filter=\"{}\"", esc(filter)));
    }
    if let Some(italic) = attrs.italic {
        parts.push(format!(
            "font-style=\"{}\"",
            if italic { "italic" } else { "normal" }
        ));
    }
    if let Some(underline) = attrs.underline {
        parts.push(format!(
            "text-decoration=\"{}\"",
            if underline { "underline" } else { "none" }
        ));
    }
    format!("<text {}>{}</text>", parts.join(" "), esc(content))
}

pub fn line(x1: f64, y1: f64, x2: f64, y2: f64, stroke: &str, width: f64) -> String {
    format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>",
        n(x1),
        n(y1),
        n(x2),
        n(y2),
        stroke,
        width
    )
}

pub fn rect(x: f64, y: f64, w: f64, h: f64, fill: &str, filter: Option<&str>) -> String {
    let filter_attr = filter
        .map(|f| format!(" filter=\"{}\"", esc(f)))
        .unwrap_or_default();
    format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"{} />",
        n(x),
        n(y),
        n(w.max(0.0)),
        n(h.max(0.0)),
        esc(fill),
        filter_attr
    )
}

pub fn fit(content: &str, max_width: f64, size: f64) -> String {
    if text_width(content, size) <= max_width {
        return content.to_string();
    }
    let chars = ((max_width / (size * 0.55)).floor() as isize - 1).max(1) as usize;
    format!("{}…", content.chars().take(chars).collect::<String>())
}
