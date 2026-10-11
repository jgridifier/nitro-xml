// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-value-axis.ts

use super::chart_scale::{nice_scale, AxisScale, NiceScaleOptions};
use super::chart_view::{ChartViewModel, ValueAxisView};
use super::summary::ChartSummaryType;

pub fn value_axis(model: &ChartViewModel) -> ValueAxisView {
    let stackable = matches!(
        model.type_,
        ChartSummaryType::Bar
            | ChartSummaryType::Column
            | ChartSummaryType::Line
            | ChartSummaryType::Area
    );
    if stackable && model.grouping == "percentStacked" {
        return ValueAxisView {
            scale: AxisScale {
                min: 0.0,
                max: 1.0,
                major_unit: 0.1,
                ticks: vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0],
            },
            percent: true,
        };
    }
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    if stackable && model.grouping == "stacked" {
        for i in 0..model.categories.len() {
            let mut pos = 0.0;
            let mut neg = 0.0;
            for s in &model.series {
                let v = s.values.get(i).copied().flatten().unwrap_or(0.0);
                if v >= 0.0 {
                    pos += v;
                } else {
                    neg += v;
                }
            }
            min = min.min(neg);
            max = max.max(pos);
        }
    } else {
        for s in &model.series {
            for v in &s.values {
                if let Some(v) = v {
                    min = min.min(*v);
                    max = max.max(*v);
                }
            }
        }
    }
    if min == f64::INFINITY {
        min = 0.0;
        max = 1.0;
    }
    ValueAxisView {
        scale: nice_scale(min, max, NiceScaleOptions::default()),
        percent: false,
    }
}

pub fn category_totals(model: &ChartViewModel) -> Vec<f64> {
    model
        .categories
        .iter()
        .enumerate()
        .map(|(i, _)| {
            model
                .series
                .iter()
                .map(|s| s.values.get(i).copied().flatten().unwrap_or(0.0).abs())
                .sum()
        })
        .collect()
}
