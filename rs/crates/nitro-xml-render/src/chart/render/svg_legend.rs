// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-svg-legend.ts

use super::chart_view::ChartViewModel;
use super::summary::ChartSummaryType;
use super::svg_appearance::{chart_area_rect, chart_text_attributes};
use super::svg_text_metrics::{chart_text_width, fit_chart_text, ChartSvgMeasureOptions};
use super::svg_util::{rect, text, Rect};

pub struct LegendResult {
    pub plot: Rect,
    pub markup: String,
    pub foreground: bool,
}

pub fn chart_legend_svg(
    model: &ChartViewModel,
    area: Rect,
    _frame_w: f64,
    _frame_h: f64,
    measure: &mut ChartSvgMeasureOptions,
) -> LegendResult {
    let mut automatic: Vec<String> = Vec::new();
    let mut bounds_box: Option<Rect> = None;
    let reserved = automatic_legend(model, area, &mut automatic, measure, &mut bounds_box);
    let plot = if model.legend_overlay { area } else { reserved };
    if bounds_box.is_none() {
        return LegendResult {
            plot,
            markup: String::new(),
            foreground: false,
        };
    }
    LegendResult {
        plot,
        markup: automatic.join(""),
        foreground: model.legend_overlay,
    }
}

fn legend_entries(model: &ChartViewModel) -> Vec<(String, String)> {
    if matches!(
        model.type_,
        ChartSummaryType::Pie | ChartSummaryType::Doughnut
    ) {
        let first = model.series.first();
        return model
            .categories
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let color = first
                    .and_then(|s| s.point_colors.as_ref().and_then(|c| c.get(i).cloned()))
                    .unwrap_or_else(|| {
                        first
                            .map(|s| s.color.clone())
                            .unwrap_or_else(|| "#4472C4".to_string())
                    });
                (label.clone(), color)
            })
            .collect();
    }
    model
        .series
        .iter()
        .map(|s| (s.name.clone(), s.color.clone()))
        .collect()
}

fn automatic_legend(
    model: &ChartViewModel,
    area: Rect,
    out: &mut Vec<String>,
    measure: &mut ChartSvgMeasureOptions,
    bounds: &mut Option<Rect>,
) -> Rect {
    let entries = legend_entries(model);
    if !model.show_legend || entries.is_empty() {
        return area;
    }
    let pos = model.legend_position.as_str();
    let attrs = chart_text_attributes(model, "legend", 10.0);
    let size = attrs.size.unwrap_or(10.0);
    let row_h = (size * 1.4).max(16.0);
    if pos == "t" || pos == "b" {
        let widths: Vec<f64> = entries
            .iter()
            .map(|(label, _)| chart_text_width(label, &attrs, measure).min(area.w / 2.0) + 20.0)
            .collect();
        let total = widths.iter().sum::<f64>();
        let mut x = area.x + (area.w - total).max(0.0) / 2.0;
        let y = if pos == "t" {
            area.y + 4.0
        } else {
            area.y + area.h - row_h + 4.0
        };
        *bounds = Some(Rect {
            x: x - 4.0,
            y: y - 4.0,
            w: total.min(area.w) + 8.0,
            h: row_h + 4.0,
        });
        out.push(chart_area_rect(model, "legend", bounds.unwrap()));
        for (i, (label, color)) in entries.iter().enumerate() {
            let w = widths.get(i).copied().unwrap_or(0.0);
            out.push(rect(x, y, 8.0, 8.0, color, None));
            out.push(text(
                x + 12.0,
                y + 8.0,
                &fit_chart_text(label, w - 16.0, &attrs, measure),
                &attrs,
            ));
            x += w;
        }
        return if pos == "t" {
            Rect {
                x: area.x,
                y: area.y + row_h + 4.0,
                w: area.w,
                h: area.h - row_h - 4.0,
            }
        } else {
            Rect {
                x: area.x,
                y: area.y,
                w: area.w,
                h: area.h - row_h - 4.0,
            }
        };
    }
    let width = (area.w * 0.35).min(
        entries
            .iter()
            .map(|(l, _)| chart_text_width(l, &attrs, measure))
            .fold(0.0, f64::max)
            + 22.0,
    );
    let height = entries.len() as f64 * row_h;
    let x = if pos == "l" {
        area.x + 4.0
    } else {
        area.x + area.w - width
    };
    let y0 = if pos == "tr" {
        area.y + 4.0
    } else {
        area.y + (area.h - height).max(0.0) / 2.0
    };
    *bounds = Some(Rect {
        x: x - 4.0,
        y: y0,
        w: width + 4.0,
        h: height.min(area.y + area.h - y0),
    });
    out.push(chart_area_rect(model, "legend", bounds.unwrap()));
    for (i, (label, color)) in entries.iter().enumerate() {
        let y = y0 + i as f64 * row_h;
        if y + row_h > area.y + area.h + 2.0 {
            continue;
        }
        out.push(rect(x, y + 3.0, 8.0, 8.0, color, None));
        out.push(text(
            x + 12.0,
            y + 11.0,
            &fit_chart_text(label, width - 16.0, &attrs, measure),
            &attrs,
        ));
    }
    if pos == "l" {
        Rect {
            x: area.x + width + 8.0,
            y: area.y,
            w: area.w - width - 8.0,
            h: area.h,
        }
    } else {
        Rect {
            x: area.x,
            y: area.y,
            w: area.w - width - 8.0,
            h: area.h,
        }
    }
}
