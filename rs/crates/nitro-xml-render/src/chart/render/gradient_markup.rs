// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/gradient-markup.ts

use super::gradient_coincident_stops::GradientStop;
use super::gradient_definition::ChartSvgGradientDef;
use super::svg_util::esc;

fn stops_markup(stops: &[GradientStop]) -> String {
    stops
        .iter()
        .map(|stop| {
            format!(
                "<stop offset=\"{}\" stop-color=\"{}\" stop-opacity=\"{}\"/>",
                stop.offset,
                esc(&stop.color),
                stop.opacity.unwrap_or(1.0)
            )
        })
        .collect()
}

pub fn chart_gradient_markup(def: &ChartSvgGradientDef) -> String {
    match def {
        ChartSvgGradientDef::Linear {
            id,
            gradient_units,
            x1,
            y1,
            x2,
            y2,
            stops,
        } => {
            let units = gradient_units
                .as_ref()
                .map(|u| format!("gradientUnits=\"{}\" ", u))
                .unwrap_or_default();
            format!(
                "<linearGradient id=\"{}\" {}x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">{}</linearGradient>",
                esc(id),
                units,
                x1,
                y1,
                x2,
                y2,
                stops_markup(stops)
            )
        }
        ChartSvgGradientDef::Radial {
            id,
            cx,
            cy,
            r,
            gradient_transform,
            stops,
        } => {
            let transform = gradient_transform
                .as_ref()
                .map(|t| format!(" gradientTransform=\"{}\"", esc(t)))
                .unwrap_or_default();
            format!(
                "<radialGradient id=\"{}\" cx=\"{}\" cy=\"{}\" r=\"{}\">{}{}</radialGradient>",
                esc(id),
                cx,
                cy,
                r,
                transform,
                stops_markup(stops)
            )
        }
    }
}
