// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-title-text.ts

use nitro_xml_opc::chart::ChartTitle;
use nitro_xml_opc::drawingml::DrawingTextSpacing;

use super::style_resolve::ResolvedChartStyleEntry;
use super::summary::ChartSummary;

#[derive(Clone, Debug, Default)]
pub struct ChartTitleRun {
    pub text: String,
    pub appearance: ResolvedChartStyleEntry,
}

#[derive(Clone, Debug)]
pub struct ChartTitleParagraph {
    pub align: Option<String>,
    pub line_spacing: Option<DrawingTextSpacing>,
    pub space_before: Option<DrawingTextSpacing>,
    pub space_after: Option<DrawingTextSpacing>,
    pub runs: Vec<ChartTitleRun>,
}

#[derive(Clone, Debug)]
pub struct ChartTitleText {
    pub paragraphs: Vec<ChartTitleParagraph>,
}

/// Resolve imported mixed text through chart/theme precedence (`formatting.entries.title.textBody`).
///
/// Upstream only exposes rich title runs when the summary carries formatting with a title
/// `textBody` whose flat text matches `chart.title`. S1 does not yet surface that formatting
/// map on `ChartSummary`, so this returns `None` and the painter wraps the plain title string
/// (same as upstream when `formatting` is absent).
pub fn chart_title_text(
    chart: &ChartSummary,
    _title: &ChartTitle,
    _title_appearance: Option<&ResolvedChartStyleEntry>,
) -> Option<ChartTitleText> {
    let _plain = chart.title.as_deref()?;
    // TODO(#28 follow-up): when `ChartSummary.formatting` is ported, read
    // `formatting.entries.title.textBody` and build paragraphs/runs like upstream
    // `chartTitleText` (including per-run appearance via `chartAppearance`).
    None
}
