// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-svg-appearance.ts (S1 subset)

use std::sync::atomic::{AtomicUsize, Ordering};

use super::chart_appearance::chart_points_to_pixels;
use super::chart_view::ChartViewModel;
use super::css_font_family::css_font_family;
use super::drawing_shadow::svg_drop_shadow_element;
use super::gradient_definition::{build_chart_gradient_def, ChartGradientFill};
use super::gradient_markup::chart_gradient_markup;
use super::svg_util::{esc, n, Rect, TextAttrs, AXIS_COLOR, GRID_COLOR};

static NEXT_PAINT_ID: AtomicUsize = AtomicUsize::new(0);

fn push_shadow_filter(
    defs: &mut Vec<String>,
    prefix: &str,
    shadow: &super::drawing_shadow::DrawingSvgShadow,
    suffix: &str,
    width: u32,
    height: u32,
) -> String {
    let id = format!("{}-{}", prefix, suffix);
    let pad = (shadow.blur * 3.0 + shadow.dx.abs() + shadow.dy.abs()).max(4.0);
    defs.push(format!(
        "<filter id=\"{}\" filterUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" color-interpolation-filters=\"sRGB\">{}</filter>",
        id,
        n(-pad),
        n(-pad),
        n(width as f64 + pad * 2.0),
        n(height as f64 + pad * 2.0),
        svg_drop_shadow_element(shadow)
    ));
    format!("url(#{})", id)
}

pub fn chart_gradient_paint(
    model: &ChartViewModel,
    width: u32,
    height: u32,
) -> (ChartViewModel, String) {
    let prefix = format!(
        "xlsx-chart-{}",
        NEXT_PAINT_ID.fetch_add(1, Ordering::Relaxed) + 1
    );
    let mut out_model = model.clone();
    let mut defs: Vec<String> = Vec::new();
    for (si, s) in out_model.series.iter_mut().enumerate() {
        if let Some(shadow) = &s.shadow {
            s.shadow_filter = Some(push_shadow_filter(
                &mut defs,
                &prefix,
                shadow,
                &format!("s{}-shadow", si),
                width,
                height,
            ));
        }
        let bounds = if matches!(
            out_model.type_,
            super::summary::ChartSummaryType::Bar | super::summary::ChartSummaryType::Column
        ) {
            Some((1.0, 1.0))
        } else {
            None
        };
        if let Some(g) = &s.gradient {
            let id = format!("{}-s{}", prefix, si);
            s.color = paint_gradient(g, &id, bounds, &mut defs);
        }
    }
    let defs_str = if defs.is_empty() {
        String::new()
    } else {
        format!("<defs>{}</defs>", defs.join(""))
    };
    (out_model, defs_str)
}

fn paint_gradient(
    gradient: &ChartGradientFill,
    id: &str,
    bounds: Option<(f64, f64)>,
    defs: &mut Vec<String>,
) -> String {
    let (w, h) = bounds.unwrap_or((1024.0, 768.0));
    let def = build_chart_gradient_def(id, gradient, w, h);
    defs.push(chart_gradient_markup(&def));
    format!("url(#{})", id)
}

pub fn chart_text_attributes(model: &ChartViewModel, part: &str, fallback: f64) -> TextAttrs {
    let entry = model.appearance.as_ref().and_then(|a| a.get(part));
    TextAttrs {
        size: entry
            .and_then(|e| e.font_size)
            .map(chart_points_to_pixels)
            .or(Some(fallback)),
        fill: entry.and_then(|e| e.color.clone()),
        weight: entry
            .and_then(|e| e.bold)
            .map(|b| if b { "bold" } else { "normal" }),
        italic: entry.and_then(|e| e.italic),
        underline: entry.and_then(|e| e.underline),
        family: entry
            .and_then(|e| e.color.as_ref())
            .map(|_| css_font_family("Calibri")),
        ..Default::default()
    }
}

pub fn chart_stroke(model: &ChartViewModel, part: &str) -> (String, f64) {
    let entry = model.appearance.as_ref().and_then(|a| a.get(part));
    let color = entry.and_then(|e| e.line_color.clone()).unwrap_or_else(|| {
        if part == "gridlineMajor" {
            GRID_COLOR
        } else {
            AXIS_COLOR
        }
        .to_string()
    });
    let width = entry
        .and_then(|e| e.line_width)
        .map(chart_points_to_pixels)
        .unwrap_or(1.0);
    (color, width)
}

pub fn chart_area_rect(model: &ChartViewModel, part: &str, area: Rect) -> String {
    if area.w <= 0.0 || area.h <= 0.0 {
        return String::new();
    }
    let entry = model.appearance.as_ref().and_then(|a| a.get(part));
    let fill = entry.and_then(|e| e.fill_color.clone()).unwrap_or_else(|| {
        if part == "chartArea" {
            "#FFFFFF".to_string()
        } else {
            "none".to_string()
        }
    });
    let stroke = entry
        .and_then(|e| e.line_color.clone())
        .unwrap_or_else(|| "none".to_string());
    if fill == "none" && stroke == "none" {
        return String::new();
    }
    let width = entry
        .and_then(|e| e.line_width)
        .map(chart_points_to_pixels)
        .unwrap_or(1.0);
    format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\" data-chart-part=\"{}\"/>",
        n(area.x),
        n(area.y),
        n(area.w),
        n(area.h),
        esc(&fill),
        esc(&stroke),
        n(width),
        part
    )
}
