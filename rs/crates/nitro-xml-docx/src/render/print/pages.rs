// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/docx/print-layout.ts; changes listed in PROVENANCE.md.

use nitro_xml_render::CommandBuffer;

use super::blocks::parse_block;
use super::dom::{js_num, Dom, StringIntern};
use super::header_footer::decorate_pages;
use super::line_numbers::{paint_line_number_labels, LineCounter};
use super::options::PrintLayoutOptions;
use super::page_borders::paint_page_border_box;
use super::picture::{paint_picture, PicturePaint};
use super::styles::inject_print_styles;
use super::table::paint_table;
use crate::layout::{
    css_font_stack, ligature_css, LayoutBlockBox, LayoutFragment, LayoutLine, LayoutPageBox,
    LayoutParagraphBorder, LayoutParagraphBox, LayoutParagraphFrame, LayoutResult,
    DEFAULT_FONT_SIZE_PT,
};

const LEADER_CHARACTERS: &[(&str, &str)] = &[("dot", "."), ("hyphen", "-"), ("middleDot", "·")];

/// Paint one Print Layout frame into `buf` (caller-owned; this begins the buffer).
pub fn paint_print_layout<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    result: &LayoutResult,
    options: &PrintLayoutOptions<'_>,
) {
    buf.begin();
    let mut dom = Dom::new(buf, intern);
    inject_print_styles(&mut dom, 0);
    let container = dom.el(0, "div");
    dom.class(container, "dve-print-pages");
    let zoom = if options.zoom.is_finite() && options.zoom > 0.0 {
        options.zoom
    } else {
        1.0
    };
    dom.style(container, "--dve-zoom", &js_num(zoom));
    if let Some(color) = options.page_color {
        dom.style(container, "--dve-page-color", color);
    }
    dom.attr(
        container,
        "data-page-count",
        &result.pages.len().to_string(),
    );

    let mut counter = LineCounter::default();
    let mut sheets = Vec::with_capacity(result.pages.len());
    for (page_index, page) in result.pages.iter().enumerate() {
        let sheet = dom.el(container, "div");
        sheets.push(sheet);
        dom.class(sheet, "dve-print-page");
        dom.attr(sheet, "data-page-index", &page_index.to_string());
        if options.current_page == Some(page_index) {
            dom.attr(sheet, "data-nitro-current", "1");
        }
        dom.px(sheet, "width", page.width_px);
        dom.px(sheet, "height", page.height_px);
        if let Some(borders) = options
            .page_borders
            .get(page.section_index)
            .and_then(|b| b.as_ref())
        {
            paint_page_border_box(&mut dom, sheet, page, borders);
        }
        for (i, column) in page.columns.iter().enumerate() {
            if i > 0 {
                paint_column_rule(&mut dom, sheet, page, &page.columns[i - 1], column);
            }
            let column_el = dom.el(sheet, "div");
            dom.class(column_el, "dve-print-column");
            dom.px(column_el, "top", page.margin_top_px);
            dom.px(column_el, "left", page.margin_left_px + column.x_px);
            dom.px(column_el, "width", column.width_px);
            dom.px(
                column_el,
                "height",
                page.height_px - page.margin_top_px - page.margin_bottom_px,
            );
            for value in &column.blocks {
                let Some(block) = parse_block(value) else {
                    continue;
                };
                paint_block(&mut dom, column_el, &block, column.width_px, options);
            }
        }
        let mut bands: Vec<(usize, Vec<usize>)> = Vec::new();
        for (i, column) in page.columns.iter().enumerate() {
            let index = column.section_index.unwrap_or(page.section_index);
            if let Some((_, cols)) = bands.iter_mut().find(|(s, _)| *s == index) {
                cols.push(i);
            } else {
                bands.push((index, vec![i]));
            }
        }
        for (section_index, cols) in &bands {
            if let Some(numbering) = options
                .line_numbers
                .get(*section_index)
                .and_then(|n| n.as_ref())
            {
                let columns: Vec<_> = cols.iter().map(|i| &page.columns[*i]).collect();
                paint_line_number_labels(
                    &mut dom,
                    sheet,
                    page,
                    *section_index,
                    &columns,
                    numbering,
                    &mut counter,
                    options.suppressed_line_number_paragraphs,
                );
            }
        }
        if page.footnotes.as_ref().is_some_and(|n| !n.is_empty()) {
            paint_footnotes(&mut dom, sheet, page, options);
        }
        if let Some(floats) = &page.floats {
            for float in floats {
                let el = paint_picture(
                    &mut dom,
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
                    options.chart_color_palettes,
                    options.chart_svg_cache,
                    options.model.and_then(|m| m.theme.as_ref()),
                );
                let mut class = String::from("dve-print-picture dve-print-float");
                if options.picture_urls.get(&float.part_name).is_none() {
                    class = format!("dve-print-picture-missing {class}");
                }
                if float.behind_text {
                    class.push_str(" dve-print-float-behind");
                }
                dom.class(el, &class);
                dom.px(el, "left", float.x_px);
                dom.px(el, "top", float.y_px);
            }
        }
    }
    decorate_pages(&mut dom, &sheets, &result.pages, options);
}

fn paint_column_rule<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    sheet: u32,
    page: &LayoutPageBox,
    previous: &crate::layout::LayoutColumnBox,
    column: &crate::layout::LayoutColumnBox,
) {
    let sep = column.separator.or(page.column_separator).unwrap_or(false);
    if !sep {
        return;
    }
    let prev_sec = previous.section_index.unwrap_or(page.section_index);
    let col_sec = column.section_index.unwrap_or(page.section_index);
    if prev_sec != col_sec {
        return;
    }
    let rule = dom.el(sheet, "div");
    dom.class(rule, "dve-print-column-rule");
    dom.style(rule, "position", "absolute");
    dom.px(
        rule,
        "left",
        page.margin_left_px + (previous.x_px + previous.width_px + column.x_px) / 2.0,
    );
    let start = column.start_y_px.unwrap_or(0.0);
    dom.px(rule, "top", page.margin_top_px + start);
    let end = column
        .end_y_px
        .unwrap_or(page.height_px - page.margin_top_px - page.margin_bottom_px);
    dom.px(rule, "height", end - start);
    dom.style(rule, "border-left", "1px solid currentColor");
}

fn paint_block<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    parent: u32,
    block: &LayoutBlockBox,
    column_width_px: f64,
    options: &PrintLayoutOptions<'_>,
) -> u32 {
    match block {
        LayoutBlockBox::Paragraph(paragraph) => paint_paragraph(dom, parent, paragraph, options),
        LayoutBlockBox::Table(table) => paint_table(
            dom,
            parent,
            table,
            column_width_px,
            |dom, parent, paragraph| {
                paint_paragraph(dom, parent, paragraph, options);
            },
        ),
    }
}

fn paint_paragraph<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    parent: u32,
    paragraph: &LayoutParagraphBox,
    options: &PrintLayoutOptions<'_>,
) -> u32 {
    let el = dom.el(parent, "div");
    dom.class(el, "dve-print-block");
    dom.px(el, "top", paragraph.y_px);
    dom.px(el, "height", paragraph.height_px);
    if let Some(frame) = &paragraph.frame {
        paint_paragraph_frame(dom, el, frame, paragraph.height_px);
    }
    for line in &paragraph.lines {
        paint_line(dom, el, line, options);
    }
    el
}

fn paint_paragraph_frame<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    parent: u32,
    frame: &LayoutParagraphFrame,
    height_px: f64,
) {
    let el = dom.el(parent, "div");
    dom.class(el, "dve-print-paragraph-frame");
    dom.px(el, "left", frame.left_px);
    dom.px(el, "width", frame.width_px);
    dom.px(el, "height", height_px);
    if let Some(shading) = &frame.shading {
        dom.style(el, "background", shading);
    }
    if let Some(borders) = &frame.borders {
        side(dom, el, "top", borders.top.as_ref());
        side(dom, el, "right", borders.right.as_ref());
        side(dom, el, "bottom", borders.bottom.as_ref());
        side(dom, el, "left", borders.left.as_ref());
    }
}

fn side<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    el: u32,
    name: &str,
    line: Option<&LayoutParagraphBorder>,
) {
    let Some(line) = line else { return };
    let prop = format!("border-{name}");
    let val = format!("{}px {} {}", js_num(line.width_px), line.style, line.color);
    dom.style(el, &prop, &val);
}

fn paint_line<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    parent: u32,
    line: &LayoutLine,
    options: &PrintLayoutOptions<'_>,
) {
    let line_el = dom.el(parent, "div");
    dom.class(line_el, "dve-print-line");
    dom.px(line_el, "top", line.y_px);
    dom.px(line_el, "height", line.height_px);
    for fragment in &line.fragments {
        if let Some(object) = &fragment.object {
            let picture = paint_picture(
                dom,
                line_el,
                &PicturePaint {
                    part_name: &object.part_name,
                    content_type: &object.content_type,
                    width_px: object.width_px,
                    height_px: object.height_px,
                    chart: object.chart.as_ref(),
                    alt: "",
                },
                options.picture_urls,
                options.chart_spaces,
                options.chart_color_palettes,
                options.chart_svg_cache,
                options.model.and_then(|m| m.theme.as_ref()),
            );
            dom.px(picture, "left", fragment.x_px);
            dom.px(picture, "top", fragment.top_px.unwrap_or(0.0));
            continue;
        }
        if fragment.leader.is_some() {
            paint_leader(dom, line_el, fragment);
            continue;
        }
        if fragment.text.is_empty() && fragment.width_px == 0.0 {
            continue;
        }
        let span = dom.el(line_el, "span");
        style_fragment(dom, span, fragment);
    }
}

fn paint_leader<I: StringIntern>(dom: &mut Dom<'_, I>, parent: u32, fragment: &LayoutFragment) {
    let el = dom.el(parent, "span");
    let mut blank = fragment.clone();
    blank.text.clear();
    style_fragment(dom, el, &blank);
    dom.class(el, "dve-print-leader");
    let scale = fragment.text_scale_percent.unwrap_or(100.0) / 100.0;
    let width = if scale == 0.0 {
        fragment.width_px
    } else {
        fragment.width_px / scale
    };
    dom.px(el, "width", width);
    dom.style(el, "overflow", "hidden");
    dom.style(el, "text-align", "right");
    let leader = fragment.leader.as_deref().unwrap_or("");
    let character = LEADER_CHARACTERS
        .iter()
        .find(|(k, _)| *k == leader)
        .map(|(_, c)| *c);
    if let Some(character) = character {
        let size_px = fragment.font_size_pt.unwrap_or(DEFAULT_FONT_SIZE_PT) * 96.0 / 72.0;
        let denom = (if scale == 0.0 { 1.0 } else { scale }) * size_px * 0.25;
        let count = if denom == 0.0 {
            1
        } else {
            (fragment.width_px / denom).ceil() as usize + 1
        };
        dom.text(el, &character.repeat(count));
        dom.style(el, "direction", "rtl");
    } else {
        dom.style(el, "height", "1em");
        let px = if leader == "heavy" { 2 } else { 1 };
        dom.style(el, "border-bottom", &format!("{px}px solid currentColor"));
    }
}

fn style_fragment<I: StringIntern>(dom: &mut Dom<'_, I>, el: u32, fragment: &LayoutFragment) {
    dom.style(el, "position", "absolute");
    dom.px(el, "left", fragment.x_px);
    dom.style(el, "white-space", "pre");
    if fragment.bold == Some(true) {
        dom.style(el, "font-weight", "700");
    }
    if fragment.italic == Some(true) {
        dom.style(el, "font-style", "italic");
    }
    dom.style(
        el,
        "font-family",
        &css_font_stack(fragment.font_family.as_deref()),
    );
    let size = fragment.font_size_pt.unwrap_or(DEFAULT_FONT_SIZE_PT)
        * if fragment.script.is_some() { 0.65 } else { 1.0 };
    dom.style(el, "font-size", &format!("{}pt", js_num(size)));
    let scale = fragment.text_scale_percent.unwrap_or(100.0) / 100.0;
    if fragment.text.is_empty() {
        let width = if scale == 0.0 {
            fragment.width_px
        } else {
            fragment.width_px / scale
        };
        dom.px(el, "width", width);
    }
    if scale != 1.0 {
        dom.style(el, "transform", &format!("scaleX({})", js_num(scale)));
        dom.style(el, "transform-origin", "left center");
    }
    if let Some(spacing) = fragment.character_spacing_px {
        let px = if scale == 0.0 { 0.0 } else { spacing / scale };
        dom.px(el, "letter-spacing", px);
    }
    if let Some(threshold) = fragment.kerning_threshold_pt {
        let size_pt = fragment.font_size_pt.unwrap_or(DEFAULT_FONT_SIZE_PT);
        let kern = if threshold > 0.0 && size_pt >= threshold {
            "normal"
        } else {
            "none"
        };
        dom.style(el, "font-kerning", kern);
    }
    if let Some(top) = fragment.top_px {
        dom.px(el, "top", top);
    }
    if let Some(h) = fragment.box_height_px {
        dom.px(el, "line-height", h);
    }
    if let Some(color) = &fragment.color {
        dom.style(el, "color", color);
    }
    if let Some(ligatures) = &fragment.ligatures {
        dom.style(el, "font-variant-ligatures", &ligature_css(ligatures));
    }
    let mut lines: Vec<&str> = Vec::new();
    if fragment.underline == Some(true) {
        lines.push("underline");
    }
    if fragment.strike == Some(true) {
        lines.push("line-through");
    }
    if !lines.is_empty() {
        dom.style(el, "text-decoration-line", &lines.join(" "));
    }
    if let Some(css) = fragment.highlight.as_deref().and_then(highlight_css) {
        dom.style(el, "background-color", css);
    }
    dom.text(el, &fragment.text);
}

fn highlight_css(name: &str) -> Option<&'static str> {
    Some(match name {
        "yellow" => "#FFFF00",
        "green" => "#00FF00",
        "cyan" => "#00FFFF",
        "magenta" => "#FF00FF",
        "blue" => "#0000FF",
        "red" => "#FF0000",
        "darkBlue" => "#000080",
        "darkCyan" => "#008080",
        "darkGreen" => "#008000",
        "darkMagenta" => "#800080",
        "darkRed" => "#800000",
        "darkYellow" => "#808000",
        "darkGray" => "#808080",
        "lightGray" => "#C0C0C0",
        "black" => "#000000",
        "white" => "#FFFFFF",
        _ => return None,
    })
}

fn paint_footnotes<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    sheet: u32,
    page: &LayoutPageBox,
    options: &PrintLayoutOptions<'_>,
) {
    let notes = page.footnotes.as_deref().unwrap_or(&[]);
    let height_px: f64 = notes.iter().map(|n| n.height_px).sum();
    let width_px = page.width_px - page.margin_left_px - page.margin_right_px;
    let area = dom.el(sheet, "div");
    dom.class(area, "dve-print-footnotes");
    dom.attr(area, "aria-label", "Footnotes");
    dom.px(area, "left", page.margin_left_px);
    dom.px(area, "width", width_px);
    dom.px(
        area,
        "top",
        page.height_px - page.margin_bottom_px - height_px,
    );
    dom.px(area, "height", height_px);
    let separator = dom.el(area, "div");
    dom.class(separator, "dve-print-footnote-separator");
    for note in notes {
        let note_el = dom.el(area, "div");
        dom.class(note_el, "dve-print-footnote");
        dom.attr(note_el, "data-note-id", &note.id);
        dom.px(note_el, "top", note.y_px);
        dom.px(note_el, "height", note.height_px);
        for paragraph in &note.paragraphs {
            paint_paragraph(dom, note_el, paragraph, options);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::dom::StringIntern;
    use super::super::options::{PrintLayoutOptions, PrintNow};
    use super::*;
    use crate::review_formatting_display::ReviewDisplayMode;
    use serde_json::json;
    use std::collections::{HashMap, HashSet};

    struct Rec(Vec<String>);
    impl StringIntern for Rec {
        fn intern(&mut self, s: &str) -> u32 {
            if let Some(i) = self.0.iter().position(|x| x == s) {
                return i as u32;
            }
            self.0.push(s.to_string());
            (self.0.len() - 1) as u32
        }
    }

    #[test]
    fn column_block_json_paints_fragment_text() {
        // Catches: column.blocks `kind` tag failing to deserialize into LayoutBlockBox, so body text never reaches the frame.
        let block = json!({
            "kind": "paragraph",
            "blockId": "p1",
            "yPx": 0.0,
            "heightPx": 20.0,
            "lines": [{
                "yPx": 0.0,
                "heightPx": 20.0,
                "fragments": [{ "text": "Hi", "xPx": 1.0, "widthPx": 12.0, "runIndex": 0 }],
                "sourceStart": 0,
                "sourceEnd": 2
            }]
        });
        let result = LayoutResult {
            pages: vec![LayoutPageBox {
                index: 0,
                section_index: 0,
                section_indices: None,
                page_in_section: 0,
                width_px: 100.0,
                height_px: 100.0,
                margin_top_px: 10.0,
                margin_right_px: 10.0,
                margin_bottom_px: 10.0,
                margin_left_px: 10.0,
                columns: vec![crate::layout::LayoutColumnBox {
                    x_px: 0.0,
                    width_px: 80.0,
                    blocks: vec![block],
                    section_index: None,
                    start_y_px: None,
                    end_y_px: None,
                    separator: None,
                }],
                column_separator: None,
                floats: None,
                footnotes: None,
            }],
            approximations: Vec::new(),
        };
        let lines = Vec::new();
        let borders = Vec::new();
        let suppressed = HashSet::new();
        let urls = HashMap::new();
        let charts = HashMap::new();
        let palettes = HashMap::new();
        let chart_svg_cache = HashMap::new();
        let options = PrintLayoutOptions {
            line_numbers: &lines,
            page_borders: &borders,
            suppressed_line_number_paragraphs: &suppressed,
            picture_urls: &urls,
            chart_spaces: &charts,
            chart_color_palettes: &palettes,
            model: None,
            review_mode: ReviewDisplayMode::All,
            now: PrintNow::default(),
            page_color: None,
            zoom: 1.0,
            current_page: Some(0),
            chart_svg_cache: &chart_svg_cache,
        };
        let mut buf = CommandBuffer::new();
        let mut intern = Rec(Vec::new());
        paint_print_layout(&mut buf, &mut intern, &result, &options);
        assert!(
            intern.0.iter().any(|s| s == "Hi"),
            "fragment text missing from the frame: {:?}",
            intern.0.iter().filter(|s| s.len() < 8).collect::<Vec<_>>()
        );
        assert!(intern.0.iter().any(|s| s == "dve-print-line"));
    }
}
