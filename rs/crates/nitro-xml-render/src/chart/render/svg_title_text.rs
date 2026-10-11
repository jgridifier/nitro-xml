// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-svg-title-text.ts

use nitro_xml_opc::drawingml::DrawingTextSpacing;

use super::chart_appearance::chart_points_to_pixels;
use super::chart_title_text::{ChartTitleParagraph, ChartTitleRun};
use super::chart_view::ChartViewModel;
use super::manual_layout_resolve::{
    has_manual_layout_fields, resolve_manual_layout_rect, ChartLayoutRect,
};
use super::svg_appearance::{chart_area_rect, chart_text_attributes};
use super::svg_text_metrics::{chart_font_metrics, chart_text_width, ChartSvgMeasureOptions};
use super::svg_util::{n, text, TextAttrs};

pub struct RichTitle {
    pub markup: String,
    pub height: f64,
}

fn spacing(value: Option<&DrawingTextSpacing>, natural_height: f64, fallback: f64) -> f64 {
    match value {
        Some(v) if v.unit == "points" => chart_points_to_pixels(v.value),
        Some(v) => natural_height * v.value,
        None => fallback,
    }
}

fn attrs_for_run(model: &ChartViewModel, run: &ChartTitleRun, base_size: f64) -> TextAttrs {
    let mut merged = chart_text_attributes(model, "title", base_size);
    if let Some(size) = run.appearance.font_size {
        merged.size = Some(chart_points_to_pixels(size));
    }
    if let Some(bold) = run.appearance.bold {
        merged.weight = Some(if bold { "bold" } else { "normal" });
    }
    if let Some(italic) = run.appearance.italic {
        merged.italic = Some(italic);
    }
    if let Some(underline) = run.appearance.underline {
        merged.underline = Some(underline);
    }
    if let Some(color) = &run.appearance.color {
        merged.fill = Some(color.clone());
    }
    merged
}

fn wrap_runs(
    runs: &[ChartTitleRun],
    capacity: f64,
    model: &ChartViewModel,
    base_size: f64,
    measure: &mut ChartSvgMeasureOptions,
) -> Vec<Vec<ChartTitleRun>> {
    // Word wrap matches upstream `wrapStyledRuns` when a line exceeds capacity; hard breaks
    // from `\n` are handled before this runs.
    let line_width = runs
        .iter()
        .map(|run| chart_text_width(&run.text, &attrs_for_run(model, run, base_size), measure))
        .sum::<f64>();
    if line_width <= capacity {
        return vec![runs.to_vec()];
    }

    let mut lines: Vec<Vec<ChartTitleRun>> = Vec::new();
    let mut current: Vec<ChartTitleRun> = Vec::new();
    let mut current_width = 0.0;

    let flush =
        |current: &mut Vec<ChartTitleRun>, lines: &mut Vec<Vec<ChartTitleRun>>, width: &mut f64| {
            if !current.is_empty() {
                lines.push(current.clone());
                current.clear();
                *width = 0.0;
            }
        };

    for run in runs {
        let words: Vec<&str> = run.text.split_whitespace().collect();
        for (wi, word) in words.iter().enumerate() {
            let prefix = if wi > 0 || !current.is_empty() {
                " "
            } else {
                ""
            };
            let token = format!("{}{}", prefix, word);
            let w = chart_text_width(&token, &attrs_for_run(model, run, base_size), measure);
            if !current.is_empty() && current_width + w > capacity {
                flush(&mut current, &mut lines, &mut current_width);
                let token = word.to_string();
                let w = chart_text_width(&token, &attrs_for_run(model, run, base_size), measure);
                current.push(ChartTitleRun {
                    text: token,
                    appearance: run.appearance.clone(),
                });
                current_width = w;
            } else {
                current.push(ChartTitleRun {
                    text: token,
                    appearance: run.appearance.clone(),
                });
                current_width += w;
            }
        }
    }
    flush(&mut current, &mut lines, &mut current_width);
    if lines.is_empty() {
        lines.push(vec![ChartTitleRun {
            text: String::new(),
            appearance: runs
                .first()
                .map(|r| r.appearance.clone())
                .unwrap_or_default(),
        }]);
    }
    lines
}

pub fn chart_rich_title_svg(
    model: &ChartViewModel,
    width: u32,
    measure: &mut ChartSvgMeasureOptions,
    frame_height: u32,
) -> Option<RichTitle> {
    let title = model.title.as_deref()?;
    let base = chart_text_attributes(model, "title", 14.0);
    let capacity = (width as f64 * 0.8 - 8.0).max(1.0);
    if model.title_text.is_none()
        && !has_manual_layout_fields(model.title_layout.as_ref())
        && !model.title_overlay
        && !title.contains('\n')
        && chart_text_width(title, &base, measure) <= capacity
    {
        return None;
    }

    let paragraphs = if let Some(tt) = &model.title_text {
        tt.paragraphs.clone()
    } else {
        vec![ChartTitleParagraph {
            align: None,
            line_spacing: None,
            space_before: None,
            space_after: None,
            runs: vec![ChartTitleRun {
                text: title.to_string(),
                appearance: model
                    .appearance
                    .as_ref()
                    .and_then(|a| a.get("title"))
                    .cloned()
                    .unwrap_or_default(),
            }],
        }]
    };

    let base_size = base.size.unwrap_or(14.0);
    let mut hard_lines: Vec<ChartTitleParagraph> = Vec::new();
    for paragraph in paragraphs {
        let mut line = ChartTitleParagraph {
            align: paragraph.align.clone(),
            line_spacing: paragraph.line_spacing.clone(),
            space_before: paragraph.space_before.clone(),
            space_after: paragraph.space_after.clone(),
            runs: Vec::new(),
        };
        hard_lines.push(line.clone());
        for run in &paragraph.runs {
            for (index, piece) in run.text.lines().enumerate() {
                let piece = piece.trim();
                if index > 0 {
                    line = ChartTitleParagraph {
                        align: paragraph.align.clone(),
                        line_spacing: paragraph.line_spacing.clone(),
                        space_before: paragraph.space_before.clone(),
                        space_after: paragraph.space_after.clone(),
                        runs: Vec::new(),
                    };
                    hard_lines.push(line.clone());
                }
                if !piece.is_empty() {
                    hard_lines.last_mut().unwrap().runs.push(ChartTitleRun {
                        text: piece.to_string(),
                        appearance: run.appearance.clone(),
                    });
                }
            }
        }
    }

    let mut lines: Vec<ChartTitleParagraph> = Vec::new();
    for line in hard_lines {
        let wrapped = wrap_runs(&line.runs, capacity, model, base_size, measure);
        let wrap_len = wrapped.len();
        for (index, runs) in wrapped.into_iter().enumerate() {
            lines.push(ChartTitleParagraph {
                align: line.align.clone(),
                line_spacing: line.line_spacing.clone(),
                space_before: if index == 0 {
                    line.space_before.clone()
                } else {
                    None
                },
                space_after: if index + 1 == wrap_len {
                    line.space_after.clone()
                } else {
                    None
                },
                runs,
            });
        }
    }

    struct LineMetrics {
        runs: Vec<(ChartTitleRun, TextAttrs)>,
        width: f64,
        size: f64,
        font_metrics: Vec<Option<(f64, f64)>>,
        align: Option<String>,
        line_spacing: Option<DrawingTextSpacing>,
        space_before: Option<DrawingTextSpacing>,
        space_after: Option<DrawingTextSpacing>,
    }

    let metrics: Vec<LineMetrics> = lines
        .iter()
        .map(|line| {
            let runs: Vec<(ChartTitleRun, TextAttrs)> = line
                .runs
                .iter()
                .map(|run| {
                    let attrs = attrs_for_run(model, run, base_size);
                    (run.clone(), attrs)
                })
                .collect();
            let width = runs
                .iter()
                .map(|(run, attrs)| chart_text_width(&run.text, attrs, measure))
                .sum();
            let size = runs
                .iter()
                .map(|(_, attrs)| attrs.size.unwrap_or(14.0))
                .fold(0.0, f64::max)
                .max(base_size);
            let font_metrics = runs
                .iter()
                .map(|(_, attrs)| chart_font_metrics(attrs, measure))
                .collect();
            LineMetrics {
                runs,
                width,
                size,
                font_metrics,
                align: line.align.clone(),
                line_spacing: line.line_spacing.clone(),
                space_before: line.space_before.clone(),
                space_after: line.space_after.clone(),
            }
        })
        .collect();

    let measured = metrics
        .iter()
        .all(|line| line.font_metrics.iter().all(|f| f.is_some()));
    let vertical: Vec<(f64, f64, f64, f64, f64, f64)> = metrics
        .iter()
        .map(|line| {
            let ascent = if measured {
                line.font_metrics
                    .iter()
                    .filter_map(|f| f.map(|(a, _)| a))
                    .fold(0.0, f64::max)
            } else {
                line.size
            };
            let descent = if measured {
                line.font_metrics
                    .iter()
                    .filter_map(|f| f.map(|(_, d)| d))
                    .fold(0.0, f64::max)
            } else {
                line.size * 0.4 + 4.0
            };
            let natural = ascent + descent;
            let height = spacing(line.line_spacing.as_ref(), natural, natural);
            let leading = (height - natural) / 2.0;
            (
                ascent + leading,
                descent + leading,
                height,
                spacing(line.space_before.as_ref(), natural, 0.0),
                spacing(line.space_after.as_ref(), natural, 0.0),
                leading,
            )
        })
        .collect();

    let box_width =
        (width as f64 - 16.0).min(metrics.iter().map(|l| l.width).fold(0.0, f64::max) + 8.0);
    let height = vertical
        .iter()
        .map(|(_, _, height, before, after, _)| height + before + after)
        .sum::<f64>()
        + if measured { 4.0 } else { 0.0 };
    let mut baseline = 8.0
        + (metrics.first().map(|l| l.size).unwrap_or(base_size))
        + vertical
            .first()
            .map(|(_, _, _, before, _, leading)| before + leading)
            .unwrap_or(0.0);

    let mut spans: Vec<String> = Vec::new();
    for (index, line) in metrics.iter().enumerate() {
        if index > 0 {
            let prev = &vertical[index - 1];
            let cur = &vertical[index];
            baseline += prev.1 + prev.4 + cur.3 + cur.0;
        }
        let mut x = match line.align.as_deref() {
            Some("l") => (width as f64 - box_width) / 2.0 + 4.0,
            Some("r") => (width as f64 + box_width) / 2.0 - 4.0 - line.width,
            _ => (width as f64 - line.width) / 2.0,
        };
        for (run, attrs) in &line.runs {
            let mut a = attrs.clone();
            a.anchor = Some("start");
            let tspan = text(x, baseline, &run.text, &a)
                .replace("<text ", "<tspan data-chart-title-run=\"true\" ")
                .replace("</text>", "</tspan>");
            spans.push(tspan);
            x += chart_text_width(&run.text, attrs, measure);
        }
    }

    let parent = text(
        0.0,
        0.0,
        "",
        &TextAttrs {
            size: base.size,
            fill: base.fill.clone(),
            weight: base.weight,
            italic: base.italic,
            underline: Some(false),
            family: base.family.clone(),
            ..Default::default()
        },
    )
    .replace("</text>", &format!("{}</text>", spans.join("")));

    let markup = chart_area_rect(
        model,
        "title",
        super::svg_util::Rect {
            x: (width as f64 - box_width) / 2.0,
            y: 4.0,
            w: box_width.max(0.0),
            h: height,
        },
    ) + &parent;

    let automatic = ChartLayoutRect {
        x: (width as f64 - box_width) / 2.0,
        y: 4.0,
        width: box_width,
        height,
    };
    let final_markup = if let Some(manual) = resolve_manual_layout_rect(
        model.title_layout.as_ref(),
        width as f64,
        frame_height as f64,
        automatic,
    ) {
        format!(
            "<g data-chart-title-layout=\"true\" transform=\"translate({} {})\">{}</g>",
            n(manual.x - automatic.x),
            n(manual.y - automatic.y),
            markup
        )
    } else {
        markup
    };

    Some(RichTitle {
        markup: final_markup,
        height: height.max(24.0),
    })
}
