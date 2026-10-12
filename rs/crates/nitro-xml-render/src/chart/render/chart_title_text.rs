// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-title-text.ts

use nitro_xml_opc::chart::ChartTitle;
use nitro_xml_opc::drawingml::{
    DrawingColor, DrawingTextParagraph, DrawingTextRun, DrawingTextRunProperties,
    DrawingTextSpacing,
};

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

fn apply_run_properties(
    entry: &mut ResolvedChartStyleEntry,
    props: &DrawingTextRunProperties,
    resolve: &dyn Fn(&DrawingColor) -> Option<String>,
) {
    if let Some(sz) = props.size_pt {
        entry.font_size = Some(sz);
    }
    if let Some(b) = props.bold {
        entry.bold = Some(b);
    }
    if let Some(i) = props.italic {
        entry.italic = Some(i);
    }
    if let Some(u) = props.underline {
        entry.underline = Some(u);
    }
    if let Some(color) = props.color.as_ref() {
        if let Some(css) = resolve(color) {
            entry.color = Some(css);
        }
    }
}

fn apply_drawing_run(
    entry: &mut ResolvedChartStyleEntry,
    run: &DrawingTextRun,
    resolve: &dyn Fn(&DrawingColor) -> Option<String>,
) {
    let props = DrawingTextRunProperties {
        size_pt: run.size_pt,
        bold: run.bold,
        italic: run.italic,
        underline: run.underline,
        color: run.color.clone(),
        typeface: run.typeface.clone(),
    };
    apply_run_properties(entry, &props, resolve);
}

fn run_appearance(
    base: &ResolvedChartStyleEntry,
    paragraph: &DrawingTextParagraph,
    run: &DrawingTextRun,
    resolve: &dyn Fn(&DrawingColor) -> Option<String>,
) -> ResolvedChartStyleEntry {
    let mut entry = base.clone();
    if let Some(def) = paragraph.default_properties.as_ref() {
        apply_run_properties(&mut entry, def, resolve);
    }
    apply_drawing_run(&mut entry, run, resolve);
    entry
}

/// Resolve imported mixed text through chart/theme precedence (`formatting.entries.title.textBody`).
///
/// When the chart title carries `c:tx/c:rich` whose flat text matches `chart.title`, build the
/// same multi-run title model upstream uses via `readChartFormatting` + `chartTitleText`.
pub fn chart_title_text(
    chart: &ChartSummary,
    title: &ChartTitle,
    title_appearance: Option<&ResolvedChartStyleEntry>,
    resolve: &dyn Fn(&DrawingColor) -> Option<String>,
) -> Option<ChartTitleText> {
    let plain = chart.title.as_deref()?;
    let rich = title
        .tx
        .as_ref()
        .and_then(|tx| tx.rich.as_ref())
        .map(|r| &r.body)?;
    if rich.text != plain {
        return None;
    }
    let first = rich.paragraphs.first()?;
    if rich.paragraphs.len() == 1
        && first.runs.len() <= 1
        && !rich.text.contains('\n')
        && first.line_spacing.is_none()
        && first.space_before.is_none()
        && first.space_after.is_none()
    {
        return None;
    }
    let base = title_appearance.cloned().unwrap_or_default();
    Some(ChartTitleText {
        paragraphs: rich
            .paragraphs
            .iter()
            .map(|paragraph| ChartTitleParagraph {
                align: paragraph.align.clone(),
                line_spacing: paragraph.line_spacing.clone(),
                space_before: paragraph.space_before.clone(),
                space_after: paragraph.space_after.clone(),
                runs: paragraph
                    .runs
                    .iter()
                    .map(|run| ChartTitleRun {
                        text: run.text.clone(),
                        appearance: run_appearance(&base, paragraph, run, resolve),
                    })
                    .collect(),
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use nitro_xml_opc::chart::parse_chart_space_xml;
    use nitro_xml_opc::drawingml::{
        resolve_drawing_color, HslRounding, ResolveOptions, TransformOrder,
    };

    use super::*;
    use crate::chart::render::chart_colors::chart_color_scheme;
    use crate::chart::render::drawing_color_css::drawing_color_css;
    use crate::chart::render::summary::ChartThemePalette;
    use crate::chart::render::summary_from_space::chart_summary_from_space;

    // Catches: multi-run chart titles collapsing to one centered string (poi-chartex chart1).
    #[test]
    fn rich_title_from_tx_rich_matches_plain_title() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/real/docx/poi-chartex.docx");
        let bytes = std::fs::read(&path).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut xml = String::new();
        {
            let mut file = archive.by_name("word/charts/chart1.xml").unwrap();
            std::io::Read::read_to_string(&mut file, &mut xml).unwrap();
        }
        let space = parse_chart_space_xml(&xml).chart_space;
        let summary = chart_summary_from_space(&space);
        let title = space.title.as_ref().unwrap();
        let theme = ChartThemePalette {
            colors: vec![],
            major_font: "Calibri Light".into(),
            minor_font: "Calibri".into(),
        };
        let scheme = chart_color_scheme(&theme);
        let resolve = |color: &DrawingColor| {
            let resolve_opts = ResolveOptions {
                transform_order: TransformOrder::Document,
                hsl_rounding: HslRounding::HalfDown,
            };
            drawing_color_css(
                resolve_drawing_color(color, Some(&scheme), Some(resolve_opts)).as_ref(),
            )
        };
        let text = chart_title_text(&summary, title, None, &resolve).unwrap();
        assert_eq!(text.paragraphs.len(), 1);
        assert_eq!(text.paragraphs[0].runs.len(), 2);
        assert_eq!(text.paragraphs[0].runs[0].text, "my chart");
        assert_eq!(text.paragraphs[0].runs[1].text, " looks nice");
    }
}
