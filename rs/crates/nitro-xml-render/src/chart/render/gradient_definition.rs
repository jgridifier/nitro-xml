// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/gradient-definition.ts (S1 subset)

use nitro_xml_opc::drawingml::{DrawingColor, DrawingFill};
use std::collections::HashMap;

use super::gradient_coincident_stops::{spread_coincident_stops, GradientStop};
use super::gradient_stop_edit::sort_gradient_stops;
use super::sigma_gradient_stops::sigma_gradient_stops;

#[derive(Clone, Debug)]
pub struct ChartGradientFill {
    pub gradient_type: String,
    pub stops: Vec<GradientStop>,
    pub angle: Option<f64>,
    pub scaled: Option<bool>,
    pub interpolation: Option<String>,
    pub focal_x: Option<f64>,
    pub focal_y: Option<f64>,
    pub path: Option<String>,
    pub aspect: Option<f64>,
}

#[derive(Clone, Debug)]
pub enum ChartSvgGradientDef {
    Linear {
        id: String,
        gradient_units: Option<String>,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stops: Vec<GradientStop>,
    },
    Radial {
        id: String,
        cx: f64,
        cy: f64,
        r: f64,
        gradient_transform: Option<String>,
        stops: Vec<GradientStop>,
    },
}

pub fn resolve_chart_gradient(
    fill: &DrawingFill,
    scheme: &HashMap<String, String>,
) -> Option<ChartGradientFill> {
    let DrawingFill::Gradient {
        stops,
        angle,
        scaled,
        path,
        fill_to_rect,
        ..
    } = fill
    else {
        return None;
    };
    let resolve = |color: &DrawingColor| {
        nitro_xml_opc::drawingml::resolve_drawing_color(
            color,
            Some(scheme),
            Some(nitro_xml_opc::drawingml::ResolveOptions {
                transform_order: nitro_xml_opc::drawingml::TransformOrder::Document,
                hsl_rounding: nitro_xml_opc::drawingml::HslRounding::Nearest,
            }),
        )
    };
    let resolved_stops: Vec<GradientStop> = stops
        .iter()
        .filter_map(|stop| {
            resolve(&stop.color).map(|c| GradientStop {
                offset: stop.position / 100.0,
                color: c.hex.trim_start_matches('#').to_string(),
                opacity: Some(c.alpha),
            })
        })
        .collect();
    let interpolation = if path.is_none()
        && scaled == &Some(true)
        && stops.len() == 2
        && resolved_stops.len() == 2
        && resolved_stops
            .iter()
            .all(|s| (s.opacity.unwrap_or(1.0) - 1.0).abs() < 1e-9)
    {
        Some("sigma-gamma22".to_string())
    } else {
        None
    };
    let (focal_x, focal_y) = if let Some(focus) = fill_to_rect {
        (
            Some((1.0 + focus.l - focus.r) / 2.0),
            Some((1.0 + focus.t - focus.b) / 2.0),
        )
    } else {
        (None, None)
    };
    Some(ChartGradientFill {
        gradient_type: if path.is_some() { "radial" } else { "linear" }.to_string(),
        stops: resolved_stops,
        angle: *angle,
        scaled: *scaled,
        interpolation,
        focal_x,
        focal_y,
        path: path.clone(),
        aspect: None,
    })
}

pub fn build_chart_gradient_def(
    id: &str,
    fill: &ChartGradientFill,
    bounds_w: f64,
    bounds_h: f64,
) -> ChartSvgGradientDef {
    let source_stops = sort_gradient_stops(&fill.stops, |s| s.offset * 100.0)
        .into_iter()
        .map(|s| GradientStop {
            offset: s.offset.clamp(0.0, 1.0),
            color: s.color.clone(),
            opacity: s.opacity,
        })
        .collect::<Vec<_>>();
    let stops = if fill.interpolation.as_deref() == Some("sigma-gamma22") {
        sigma_gradient_stops(&source_stops).unwrap_or(source_stops)
    } else {
        source_stops
    };
    if fill.gradient_type == "radial" {
        let cx = fill.focal_x.unwrap_or(0.5);
        let cy = fill.focal_y.unwrap_or(0.5);
        let aspect = if fill.path.as_deref() == Some("circle") && bounds_w > 0.0 && bounds_h > 0.0 {
            bounds_h / bounds_w
        } else {
            1.0
        };
        let r = [0.0, 1.0]
            .into_iter()
            .flat_map(|x| {
                [0.0, 1.0].map(|y| ((x - cx).powi(2) + ((y - cy) * aspect).powi(2)).sqrt())
            })
            .fold(0.0_f64, f64::max);
        let gradient_transform = if aspect == 1.0 {
            None
        } else {
            Some(format!(
                "matrix(1 0 0 {} 0 {})",
                1.0 / aspect,
                cy * (1.0 - 1.0 / aspect)
            ))
        };
        return ChartSvgGradientDef::Radial {
            id: id.to_string(),
            cx,
            cy,
            r,
            gradient_transform,
            stops,
        };
    }
    let rad = fill.angle.unwrap_or(0.0) * std::f64::consts::PI / 180.0;
    let span = if fill.scaled == Some(true) {
        rad.cos().abs() + rad.sin().abs()
    } else {
        1.0
    };
    let dx = (rad.cos() * span) / 2.0;
    let dy = (rad.sin() * span) / 2.0;
    let round = |v: f64| (v * 10000.0).round() / 10000.0;
    ChartSvgGradientDef::Linear {
        id: id.to_string(),
        gradient_units: None,
        x1: round(0.5 - dx),
        y1: round(0.5 - dy),
        x2: round(0.5 + dx),
        y2: round(0.5 + dy),
        stops: spread_coincident_stops(&stops),
    }
}
