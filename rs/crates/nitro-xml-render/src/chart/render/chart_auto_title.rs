// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/chart-auto-title.ts; changes listed in PROVENANCE.md.

use super::chart_view::ChartViewModel;

/// PowerPoint's placeholder for an auto title on a multi-series chart.
pub const CHART_AUTO_TITLE: &str = "Chart Title";

/// Upstream `withAutoTitle` — UI / P9 only (`bar3d_extrusion`), never on S1 oracle dumps.
///
/// A classic chart with no `c:title` still gets `hasTitle` when it plots exactly one
/// named series and `c:autoTitleDeleted` is not set.
pub fn apply_with_auto_title(view: &mut ChartViewModel, auto_title_deleted: Option<bool>) {
    if view.title_placeholder
        || view.title.is_some()
        || auto_title_deleted == Some(true)
        || view.series.len() != 1
        || view.series[0].name.is_empty()
    {
        return;
    }
    view.title_placeholder = true;
    view.auto_title = true;
}

/// Title text to draw on the pptx bar3D extrusion path (matches upstream `resolveChartTitleText`).
pub fn resolve_p9_chart_title_text(model: &ChartViewModel) -> Option<String> {
    if !model.title_placeholder {
        return model.title.clone();
    }
    if let Some(title) = &model.title {
        return Some(title.clone());
    }
    if model.series.is_empty() {
        return None;
    }
    if model.series.len() == 1 {
        return Some(model.series[0].name.clone());
    }
    Some(CHART_AUTO_TITLE.to_string())
}

#[cfg(test)]
mod tests {
    use super::super::chart_view::{ChartSeriesView, ChartViewModel};
    use super::super::summary::ChartSummaryType;
    use super::*;

    fn bare_model(title_placeholder: bool, title: Option<&str>, series: usize) -> ChartViewModel {
        ChartViewModel {
            type_: ChartSummaryType::Column,
            grouping: "clustered".to_string(),
            title: title.map(str::to_string),
            title_placeholder,
            auto_title: false,
            show_legend: true,
            legend_position: "b".to_string(),
            categories: vec!["A".to_string()],
            series: (0..series)
                .map(|i| ChartSeriesView {
                    name: format!("Series {}", i + 1),
                    values: vec![Some(1.0)],
                    x_values: None,
                    color: "#000".to_string(),
                    point_colors: None,
                    shadow_filter: None,
                    shadow: None,
                    gradient: None,
                    point_gradients: None,
                })
                .collect(),
            value_axis: None,
            x_axis: None,
            horizontal: false,
            supported: true,
            appearance: None,
            bar_gap_width: None,
            bar_overlap: None,
            is3_d: true,
            view3_d: None,
            bar3d_extrusion: true,
            picture_fills_enabled: false,
            picture_prefix: "chart".to_string(),
            picture_fills: Default::default(),
            title_overlay: false,
            title_layout: None,
            title_text: None,
            legend_overlay: false,
            category_label_offset: 100.0,
        }
    }

    // Catches: P9 bar3d path must synthesize CHART_AUTO_TITLE for multi-series empty c:title.
    #[test]
    fn resolve_p9_multi_series_placeholder_uses_chart_auto_title() {
        let model = bare_model(true, None, 3);
        assert_eq!(
            resolve_p9_chart_title_text(&model).as_deref(),
            Some(CHART_AUTO_TITLE)
        );
    }

    // Catches: S1/oracle callers must not use resolve_p9 when title_placeholder is off.
    #[test]
    fn resolve_p9_without_placeholder_returns_none() {
        let model = bare_model(false, None, 3);
        assert_eq!(resolve_p9_chart_title_text(&model), None);
    }

    // Catches: withAutoTitle must arm title_placeholder+auto_title for a lone named series
    // (f8f44 no-c:title) without writing the series name into title (resolve_p9 does that).
    #[test]
    fn apply_with_auto_title_arms_single_named_series() {
        let mut model = bare_model(false, None, 1);
        apply_with_auto_title(&mut model, None);
        assert!(model.title_placeholder);
        assert!(model.auto_title);
        assert!(model.title.is_none());
        assert_eq!(
            resolve_p9_chart_title_text(&model).as_deref(),
            Some("Series 1")
        );
    }

    // Catches: withAutoTitle must not fire when autoTitleDeleted or multi-series (S1 stays clean).
    #[test]
    fn apply_with_auto_title_skips_deleted_and_multi_series() {
        let mut deleted = bare_model(false, None, 1);
        apply_with_auto_title(&mut deleted, Some(true));
        assert!(!deleted.title_placeholder);
        assert!(!deleted.auto_title);

        let mut multi = bare_model(false, None, 2);
        apply_with_auto_title(&mut multi, None);
        assert!(!multi.title_placeholder);
        assert!(!multi.auto_title);
    }
}
