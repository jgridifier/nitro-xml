// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-svg-title-text.ts (stub: no rich title without formatting IR)

use super::chart_view::ChartViewModel;
use super::svg_text_metrics::ChartSvgMeasureOptions;

pub struct RichTitle {
    pub markup: String,
    pub height: f64,
}

pub fn chart_rich_title_svg(
    _model: &ChartViewModel,
    _width: u32,
    _options: &mut ChartSvgMeasureOptions,
    _height: u32,
) -> Option<RichTitle> {
    None
}
