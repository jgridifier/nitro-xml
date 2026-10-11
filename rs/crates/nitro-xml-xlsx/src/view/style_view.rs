// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/style-view.ts; changes listed in PROVENANCE.md.

use crate::model::{Border, BorderEdge, Fill, Font, ThemePalette};
use crate::view::colors::resolve_color;
use crate::view::types::{BordersView, EdgeView, FillView, FontView};

const BLACK: &str = "#000000";
const WHITE: &str = "#FFFFFF";
const PX_PER_PT: f64 = 96.0 / 72.0;

pub fn merge_font(base: &Font, font: Option<&Font>) -> Font {
    let Some(font) = font else {
        return base.clone();
    };
    let mut out = base.clone();
    if font.name.is_some() {
        out.name = font.name.clone();
    }
    if font.size.is_some() {
        out.size = font.size;
    }
    if font.bold.is_some() {
        out.bold = font.bold;
    }
    if font.italic.is_some() {
        out.italic = font.italic;
    }
    if font.underline.is_some() {
        out.underline = font.underline.clone();
    }
    if font.strike.is_some() {
        out.strike = font.strike;
    }
    if font.color.is_some() {
        out.color = font.color.clone();
    }
    if font.vert_align.is_some() {
        out.vert_align = font.vert_align.clone();
    }
    if font.family.is_some() {
        out.family = font.family;
    }
    if font.scheme.is_some() {
        out.scheme = font.scheme.clone();
    }
    out
}

pub fn font_view(font: &Font, theme: &ThemePalette, indexed: &[&str]) -> FontView {
    let family = if font.scheme.as_deref() == Some("major") {
        if theme.major_font.is_empty() {
            font.name.clone().unwrap_or_else(|| "Calibri Light".into())
        } else {
            theme.major_font.clone()
        }
    } else if font.scheme.as_deref() == Some("minor") {
        if theme.minor_font.is_empty() {
            font.name.clone().unwrap_or_else(|| "Calibri".into())
        } else {
            theme.minor_font.clone()
        }
    } else {
        font.name
            .clone()
            .unwrap_or_else(|| theme.minor_font.clone())
    };
    let size = font.size.filter(|s| *s > 0.0).unwrap_or(11.0);
    let mut view = FontView {
        family,
        size_px: size * PX_PER_PT,
        bold: font.bold == Some(true),
        italic: font.italic == Some(true),
        strike: font.strike == Some(true),
        color: resolve_color(font.color.as_ref(), theme, Some(BLACK), indexed)
            .unwrap_or_else(|| BLACK.to_string()),
        underline: None,
        vert_align: None,
    };
    if let Some(u) = &font.underline {
        view.underline = Some(
            if u == "double" || u == "doubleAccounting" {
                "double"
            } else {
                "single"
            }
            .into(),
        );
    }
    if font.vert_align.as_deref() == Some("superscript") {
        view.vert_align = Some("super".into());
    } else if font.vert_align.as_deref() == Some("subscript") {
        view.vert_align = Some("sub".into());
    }
    view
}

fn round(n: f64) -> f64 {
    (n * 100.0).round() / 100.0
}

pub fn fill_view(
    fill: Option<&Fill>,
    theme: &ThemePalette,
    differential: bool,
    indexed: &[&str],
) -> Option<FillView> {
    let fill = fill?;
    match fill {
        Fill::Gradient {
            gradient,
            degree,
            stops,
        } => {
            let mut stops_sorted = stops.clone();
            stops_sorted.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
            let stops_css: Vec<String> = stops_sorted
                .iter()
                .map(|s| {
                    format!(
                        "{} {}%",
                        resolve_color(Some(&s.color), theme, Some(WHITE), indexed)
                            .unwrap_or_else(|| WHITE.to_string()),
                        round(s.position * 100.0)
                    )
                })
                .collect();
            if stops_css.is_empty() {
                return None;
            }
            let mut stops_css = stops_css;
            if stops_css.len() == 1 {
                stops_css.push(stops_css[0].clone());
            }
            let css = if gradient == "path" {
                format!("radial-gradient(circle, {})", stops_css.join(", "))
            } else {
                let deg = round((degree.unwrap_or(0.0) + 90.0) % 360.0);
                format!("linear-gradient({}deg, {})", deg, stops_css.join(", "))
            };
            Some(FillView::Gradient { gradient: css })
        }
        Fill::Pattern {
            pattern,
            fg_color,
            bg_color,
        } => {
            if pattern == "none" {
                if differential && bg_color.is_some() {
                    return Some(FillView::Background {
                        background: resolve_color(bg_color.as_ref(), theme, Some(WHITE), indexed)
                            .unwrap_or_else(|| WHITE.to_string()),
                    });
                }
                return None;
            }
            if pattern == "solid" {
                let color = if differential {
                    bg_color.as_ref().or(fg_color.as_ref())
                } else {
                    fg_color.as_ref().or(bg_color.as_ref())
                };
                return Some(FillView::Background {
                    background: resolve_color(color, theme, Some(BLACK), indexed)
                        .unwrap_or_else(|| BLACK.to_string()),
                });
            }
            Some(FillView::Pattern {
                pattern: pattern.clone(),
                fg: resolve_color(fg_color.as_ref(), theme, Some(BLACK), indexed)
                    .unwrap_or_else(|| BLACK.to_string()),
                bg: resolve_color(bg_color.as_ref(), theme, Some(WHITE), indexed)
                    .unwrap_or_else(|| WHITE.to_string()),
            })
        }
    }
}

fn border_style(style: &str) -> Option<(f64, &'static str)> {
    match style {
        "hair" => Some((1.0, "dotted")),
        "thin" => Some((1.0, "solid")),
        "dotted" => Some((1.0, "dotted")),
        "dashed" => Some((1.0, "dashed")),
        "dashDot" => Some((1.0, "dashed")),
        "dashDotDot" => Some((1.0, "dashed")),
        "medium" => Some((2.0, "solid")),
        "mediumDashed" => Some((2.0, "dashed")),
        "mediumDashDot" => Some((2.0, "dashed")),
        "mediumDashDotDot" => Some((2.0, "dashed")),
        "slantDashDot" => Some((2.0, "dashed")),
        "thick" => Some((3.0, "solid")),
        "double" => Some((3.0, "double")),
        _ => None,
    }
}

pub fn edge_view(
    edge: Option<&BorderEdge>,
    theme: &ThemePalette,
    indexed: &[&str],
) -> Option<EdgeView> {
    let edge = edge?;
    let (width_px, style) = border_style(&edge.style)?;
    Some(EdgeView {
        width_px,
        style: style.into(),
        color: resolve_color(edge.color.as_ref(), theme, Some(BLACK), indexed)
            .unwrap_or_else(|| BLACK.to_string()),
    })
}

pub fn merge_border(base: &Border, border: Option<&Border>) -> Border {
    let Some(border) = border else {
        return base.clone();
    };
    let mut out = base.clone();
    if border.left.is_some() {
        out.left = border.left.clone();
    }
    if border.right.is_some() {
        out.right = border.right.clone();
    }
    if border.top.is_some() {
        out.top = border.top.clone();
    }
    if border.bottom.is_some() {
        out.bottom = border.bottom.clone();
    }
    if border.diagonal.is_some() {
        out.diagonal = border.diagonal.clone();
    }
    out
}

pub fn borders_view(border: &Border, theme: &ThemePalette, indexed: &[&str]) -> BordersView {
    let mut out = BordersView::default();
    if let Some(e) = edge_view(border.top.as_ref(), theme, indexed) {
        out.top = Some(e);
    }
    if let Some(e) = edge_view(border.right.as_ref(), theme, indexed) {
        out.right = Some(e);
    }
    if let Some(e) = edge_view(border.bottom.as_ref(), theme, indexed) {
        out.bottom = Some(e);
    }
    if let Some(e) = edge_view(border.left.as_ref(), theme, indexed) {
        out.left = Some(e);
    }
    if border.diagonal_up == Some(true) {
        out.diagonal_up = edge_view(border.diagonal.as_ref(), theme, indexed);
    }
    if border.diagonal_down == Some(true) {
        out.diagonal_down = edge_view(border.diagonal.as_ref(), theme, indexed);
    }
    out
}
