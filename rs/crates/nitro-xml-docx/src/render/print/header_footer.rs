// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/docx/print-header-footer.ts; changes listed in PROVENANCE.md.

use std::collections::HashMap;

use serde_json::Value;

use super::dom::{Dom, StringIntern};
use super::picture::PicturePaint;
use crate::body::DocumentModel;
use crate::fields::field_name;
use crate::layout::{
    field_display_text, float_position, page_number_values, page_numbers, paragraph_floats,
    LayoutColumnBox, LayoutFloat, LayoutPageBox, PageFieldContext,
};
use crate::review_formatting_display::{is_run_hidden_for_review, ReviewDisplayMode};

use super::options::PrintLayoutOptions;

pub fn decorate_pages<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    sheets: &[u32],
    pages: &[LayoutPageBox],
    options: &PrintLayoutOptions<'_>,
) {
    let Some(model) = options.model else {
        return;
    };
    if sheets.len() != pages.len() {
        return;
    }
    let numbers = page_numbers(model, pages);
    let numeric = page_number_values(model, pages);
    let counts = section_page_counts(pages);
    for (index, page) in pages.iter().enumerate() {
        let sheet = sheets[index];
        let label = numbers
            .get(index)
            .cloned()
            .unwrap_or_else(|| (index + 1).to_string());
        let values = PageFieldContext {
            page: label,
            num_pages: pages.len().to_string(),
            section_pages: counts
                .get(&page.section_index)
                .copied()
                .unwrap_or(1)
                .to_string(),
            year: options.now.year,
            month: options.now.month,
            day: options.now.day,
            hour: options.now.hour,
            minute: options.now.minute,
        };
        let page_number = numeric.get(index).copied().unwrap_or(index as i32 + 1);
        let section = model
            .sections
            .as_ref()
            .and_then(|s| s.get(page.section_index));
        for kind in ["header", "footer"] {
            let slots_key = if kind == "header" {
                "headers"
            } else {
                "footers"
            };
            let Some(content) = header_footer_for_page(model, page, page_number, slots_key) else {
                continue;
            };
            let element = paint_header_footer(dom, sheet, &content, &values, kind, options);
            let distance = section
                .and_then(|s| {
                    let key = if kind == "header" {
                        "headerDistanceTwips"
                    } else {
                        "footerDistanceTwips"
                    };
                    s.get(key).and_then(json_f64)
                })
                .unwrap_or(720.0)
                / 15.0;
            dom.px(element, "left", page.margin_left_px);
            dom.px(element, "right", page.margin_right_px);
            if kind == "header" {
                dom.px(element, "top", distance);
            } else {
                dom.px(element, "bottom", distance);
            }
            let anchor_top = if kind == "header" {
                distance
            } else {
                page.height_px - distance - 20.0
            };
            paint_floating_pictures(dom, sheet, &content, page, anchor_top, options);
            if kind == "header" {
                if let Some(mark) = watermark_text(&content, page, options.review_mode) {
                    // Created after columns; z-index 0 keeps it behind text, matching Word.
                    paint_watermark(dom, sheet, page, &mark);
                }
            }
        }
    }
}

fn json_f64(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_i64().map(|i| i as f64))
}

fn section_page_counts(pages: &[LayoutPageBox]) -> HashMap<usize, usize> {
    let mut counts = HashMap::new();
    for page in pages {
        let indices = page
            .section_indices
            .clone()
            .unwrap_or_else(|| vec![page.section_index]);
        for index in indices {
            *counts.entry(index).or_insert(0) += 1;
        }
    }
    counts
}

fn header_footer_for_page(
    model: &DocumentModel,
    page: &LayoutPageBox,
    page_number: i32,
    kind: &str,
) -> Option<Value> {
    let sections = model.sections.as_ref()?;
    let section = sections.get(page.section_index)?;
    let title = section
        .get("titlePage")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let slot = if page.page_in_section == 0 && title {
        "first"
    } else if model.even_and_odd_headers && page_number % 2 == 0 {
        "even"
    } else {
        "default"
    };
    for index in (0..=page.section_index).rev() {
        if let Some(content) = sections
            .get(index)
            .and_then(|s| s.get(kind))
            .and_then(|slots| slots.get(slot))
        {
            if !content.is_null() {
                return Some(content.clone());
            }
        }
    }
    None
}

fn paint_header_footer<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    parent: u32,
    content: &Value,
    values: &PageFieldContext,
    kind: &str,
    options: &PrintLayoutOptions<'_>,
) -> u32 {
    let element = dom.el(parent, "div");
    dom.class(element, &format!("dve-print-{kind}"));
    dom.attr(
        element,
        "aria-label",
        if kind == "header" { "Header" } else { "Footer" },
    );
    if let Some(blocks) = content.get("blocks").and_then(|v| v.as_array()) {
        for block in blocks {
            paint_block(dom, element, block, values, options);
        }
    }
    element
}

fn paint_block<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    parent: u32,
    block: &Value,
    values: &PageFieldContext,
    options: &PrintLayoutOptions<'_>,
) {
    if block.get("type").and_then(|v| v.as_str()) == Some("paragraph") {
        paint_paragraph(dom, parent, block, values, options);
        return;
    }
    let table = dom.el(parent, "table");
    let Some(rows) = block.get("rows").and_then(|v| v.as_array()) else {
        return;
    };
    for row in rows {
        let tr = dom.el(table, "tr");
        let Some(cells) = row.as_array() else {
            continue;
        };
        for cell in cells {
            let td = dom.el(tr, "td");
            if let Some(span) = cell.get("gridSpan").and_then(|v| v.as_u64()) {
                if span > 1 {
                    dom.attr(td, "colspan", &span.to_string());
                }
            }
            if let Some(paragraphs) = cell.get("paragraphs").and_then(|v| v.as_array()) {
                for paragraph in paragraphs {
                    paint_paragraph(dom, td, paragraph, values, options);
                }
            }
        }
    }
}

fn paint_paragraph<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    parent: u32,
    paragraph: &Value,
    values: &PageFieldContext,
    options: &PrintLayoutOptions<'_>,
) {
    let element = dom.el(parent, "p");
    if let Some(align) = paragraph.get("align").and_then(|v| v.as_str()) {
        dom.style(element, "text-align", align);
    }
    if let Some(dir) = paragraph.get("direction").and_then(|v| v.as_str()) {
        dom.attr(element, "dir", dir);
    }
    let Some(runs) = paragraph.get("runs").and_then(|v| v.as_array()) else {
        return;
    };
    for run in runs {
        if is_run_hidden_for_review(run, options.review_mode) {
            continue;
        }
        if run_has_break(run) {
            continue;
        }
        let image = run.get("image");
        if image
            .and_then(|i| i.get("anchored"))
            .and_then(|v| v.as_bool())
            == Some(true)
        {
            continue;
        }
        if image.and_then(|i| i.get("watermark")).is_some() {
            continue;
        }
        paint_run(dom, element, run, values, options);
    }
}

fn paint_run<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    parent: u32,
    run: &Value,
    values: &PageFieldContext,
    options: &PrintLayoutOptions<'_>,
) {
    if let Some(image) = run.get("image") {
        paint_inline_picture(dom, parent, image, options);
        return;
    }
    let span = dom.el(parent, "span");
    let text = field_display_text(run, values);
    dom.text(span, &text);
    if let Some(instr) = run
        .get("field")
        .and_then(|f| f.get("instr"))
        .and_then(|v| v.as_str())
    {
        dom.attr(span, "data-field", &field_name(instr));
    }
    if run.get("bold").and_then(|v| v.as_bool()) == Some(true) {
        dom.style(span, "font-weight", "700");
    }
    if run.get("italic").and_then(|v| v.as_bool()) == Some(true) {
        dom.style(span, "font-style", "italic");
    }
    if run.get("underline").and_then(|v| v.as_bool()) == Some(true) {
        dom.style(span, "text-decoration", "underline");
    }
    if let Some(size) = run.get("fontSize").and_then(json_f64) {
        dom.style(span, "font-size", &format!("{size}pt"));
    }
    if let Some(family) = run.get("fontFamily").and_then(|v| v.as_str()) {
        let quoted = family.replace(['"', '\\'], "");
        dom.style(span, "font-family", &format!("\"{quoted}\""));
    }
    if let Some(color) = run.get("color").and_then(|v| v.as_str()) {
        if is_hex6(color) {
            dom.style(span, "color", color);
        }
    }
}

fn is_hex6(color: &str) -> bool {
    let bytes = color.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
}

fn paint_inline_picture<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    parent: u32,
    image: &Value,
    options: &PrintLayoutOptions<'_>,
) {
    let part = image.get("partName").and_then(|v| v.as_str()).unwrap_or("");
    let content_type = image
        .get("contentType")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let url = if part.is_empty() {
        None
    } else {
        options.picture_urls.get(part)
    };
    let tag = if url.is_some() { "img" } else { "span" };
    let el = dom.el(parent, tag);
    if let Some(url) = url {
        dom.attr(el, "src", url);
        let alt = image.get("altText").and_then(|v| v.as_str()).unwrap_or("");
        dom.attr(el, "alt", alt);
    } else {
        dom.class(el, "dve-print-picture-missing");
    }
    dom.style(el, "display", "inline-block");
    dom.style(el, "vertical-align", "bottom");
    let w = image.get("widthPx").and_then(json_f64).unwrap_or(0.0);
    let h = image.get("heightPx").and_then(json_f64).unwrap_or(0.0);
    dom.px(el, "width", w);
    dom.px(el, "height", h);
    let _ = content_type;
}

fn visible_paragraph(paragraph: &Value, mode: ReviewDisplayMode) -> Value {
    let mut copy = paragraph.clone();
    if let Some(runs) = copy.get_mut("runs").and_then(|v| v.as_array_mut()) {
        runs.retain(|run| !is_run_hidden_for_review(run, mode));
    }
    copy
}

fn paint_floating_pictures<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    sheet: u32,
    content: &Value,
    page: &LayoutPageBox,
    anchor_top: f64,
    options: &PrintLayoutOptions<'_>,
) {
    let Some(blocks) = content.get("blocks").and_then(|v| v.as_array()) else {
        return;
    };
    let mut paragraphs = Vec::new();
    for block in blocks {
        if block.get("type").and_then(|v| v.as_str()) == Some("paragraph") {
            paragraphs.push(block.clone());
        } else if let Some(rows) = block.get("rows").and_then(|v| v.as_array()) {
            for row in rows {
                let Some(cells) = row.as_array() else {
                    continue;
                };
                for cell in cells {
                    if let Some(ps) = cell.get("paragraphs").and_then(|v| v.as_array()) {
                        paragraphs.extend(ps.iter().cloned());
                    }
                }
            }
        }
    }
    let column = LayoutColumnBox {
        x_px: 0.0,
        width_px: page.width_px - page.margin_left_px - page.margin_right_px,
        blocks: Vec::new(),
        section_index: None,
        start_y_px: None,
        end_y_px: None,
        separator: None,
    };
    for paragraph in &paragraphs {
        let visible = visible_paragraph(paragraph, options.review_mode);
        let floats: Vec<LayoutFloat> = paragraph_floats(&visible);
        for float in floats {
            let (x, y) = float_position(&float, page, &column, (anchor_top, 0.0));
            let el = super::picture::paint_picture(
                dom,
                sheet,
                &PicturePaint {
                    part_name: &float.part_name,
                    content_type: &float.content_type,
                    width_px: float.width_px,
                    height_px: float.height_px,
                    chart: None,
                    alt: "",
                },
                options.picture_urls,
                options.chart_spaces,
                options.chart_svg_cache,
                options.model.and_then(|m| m.theme.as_ref()),
            );
            let mut class = String::from("dve-print-picture dve-print-float");
            if options.picture_urls.get(&float.part_name).is_none() {
                class = format!("dve-print-picture-missing {class}");
            }
            if float.behind_text == Some(true) {
                class.push_str(" dve-print-float-behind");
            }
            dom.class(el, &class);
            dom.style(el, "position", "absolute");
            dom.px(el, "left", x);
            dom.px(el, "top", y);
            dom.px(el, "width", float.width_px);
            dom.px(el, "height", float.height_px);
        }
    }
}

struct WatermarkSpec {
    text: String,
    color: String,
    semitransparent: bool,
    layout: String,
    font_family: Option<String>,
}

fn watermark_text(
    content: &Value,
    _page: &LayoutPageBox,
    mode: ReviewDisplayMode,
) -> Option<WatermarkSpec> {
    let blocks = content.get("blocks")?.as_array()?;
    for block in blocks {
        if block.get("type").and_then(|v| v.as_str()) != Some("paragraph") {
            continue;
        }
        let runs = block.get("runs")?.as_array()?;
        for run in runs {
            if is_run_hidden_for_review(run, mode) {
                continue;
            }
            let Some(spec) = run.get("image").and_then(|i| i.get("watermark")) else {
                continue;
            };
            let text = spec.get("text").and_then(|v| v.as_str()).unwrap_or("");
            if text.is_empty() {
                return None;
            }
            let color = spec
                .get("color")
                .and_then(|v| v.as_str())
                .unwrap_or("#c0c0c0");
            return Some(WatermarkSpec {
                text: text.to_string(),
                color: if is_hex6(color) {
                    color.to_string()
                } else {
                    "#c0c0c0".into()
                },
                semitransparent: spec
                    .get("semitransparent")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                layout: spec
                    .get("layout")
                    .and_then(|v| v.as_str())
                    .unwrap_or("horizontal")
                    .to_string(),
                font_family: spec
                    .get("fontFamily")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            });
        }
    }
    None
}

fn paint_watermark<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    sheet: u32,
    page: &LayoutPageBox,
    spec: &WatermarkSpec,
) {
    let chars = spec.text.chars().count().max(1) as f64;
    let span = if spec.layout == "diagonal" {
        page.width_px * 0.78
    } else {
        page.width_px * 0.7
    };
    let span = span.min(560.0);
    let font_px = (span / (chars * 0.6)).clamp(12.0, 220.0);
    let layer = dom.el(sheet, "div");
    dom.class(layer, "dve-print-watermark");
    dom.attr(layer, "aria-hidden", "true");
    dom.text(layer, &spec.text);
    dom.style(layer, "position", "absolute");
    dom.style(layer, "left", "0");
    dom.style(layer, "top", "0");
    dom.px(layer, "width", page.width_px);
    dom.px(layer, "height", page.height_px);
    dom.style(layer, "display", "flex");
    dom.style(layer, "align-items", "center");
    dom.style(layer, "justify-content", "center");
    dom.style(layer, "pointer-events", "none");
    dom.style(layer, "user-select", "none");
    dom.style(layer, "white-space", "nowrap");
    dom.style(layer, "overflow", "hidden");
    dom.style(
        layer,
        "font-size",
        &format!("{}px", super::dom::js_num(font_px)),
    );
    dom.style(layer, "font-weight", "700");
    dom.style(layer, "color", &spec.color);
    dom.style(
        layer,
        "opacity",
        if spec.semitransparent { "0.5" } else { "1" },
    );
    dom.style(
        layer,
        "transform",
        if spec.layout == "diagonal" {
            "rotate(-45deg)"
        } else {
            "none"
        },
    );
    dom.style(layer, "z-index", "0");
    if let Some(family) = &spec.font_family {
        let quoted = family.replace(['"', '\\'], "");
        dom.style(layer, "font-family", &format!("\"{quoted}\", sans-serif"));
    }
}

fn run_has_break(run: &Value) -> bool {
    match run.get("break") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}
