// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-svg-text-metrics.ts

use super::svg_util::{fit, text_width, TextAttrs};

pub const CHART_FONT_FAMILY: &str = "Calibri, Carlito, Arial, sans-serif";

#[derive(Clone, Debug, Default)]
pub struct ChartSvgMeasureOptions {
    pub measure_widths: Vec<f64>,
    pub width_index: usize,
}

impl ChartSvgMeasureOptions {
    pub fn from_widths(widths: Vec<f64>) -> Self {
        Self {
            measure_widths: widths,
            width_index: 0,
        }
    }

    fn next_width(&mut self, content: &str, attrs: &TextAttrs) -> Option<f64> {
        if self.width_index < self.measure_widths.len() {
            let w = self.measure_widths[self.width_index];
            self.width_index += 1;
            if w.is_finite() && w >= 0.0 {
                return Some(w);
            }
        }
        let size = attrs.size.unwrap_or(10.0);
        Some(text_width(content, size))
    }
}

fn css_font(attrs: &TextAttrs) -> String {
    format!(
        "{} {} {}px {}",
        if attrs.italic.unwrap_or(false) {
            "italic"
        } else {
            "normal"
        },
        attrs.weight.unwrap_or("normal"),
        attrs.size.unwrap_or(10.0),
        attrs.family.as_deref().unwrap_or(CHART_FONT_FAMILY)
    )
}

pub fn chart_text_width(
    content: &str,
    attrs: &TextAttrs,
    options: &mut ChartSvgMeasureOptions,
) -> f64 {
    options
        .next_width(content, attrs)
        .unwrap_or_else(|| text_width(content, attrs.size.unwrap_or(10.0)))
}

pub fn fit_chart_text(
    content: &str,
    max_width: f64,
    attrs: &TextAttrs,
    options: &mut ChartSvgMeasureOptions,
) -> String {
    if options.measure_widths.is_empty() {
        return fit(content, max_width, attrs.size.unwrap_or(10.0));
    }
    if chart_text_width(content, attrs, options) <= max_width {
        return content.to_string();
    }
    let chars: Vec<char> = content.chars().collect();
    let mut low = 0usize;
    let mut high = chars.len();
    while low < high {
        let mid = (low + high).div_ceil(2);
        let trial: String = chars.iter().take(mid).collect();
        if chart_text_width(&format!("{}…", trial), attrs, options) <= max_width {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    format!("{}…", chars.iter().take(low).collect::<String>())
}

pub fn chart_font_metrics(
    _attrs: &TextAttrs,
    _options: &ChartSvgMeasureOptions,
) -> Option<(f64, f64)> {
    None
}
