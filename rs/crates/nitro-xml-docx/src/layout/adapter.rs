// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/adapter.ts; changes listed in PROVENANCE.md.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::Value;

use crate::body::DocumentModel;
use crate::chart_paint::docx_chart_paint;
use crate::fields::{date_field_result, field_name, is_field_locked};
use crate::numbering::{compute_list_labels, display_list_label};
use crate::run_formatting::{resolve_run_formatting, RunFormattingContext};
use crate::styles::resolve_paragraph_formatting;
use crate::theme_color::resolve_theme_color_reference;

use super::adapt_drop_cap::fold_drop_caps;
use super::adapt_floats::floats_of;
use super::adapt_notes::{endnote_paragraphs, note_labels, paragraph_footnotes};
use super::adapt_paragraph_box::{group_paragraph_borders, paragraph_box};
use super::adapt_table::adapt_table;
use super::input::{
    block_to_value, LayoutBlock, LayoutDocumentInput, LayoutParagraph, LayoutRun, LayoutSectionSerde,
    LayoutTabStop,
};
use super::review_display::{review_source_length, LayoutReviewOptions};
use super::units::twips_to_px;
use crate::review_formatting_display::{is_run_hidden_for_review, ReviewDisplayMode};

const TABLE_ROW_NOTE: &str =
    "Table rows with an exact height clip content that does not fit; text in them is not shrunk.";
const VERTICAL_MERGE_NOTE: &str =
    "Vertically merged table cells are drawn as one cell, but their text stays in the first row of the merge.";
const DROP_CAP_NOTE: &str =
    "Drop caps are laid out as an enlarged first letter (a raised cap), not as a dropped cap.";
const TEXT_BOX_NOTE: &str =
    "Text boxes are shown on the editing surface but are not drawn in Print Layout.";
const EQUATION_NOTE: &str =
    "Equations display on the editing surface; Print Layout shows markers rather than typeset equations.";
const NEXT_COLUMN_NOTE: &str = "A \"next column\" section break is laid out as a continuous section break.";

fn layout_tab_stops(
    paragraph: &Value,
    resolved: &Value,
    label: Option<&Value>,
) -> Option<Vec<LayoutTabStop>> {
    let stops_src = resolved
        .get("tabStops")
        .and_then(|v| v.as_array())
        .or_else(|| paragraph.get("tabStops").and_then(|v| v.as_array()));
    let mut stops: Vec<LayoutTabStop> = Vec::new();
    if let Some(arr) = stops_src {
        for stop in arr {
            let pos_twips = stop
                .get("posTwips")
                .and_then(|v| v.as_i64())
                .or_else(|| stop.get("posTwips").and_then(|v| v.as_f64()).map(|f| f as i64));
            let align = stop.get("align").and_then(|v| v.as_str());
            if pos_twips.is_none() || align.is_none() {
                continue;
            }
            stops.push(LayoutTabStop {
                pos_px: twips_to_px(pos_twips.unwrap()),
                align: align.unwrap().into(),
                leader: stop
                    .get("leader")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            });
        }
    }
    if let Some(label) = label {
        if let Some(twips) = label.get("tabStopTwips").and_then(|v| v.as_i64()) {
            stops.push(LayoutTabStop {
                pos_px: twips_to_px(twips),
                align: "left".into(),
                leader: None,
            });
        }
    }
    if stops.is_empty() {
        None
    } else {
        stops.sort_by(|a, b| a.pos_px.partial_cmp(&b.pos_px).unwrap_or(std::cmp::Ordering::Equal));
        Some(stops)
    }
}

pub fn adapt_document_model(
    model: &DocumentModel,
    note: &mut dyn FnMut(String),
    options: &LayoutReviewOptions,
) -> LayoutDocumentInput {
    let review_mode = options
        .review_display_mode
        .unwrap_or(ReviewDisplayMode::All);
    let catalog = model.paragraph_styles.clone();
    let mut noted = HashSet::new();
    let mut report = |message: String| {
        if noted.insert(message.clone()) {
            note(message);
        }
    };
    let now = (2026, 9, 7, 14, 5);
    let theme = model.theme.as_ref();
    let paragraph_props: Vec<crate::styles::ParagraphProps> = model
        .blocks
        .iter()
        .filter(|b| b.get("type").and_then(|v| v.as_str()) == Some("paragraph"))
        .map(|b| {
            crate::styles::ParagraphProps {
                id: b.get("id").and_then(|v| v.as_str()).unwrap_or("").into(),
                style: b.get("style").and_then(|v| v.as_str()).map(str::to_string),
                numbering: None,
                direct: b.clone(),
            }
        })
        .collect();
    let labels = compute_list_labels(
        &paragraph_props,
        catalog.as_ref(),
        model.numbering_catalog.as_ref(),
    );
    let note_label = note_labels(model);
    let between_borders = HashMap::new();
    let mut blocks: Vec<LayoutBlock> = model
        .blocks
        .iter()
        .map(|block| adapt_block(block, model, &catalog, theme, &labels, &note_label, review_mode, &mut report, now))
        .collect();
    let mut adapt_endnote = |p: &Value, mark: Option<&str>| {
        adapt_paragraph_only(
            p,
            mark,
            model,
            &catalog,
            theme,
            &labels,
            &note_label,
            review_mode,
            &mut report,
            now,
        )
    };
    blocks.extend(
        endnote_paragraphs(model, &note_label, &mut adapt_endnote)
            .into_iter()
            .map(LayoutBlock::Paragraph),
    );
    group_paragraph_borders(&mut blocks, &between_borders);
    let folded = fold_drop_caps(&model.blocks, &mut blocks);
    if !folded.is_empty() {
        report(DROP_CAP_NOTE.into());
    }
    let sections = model.sections.clone();
    if sections.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
        let page = &model.page;
        return LayoutDocumentInput {
            sections: vec![LayoutSectionSerde {
                page: super::input::LayoutPageGeometry {
                    width_px: page.get("width").and_then(|v| v.as_f64()).unwrap_or(612.0),
                    height_px: page.get("height").and_then(|v| v.as_f64()).unwrap_or(792.0),
                    margin_top_px: page.get("marginTop").and_then(|v| v.as_f64()).unwrap_or(72.0),
                    margin_right_px: page.get("marginRight").and_then(|v| v.as_f64()).unwrap_or(72.0),
                    margin_bottom_px: page.get("marginBottom").and_then(|v| v.as_f64()).unwrap_or(72.0),
                    margin_left_px: page.get("marginLeft").and_then(|v| v.as_f64()).unwrap_or(72.0),
                },
                columns: None,
                blocks: blocks
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !folded.contains(i))
                    .map(|(_, b)| block_to_value(b))
                    .collect(),
                section_break: None,
                vertical_align: None,
            }],
        };
    }
    let sections = sections.unwrap();
    let index_by_id: HashMap<String, usize> = model
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(i, b)| b.get("id").and_then(|v| v.as_str()).map(|id| (id.to_string(), i)))
        .collect();
    let mut start = 0usize;
    let mut result = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        let last = index == sections.len() - 1;
        let end_index = if last {
            blocks.len()
        } else {
            let ends = section.get("endsAtBlockId").and_then(|v| v.as_str()).unwrap_or("");
            (index_by_id.get(ends).copied().unwrap_or(start.saturating_sub(1))) + 1
        };
        let end = end_index.max(start);
        let slice = blocks[start..end].to_vec();
        start = end;
        let section_start = if index == 0 {
            None
        } else {
            section_break(section.get("type").and_then(|v| v.as_str()), &mut report)
        };
        let page_width = section.get("pageWidthTwips").and_then(|v| v.as_f64()).unwrap_or(12240.0);
        let page_height = section.get("pageHeightTwips").and_then(|v| v.as_f64()).unwrap_or(15840.0);
        let columns = section.get("columns");
        let col_count = columns.and_then(|c| c.get("count")).and_then(|v| v.as_u64()).unwrap_or(1) as usize;
        result.push(LayoutSectionSerde {
            page: super::input::LayoutPageGeometry {
                width_px: twips_to_px(page_width as i64),
                height_px: twips_to_px(page_height as i64),
                margin_top_px: twips_to_px(section.get("marginTopTwips").and_then(|v| v.as_i64()).unwrap_or(1440)),
                margin_right_px: twips_to_px(section.get("marginRightTwips").and_then(|v| v.as_i64()).unwrap_or(1440)),
                margin_bottom_px: twips_to_px(section.get("marginBottomTwips").and_then(|v| v.as_i64()).unwrap_or(1440)),
                margin_left_px: twips_to_px(section.get("marginLeftTwips").and_then(|v| v.as_i64()).unwrap_or(1440)),
            },
            columns: if col_count > 1 {
                Some(super::input::LayoutColumns {
                    count: col_count,
                    gap_px: twips_to_px(
                        columns
                            .and_then(|c| c.get("spacingTwips"))
                            .and_then(|v| v.as_i64())
                            .unwrap_or(720),
                    ),
                    separator: columns.and_then(|c| c.get("separator")).and_then(|v| v.as_bool()),
                    widths: None,
                })
            } else {
                None
            },
            section_break: section_start,
            vertical_align: section
                .get("verticalAlign")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            blocks: slice
                .iter()
                .enumerate()
                .filter_map(|(offset, block)| {
                    if folded.contains(&(start - slice.len() + offset)) {
                        return None;
                    }
                    Some(block_to_value(block))
                })
                .collect(),
        });
    }
    LayoutDocumentInput { sections: result }
}

fn section_break(section_type: Option<&str>, note: &mut dyn FnMut(String)) -> Option<String> {
    match section_type {
        Some("nextColumn") => {
            note(NEXT_COLUMN_NOTE.into());
            Some("continuous".into())
        }
        Some(other) => Some(other.into()),
        None => None,
    }
}

fn adapt_block(
    block: &Value,
    model: &DocumentModel,
    catalog: &Option<Value>,
    theme: Option<&Value>,
    labels: &BTreeMap<String, Value>,
    note_label: &dyn Fn(&str, &str) -> String,
    review_mode: ReviewDisplayMode,
    report: &mut dyn FnMut(String),
    now: (i32, u32, u32, u32, u32),
) -> LayoutBlock {
    if block.get("type").and_then(|v| v.as_str()) == Some("paragraph") {
        LayoutBlock::Paragraph(adapt_paragraph_only(
            block,
            None,
            model,
            catalog,
            theme,
            labels,
            note_label,
            review_mode,
            report,
            now,
        ))
    } else {
        report(TABLE_ROW_NOTE.into());
        report(VERTICAL_MERGE_NOTE.into());
        let mut adapt_cell = |p: &Value| {
            adapt_paragraph_only(p, None, model, catalog, theme, labels, note_label, review_mode, report, now)
        };
        LayoutBlock::Table(adapt_table(block, model, &mut adapt_cell))
    }
}

fn adapt_paragraph_only(
    paragraph: &Value,
    mark_label: Option<&str>,
    model: &DocumentModel,
    catalog: &Option<Value>,
    theme: Option<&Value>,
    labels: &BTreeMap<String, Value>,
    note_label: &dyn Fn(&str, &str) -> String,
    review_mode: ReviewDisplayMode,
    report: &mut dyn FnMut(String),
    now: (i32, u32, u32, u32, u32),
) -> LayoutParagraph {
    let paragraph_style = paragraph.get("style").and_then(|v| v.as_str());
    let resolved = catalog
        .as_ref()
        .map(|c| resolve_paragraph_formatting(
            &crate::styles::ParagraphProps {
                id: paragraph.get("id").and_then(|v| v.as_str()).unwrap_or("").into(),
                style: paragraph_style.map(str::to_string),
                numbering: None,
                direct: paragraph.clone(),
            },
            c,
        ))
        .unwrap_or(paragraph.clone());
    let label = paragraph
        .get("id")
        .and_then(|v| v.as_str())
        .and_then(|id| labels.get(id));
    let runs = paragraph
        .get("runs")
        .and_then(|v| v.as_array())
        .map(|runs| {
            runs.iter()
                .map(|run| adapt_run(run, paragraph_style, mark_label, model, theme, review_mode, note_label, report, now))
                .collect::<Vec<LayoutRun>>()
        })
        .unwrap_or_default();
    let layout_runs = if let Some(label) = label {
        let mut v = vec![label_run(paragraph, label, &runs)];
        v.extend(runs);
        v
    } else {
        runs
    };
    let mut adapt_footnote = |p: &Value, mark: Option<&str>| {
        adapt_paragraph_only(p, mark, model, catalog, theme, labels, note_label, review_mode, report, now)
    };
    let footnotes = paragraph_footnotes(paragraph, model, note_label, &mut adapt_footnote);
    let box_result = paragraph_box(&resolved, theme);
    let tab_stops = layout_tab_stops(paragraph, &resolved, label);
    LayoutParagraph {
        after_table_section_break: None,
        kind: "paragraph".into(),
        id: paragraph.get("id").and_then(|v| v.as_str()).unwrap_or("").into(),
        runs: layout_runs,
        align: resolved.get("align").and_then(|v| v.as_str()).map(str::to_string),
        direction: resolved.get("direction").and_then(|v| v.as_str()).map(str::to_string),
        spacing_before_twips: resolved.get("spacingBeforeTwips").and_then(|v| v.as_i64()),
        spacing_after_twips: resolved.get("spacingAfterTwips").and_then(|v| v.as_i64()),
        line_spacing_twips: resolved.get("lineSpacingTwips").and_then(|v| v.as_i64()),
        line_spacing_rule: resolved.get("lineSpacingRule").and_then(|v| v.as_str()).map(str::to_string),
        indent_left_twips: resolved.get("indentLeftTwips").and_then(|v| v.as_i64()),
        indent_right_twips: resolved.get("indentRightTwips").and_then(|v| v.as_i64()),
        indent_start_twips: resolved.get("indentStartTwips").and_then(|v| v.as_i64()),
        indent_end_twips: resolved.get("indentEndTwips").and_then(|v| v.as_i64()),
        first_line_twips: resolved.get("firstLineTwips").and_then(|v| v.as_i64()),
        hanging_twips: resolved.get("hangingTwips").and_then(|v| v.as_i64()),
        contextual_spacing: resolved.get("contextualSpacing").and_then(|v| v.as_bool()),
        style_id: paragraph_style.map(str::to_string),
        page_break_before: resolved.get("pageBreakBefore").and_then(|v| v.as_bool()),
        keep_next: resolved.get("keepNext").and_then(|v| v.as_bool()),
        keep_lines: resolved.get("keepLines").and_then(|v| v.as_bool()),
        widow_control: resolved.get("widowControl").and_then(|v| v.as_bool()),
        floats: floats_of(paragraph),
        tab_stops,
        footnotes: if footnotes.is_empty() { None } else { Some(footnotes) },
        borders: box_result.borders,
        shading: box_result.shading,
    }
}

fn label_run(_paragraph: &Value, label: &Value, runs: &[LayoutRun]) -> LayoutRun {
    let first = runs.first().cloned().unwrap_or(LayoutRun {
        text: String::new(),
        hidden: None,
        source_length: None,
        bold: None,
        italic: None,
        font_family: None,
        font_size_pt: None,
        text_scale_percent: None,
        ligatures: None,
        character_spacing_px: None,
        kerning_threshold_pt: None,
        position_px: None,
        break_after: None,
        object: None,
        synthetic: None,
        marker: None,
        script: None,
        color: None,
        underline: None,
        strike: None,
    });
    let text_label = label.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let suffix = match label.get("suffix").and_then(|v| v.as_str()) {
        Some("tab") => "\t",
        Some("space") => " ",
        _ => "",
    };
    let displayed = display_list_label(text_label);
    LayoutRun {
        text: format!("{}{}", displayed, suffix),
        synthetic: Some(true),
        marker: Some(super::input::ListMarker {
            length: displayed.len(),
            alignment: label
                .get("alignment")
                .and_then(|v| v.as_str())
                .unwrap_or("left")
                .into(),
        }),
        bold: first.bold,
        italic: first.italic,
        font_family: first.font_family.clone(),
        font_size_pt: first.font_size_pt,
        color: first.color.clone(),
        hidden: None,
        source_length: None,
        text_scale_percent: None,
        ligatures: None,
        character_spacing_px: None,
        kerning_threshold_pt: None,
        position_px: None,
        break_after: None,
        object: None,
        script: None,
        underline: None,
        strike: None,
    }
}

fn adapt_run(
    run: &Value,
    paragraph_style: Option<&str>,
    mark_label: Option<&str>,
    model: &DocumentModel,
    theme: Option<&Value>,
    review_mode: ReviewDisplayMode,
    note_label: &dyn Fn(&str, &str) -> String,
    report: &mut dyn FnMut(String),
    now: (i32, u32, u32, u32, u32),
) -> LayoutRun {
    if is_run_hidden_for_review(run, review_mode) {
        return LayoutRun {
            text: run.get("text").and_then(|v| v.as_str()).unwrap_or("").into(),
            hidden: Some(true),
            source_length: Some(review_source_length(run)),
            bold: None,
            italic: None,
            font_family: None,
            font_size_pt: None,
            text_scale_percent: None,
            ligatures: None,
            character_spacing_px: None,
            kerning_threshold_pt: None,
            position_px: None,
            break_after: None,
            object: None,
            synthetic: None,
            marker: None,
            script: None,
            color: None,
            underline: None,
            strike: None,
        };
    }
    let formatting = resolve_run_formatting(
        run,
        RunFormattingContext {
            paragraph_style_id: paragraph_style.map(str::to_string),
            paragraph_catalog: model.paragraph_styles.clone(),
            run_catalog: model.character_styles.clone(),
        },
    );
    if run.get("equation").is_some() {
        report(EQUATION_NOTE.into());
    }
    let name = run
        .get("field")
        .and_then(|f| f.get("instr"))
        .and_then(|v| v.as_str())
        .map(field_name)
        .unwrap_or_default();
    let note_text = if let Some(reference) = run.get("noteReference") {
        let kind = reference.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let id = reference.get("id").and_then(|v| v.as_str()).unwrap_or("");
        Some(note_label(kind, id))
    } else if run.get("noteMark").is_some() {
        mark_label.map(str::to_string)
    } else {
        None
    };
    let has_note_text = note_text.is_some();
    let text = note_text.unwrap_or_else(|| {
        if run.get("field").is_some() && !is_field_locked(run) && (name == "DATE" || name == "TIME") {
            date_field_result(
                &name,
                run.get("field").and_then(|f| f.get("instr")).and_then(|v| v.as_str()).unwrap_or(""),
                now.0,
                now.1,
                now.2,
                now.3,
                now.4,
            )
        } else if run.get("equation").is_some() {
            "[Equation]".into()
        } else {
            run.get("text").and_then(|v| v.as_str()).unwrap_or("").into()
        }
    });
    let script = if formatting.get("verticalAlign").and_then(|v| v.as_str()) == Some("superscript")
        || (has_note_text && formatting.get("verticalAlign").is_none())
    {
        Some("super".into())
    } else if formatting.get("verticalAlign").and_then(|v| v.as_str()) == Some("subscript") {
        Some("sub".into())
    } else {
        None
    };
    let image = run.get("image");
    if image.and_then(|i| i.get("unsupported")).and_then(|v| v.as_str()) == Some("Text box") {
        report(TEXT_BOX_NOTE.into());
    }
    let object = image
        .filter(|image| !image.get("anchored").and_then(|v| v.as_bool()).unwrap_or(false))
        .map(|image| {
            let mut obj = super::input::LayoutObject {
                part_name: image.get("partName").and_then(|v| v.as_str()).unwrap_or("").into(),
                content_type: image.get("contentType").and_then(|v| v.as_str()).unwrap_or("").into(),
                width_px: image.get("widthPx").and_then(|v| v.as_f64()).unwrap_or(0.0),
                height_px: image.get("heightPx").and_then(|v| v.as_f64()).unwrap_or(0.0),
                chart: None,
            };
            if image.get("chart").is_some() {
                obj.chart = Some(
                    serde_json::from_value(docx_chart_paint(
                        image.get("chart").unwrap(),
                        obj.width_px,
                        obj.height_px,
                        theme,
                    ))
                    .unwrap_or(super::input::LayoutChart {
                        label: "Chart".into(),
                        svg: None,
                    }),
                );
            }
            obj
        });
    let family = formatting
        .get("fontFamily")
        .and_then(|v| v.as_str())
        .or_else(|| {
            formatting
                .get("fontTheme")
                .and_then(|ft| ft.get("ascii").or_else(|| ft.get("hAnsi")))
                .and_then(|v| v.as_str())
                .and_then(|role| theme.and_then(|t| t.get("fonts").and_then(|f| f.get(role)).and_then(|r| r.get("latin"))))
                .and_then(|v| v.as_str())
        });
    let color = formatting
        .get("color")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            formatting
                .get("colorTheme")
                .and_then(|ct| theme.and_then(|t| resolve_theme_color_reference(ct, t)))
        });
    let display_text = if image.is_some() || formatting.get("vanish").and_then(|v| v.as_bool()).unwrap_or(false) {
        String::new()
    } else if formatting.get("caps").and_then(|v| v.as_bool()).unwrap_or(false) {
        text.to_uppercase()
    } else {
        text
    };
    LayoutRun {
        text: display_text,
        hidden: None,
        source_length: None,
        bold: formatting.get("bold").and_then(|v| v.as_bool()),
        italic: formatting.get("italic").and_then(|v| v.as_bool()),
        font_family: family.map(str::to_string),
        font_size_pt: formatting.get("fontSize").and_then(|v| v.as_f64()),
        text_scale_percent: formatting.get("textScalePercent").and_then(|v| v.as_f64()),
        ligatures: formatting.get("ligatures").and_then(|v| v.as_str()).map(str::to_string),
        character_spacing_px: formatting
            .get("characterSpacingTwips")
            .and_then(|v| v.as_f64())
            .map(|t| t / 15.0),
        kerning_threshold_pt: formatting
            .get("kerningHalfPoints")
            .and_then(|v| v.as_f64())
            .map(|k| k / 2.0),
        position_px: formatting
            .get("positionHalfPoints")
            .and_then(|v| v.as_f64())
            .map(|p| p * 2.0 / 3.0),
        break_after: run.get("break").and_then(|v| v.as_str()).map(str::to_string),
        object,
        synthetic: None,
        marker: None,
        script,
        color: color.filter(|c| c.len() == 7 && c.starts_with('#')),
        underline: formatting.get("underline").and_then(|v| v.as_bool()),
        strike: formatting
            .get("strike")
            .and_then(|v| v.as_bool())
            .or_else(|| formatting.get("doubleStrike").and_then(|v| v.as_bool())),
    }
}
