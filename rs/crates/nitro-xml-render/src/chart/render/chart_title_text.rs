// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-title-text.ts

use nitro_xml_opc::chart::{ChartText, ChartTitle};
use nitro_xml_opc::drawingml::{DrawingTextParagraph, DrawingTextRun, DrawingTextSpacing};

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

fn run_properties(run: &DrawingTextRun) -> ResolvedChartStyleEntry {
    ResolvedChartStyleEntry {
        font_size: run.size_pt,
        bold: run.bold,
        italic: run.italic,
        underline: run.underline,
        color: None,
        line_color: None,
        line_width: None,
        fill_color: None,
        labels_visible: None,
    }
}

fn title_body_text(tx: &ChartText) -> Option<&nitro_xml_opc::chart::ChartTextBody> {
    tx.rich.as_ref()
}

/// Resolve mixed title runs when the chart part carries rich text beyond a single plain line.
pub fn chart_title_text(
    chart: &ChartSummary,
    title: &ChartTitle,
    title_appearance: Option<&ResolvedChartStyleEntry>,
) -> Option<ChartTitleText> {
    let plain = chart.title.as_deref()?;
    let body = title.tx.as_ref().and_then(title_body_text)?.body.clone();
    if body.text != plain {
        return None;
    }
    if body.paragraphs.len() == 1 {
        let p = &body.paragraphs[0];
        if p.runs.len() <= 1
            && !body.text.contains('\n')
            && p.line_spacing.is_none()
            && p.space_before.is_none()
            && p.space_after.is_none()
        {
            return None;
        }
    }
    let default_appearance = title_appearance.cloned().unwrap_or_default();
    Some(ChartTitleText {
        paragraphs: body
            .paragraphs
            .iter()
            .map(|paragraph| chart_title_paragraph(paragraph, &default_appearance))
            .collect(),
    })
}

fn chart_title_paragraph(
    paragraph: &DrawingTextParagraph,
    default_appearance: &ResolvedChartStyleEntry,
) -> ChartTitleParagraph {
    ChartTitleParagraph {
        align: paragraph.align.clone(),
        line_spacing: paragraph.line_spacing.clone(),
        space_before: paragraph.space_before.clone(),
        space_after: paragraph.space_after.clone(),
        runs: paragraph
            .runs
            .iter()
            .map(|run| {
                let mut appearance = default_appearance.clone();
                let props = run_properties(run);
                if props.font_size.is_some() {
                    appearance.font_size = props.font_size;
                }
                if props.bold.is_some() {
                    appearance.bold = props.bold;
                }
                if props.italic.is_some() {
                    appearance.italic = props.italic;
                }
                if props.underline.is_some() {
                    appearance.underline = props.underline;
                }
                ChartTitleRun {
                    text: run.text.clone(),
                    appearance,
                }
            })
            .collect(),
    }
}
