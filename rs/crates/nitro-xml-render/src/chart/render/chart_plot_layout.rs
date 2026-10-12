// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/chart-view-model-layout.ts (axis band subset); changes listed in PROVENANCE.md.

/// CSS pixels per typographic point (96 dpi / 72 dpi).
pub const CHART_PX_PER_PT: f64 = 4.0 / 3.0;

/// PowerPoint default chart body text (10 pt) in slide px.
pub const DEFAULT_CHART_TEXT_PX: f64 = 10.0 * CHART_PX_PER_PT;

/// Horizontal space reserved left of the plot for the value-axis tick band (P9 `leftAxisBand`).
pub fn left_value_axis_band_px(measured_tick_reserve: f64) -> f64 {
    measured_tick_reserve.max(40.0)
}

/// Vertical space under the plot for the category axis (`categoryAxisBand` in P9).
pub fn category_axis_band_px(label_offset_percent: f64, category_font_px: f64) -> f64 {
    let offset = 4.0 + 8.0 * (label_offset_percent / 100.0);
    24.0_f64.max(offset + category_font_px * 1.2)
}

const CHART_TITLE_TOP_INSET_PX: f64 = 10.8 * CHART_PX_PER_PT;
const TITLE_BOTTOM_INSET_PX: f64 = 3.6 * CHART_PX_PER_PT;

/// P9 `chartTitleReservePx`: height reserved above the plot for the title band.
pub fn chart_title_reserve_px(has_title: bool, title_font_px: f64) -> f64 {
    if !has_title {
        return 0.0;
    }
    let size = if title_font_px.is_finite() && title_font_px > 0.0 {
        title_font_px
    } else {
        12.0
    };
    let baseline = CHART_TITLE_TOP_INSET_PX + size * 0.95;
    baseline + size * 0.25 + TITLE_BOTTOM_INSET_PX
}

#[derive(Clone, Copy, Debug)]
pub struct PlotLayout {
    pub plot_left: f64,
    pub plot_top: f64,
    pub plot_width: f64,
    pub plot_height: f64,
}

/// P9 `computePlotLayout` for the common single-axis chart (no data table / secondary axes).
pub fn compute_plot_layout(
    svg_width: f64,
    svg_height: f64,
    title_reserve_px: f64,
    has_legend: bool,
    legend_position: &str,
    category_label_offset: f64,
    category_font_px: f64,
) -> PlotLayout {
    let mut plot_left = 8.0 + left_value_axis_band_px(0.0);
    let mut plot_top = 8.0 + title_reserve_px;
    let mut plot_right = svg_width - 8.0;
    let mut plot_bottom =
        svg_height - category_axis_band_px(category_label_offset, category_font_px);

    if has_legend && legend_position != "tr" {
        match legend_position {
            "t" => plot_top += 20.0,
            "r" => plot_right -= 80.0,
            "l" => plot_left += 80.0,
            _ => plot_bottom -= 20.0,
        }
    }

    let plot_width = (plot_right - plot_left).max(1.0);
    let plot_height = (plot_bottom - plot_top).max(1.0);
    PlotLayout {
        plot_left,
        plot_top,
        plot_width,
        plot_height,
    }
}
