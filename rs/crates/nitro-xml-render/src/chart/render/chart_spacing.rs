// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-spacing.ts

use super::bar_cluster_geometry::DEFAULT_BAR_GAP_WIDTH;
use super::summary::ChartSummary;

pub fn chart_bar_spacing(chart: &ChartSummary) -> (i32, i32) {
    let stacked = chart.grouping.as_deref() == Some("stacked")
        || chart.grouping.as_deref() == Some("percentStacked");
    (
        chart.bar_gap_width.unwrap_or(DEFAULT_BAR_GAP_WIDTH),
        chart.bar_overlap.unwrap_or(if stacked { 100 } else { 0 }),
    )
}
