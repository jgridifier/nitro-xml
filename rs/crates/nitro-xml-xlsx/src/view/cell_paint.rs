// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/ui/grid/cell-paint.ts (cssFont, hashes, hasBorders, repeatToFill);
// changes listed in PROVENANCE.md.

use crate::view::css_font_family::css_font_family;
use crate::view::types::{BordersView, EdgeView, FillView, FontView};

pub fn css_font(font: &FontView, zoom: f64) -> String {
    let size = ((font.size_px * (zoom / 100.0) * 100.0).round() / 100.0).max(1.0);
    let family = css_font_family(&font.family);
    // Capacity covers typical "italic bold 11px \"Calibri\", …" without realloc.
    let mut out = String::with_capacity(64 + family.len());
    if font.italic {
        out.push_str("italic ");
    }
    if font.bold {
        out.push_str("bold ");
    }
    use std::fmt::Write as _;
    let _ = write!(out, "{size}px {family}");
    out
}

/// Memoize `css_font` across cells that share family/weight/size/zoom (#30 S paint).
pub struct FontCssMemo {
    key: u64,
    css: String,
}

impl FontCssMemo {
    pub fn new() -> Self {
        Self {
            key: u64::MAX,
            css: String::new(),
        }
    }

    pub fn get_or_compute(&mut self, font: &FontView, zoom: f64) -> &str {
        let size_c = ((font.size_px * (zoom / 100.0) * 100.0).round() as i32).max(1) as u64;
        let mut key = size_c;
        key = key
            .wrapping_mul(0x9e37_79b9)
            .wrapping_add(font.family.len() as u64);
        for b in font.family.as_bytes() {
            key = key.wrapping_mul(31).wrapping_add(*b as u64);
        }
        if font.bold {
            key ^= 0x1000_0000;
        }
        if font.italic {
            key ^= 0x2000_0000;
        }
        if key != self.key {
            self.key = key;
            self.css = css_font(font, zoom);
        }
        &self.css
    }
}

impl Default for FontCssMemo {
    fn default() -> Self {
        Self::new()
    }
}

pub fn has_borders(b: &BordersView) -> bool {
    b.has_borders()
}

pub fn repeat_to_fill(text: &str, text_width: f64, cell_width: f64) -> String {
    if text.is_empty() || text_width <= 0.0 {
        return text.to_string();
    }
    let times = ((cell_width / text_width).floor() as usize).max(1);
    text.repeat(times.min(500))
}

pub fn hashes(cell_inner_width: f64, hash_width: f64) -> String {
    let n = ((cell_inner_width / hash_width.max(1.0)).floor() as usize).max(1);
    "#".repeat(n.min(200))
}

pub fn text_decoration(font: &FontView) -> &'static str {
    match (font.underline.is_some(), font.strike) {
        (false, false) => "none",
        (true, false) => "underline",
        (false, true) => "line-through",
        (true, true) => "underline line-through",
    }
}

pub struct FillPaint {
    pub background: Option<String>,
    pub image: Option<String>,
}

pub fn fill_paint(fill: Option<&FillView>) -> Option<FillPaint> {
    match fill {
        None => None,
        Some(FillView::Background { background }) => Some(FillPaint {
            background: Some(background.clone()),
            image: None,
        }),
        Some(FillView::Gradient { gradient }) => Some(FillPaint {
            background: None,
            image: Some(gradient.clone()),
        }),
        Some(FillView::Pattern { pattern, fg, bg }) => {
            if pattern == "none" {
                None
            } else if pattern == "solid" {
                Some(FillPaint {
                    background: Some(fg.clone()),
                    image: None,
                })
            } else {
                Some(FillPaint {
                    background: Some(bg.clone()),
                    image: None,
                })
            }
        }
    }
}

pub fn edge_css(edge: Option<&EdgeView>) -> String {
    match edge {
        None => String::new(),
        Some(e) => {
            let width = if e.style == "double" {
                (e.width_px.max(3.0)).round() as i32
            } else {
                e.width_px.round() as i32
            };
            format!("{}px {} {}", width, e.style, e.color)
        }
    }
}

pub fn justify(h_align: &str) -> &'static str {
    if h_align == "right" {
        "flex-end"
    } else if h_align == "center" || h_align == "centerContinuous" || h_align == "distributed" {
        "center"
    } else {
        "flex-start"
    }
}

pub fn align_items(v_align: &str) -> &'static str {
    if v_align == "top" {
        "flex-start"
    } else if v_align == "center" || v_align == "distributed" || v_align == "justify" {
        "center"
    } else {
        "flex-end"
    }
}

pub fn text_align(h_align: &str) -> &'static str {
    if h_align == "right" {
        "right"
    } else if h_align == "center" || h_align == "centerContinuous" {
        "center"
    } else if h_align == "justify" || h_align == "distributed" {
        "justify"
    } else {
        "left"
    }
}

pub fn diagonal_svg(b: &BordersView, w: i32, h: i32) -> Option<String> {
    if b.diagonal_up.is_none() && b.diagonal_down.is_none() {
        return None;
    }
    let line = |edge: &EdgeView, x1: i32, y1: i32, x2: i32, y2: i32| -> String {
        let dash = if edge.style == "dashed" {
            " stroke-dasharray=\"4 2\""
        } else if edge.style == "dotted" {
            " stroke-dasharray=\"1 2\""
        } else {
            ""
        };
        format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"{}",
            x1, y1, x2, y2, edge.color, edge.width_px, dash
        )
    };
    let mut body = String::new();
    if let Some(ref e) = b.diagonal_down {
        body.push_str(&line(e, 0, 0, w, h));
    }
    if let Some(ref e) = b.diagonal_up {
        body.push_str(&line(e, 0, h, w, 0));
    }
    Some(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">{}</svg>",
        w,
        h,
        w,
        h,
        body
    ))
}
