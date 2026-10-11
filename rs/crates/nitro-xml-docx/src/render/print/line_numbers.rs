// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/docx/print-line-numbers.ts; changes listed in PROVENANCE.md.

use std::collections::HashSet;

use super::blocks::parse_block;
use super::dom::{Dom, StringIntern};
use crate::layout::{LayoutBlockBox, LayoutColumnBox, LayoutPageBox};

#[derive(Clone, Debug)]
pub struct PrintLineNumbering {
    pub count_by: i64,
    pub start: i64,
    pub restart: String,
    pub distance_twips: Option<f64>,
}

pub struct LineCounter {
    pub next: i64,
    pub section: i64,
}

impl Default for LineCounter {
    fn default() -> Self {
        Self {
            next: 1,
            section: -1,
        }
    }
}

pub fn line_numbering_from_section(section: &serde_json::Value) -> Option<PrintLineNumbering> {
    let raw = section.get("lineNumberSettings")?;
    let count_by = json_i64(raw, "countBy").unwrap_or(1).max(1);
    Some(PrintLineNumbering {
        count_by,
        start: json_i64(raw, "start").unwrap_or(1),
        restart: raw
            .get("restart")
            .and_then(|v| v.as_str())
            .unwrap_or("continuous")
            .to_string(),
        distance_twips: raw
            .get("distanceTwips")
            .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64))),
    })
}

fn json_i64(v: &serde_json::Value, key: &str) -> Option<i64> {
    v.get(key)
        .and_then(|n| n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)))
}

pub fn suppressed_line_number_ids(blocks: &[serde_json::Value]) -> HashSet<String> {
    blocks
        .iter()
        .filter_map(|b| {
            if b.get("type").and_then(|v| v.as_str()) != Some("paragraph") {
                return None;
            }
            if b.get("suppressLineNumbers").and_then(|v| v.as_bool()) != Some(true) {
                return None;
            }
            b.get("id").and_then(|v| v.as_str()).map(|s| s.to_string())
        })
        .collect()
}

pub fn paint_line_number_labels<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    parent: u32,
    page: &LayoutPageBox,
    section_index: usize,
    columns: &[&LayoutColumnBox],
    settings: &PrintLineNumbering,
    counter: &mut LineCounter,
    suppressed: &HashSet<String>,
) {
    let new_section = counter.section != section_index as i64;
    if counter.section == -1
        || settings.restart == "newPage"
        || (new_section && settings.restart == "newSection")
    {
        counter.next = settings.start;
    }
    counter.section = section_index as i64;
    let gap = settings.distance_twips.unwrap_or(360.0) / 15.0;
    let count_by = settings.count_by.max(1);
    for column in columns {
        for value in &column.blocks {
            let Some(block) = parse_block(value) else {
                continue;
            };
            let LayoutBlockBox::Paragraph(paragraph) = block else {
                continue;
            };
            if suppressed.contains(&paragraph.block_id) {
                continue;
            }
            for line in &paragraph.lines {
                let number = counter.next;
                counter.next += 1;
                if number % count_by != 0 {
                    continue;
                }
                let label = dom.el(parent, "span");
                dom.class(label, "dve-print-line-number");
                dom.text(label, &number.to_string());
                dom.px(
                    label,
                    "left",
                    page.margin_left_px + column.x_px - gap - 40.0,
                );
                dom.px(
                    label,
                    "top",
                    page.margin_top_px + paragraph.y_px + line.y_px,
                );
                dom.px(label, "height", line.height_px);
                dom.px(label, "line-height", line.height_px);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::LayoutColumnBox;
    use crate::render::print::dom::{Dom, StringIntern};
    use nitro_xml_render::CommandBuffer;
    use serde_json::json;

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

    fn page_with_one_line() -> (LayoutPageBox, LayoutColumnBox) {
        let block = json!({
            "kind": "paragraph",
            "blockId": "p1",
            "yPx": 0,
            "heightPx": 16,
            "lines": [{
                "yPx": 0,
                "heightPx": 16,
                "fragments": [{ "text": "A", "xPx": 0, "widthPx": 8, "runIndex": 0 }],
                "sourceStart": 0,
                "sourceEnd": 1
            }]
        });
        let column = LayoutColumnBox {
            x_px: 0.0,
            width_px: 400.0,
            blocks: vec![block],
            section_index: Some(0),
            start_y_px: None,
            end_y_px: None,
            separator: None,
        };
        let page = LayoutPageBox {
            index: 0,
            section_index: 0,
            section_indices: None,
            page_in_section: 0,
            width_px: 800.0,
            height_px: 1000.0,
            margin_top_px: 96.0,
            margin_right_px: 96.0,
            margin_bottom_px: 96.0,
            margin_left_px: 96.0,
            columns: vec![],
            column_separator: None,
            floats: None,
            footnotes: None,
        };
        (page, column)
    }

    #[test]
    fn continuous_line_numbers_carry_across_pages() {
        // Catches: continuous line numbering restarting at 1 on every page.
        let (page, column) = page_with_one_line();
        let settings = PrintLineNumbering {
            count_by: 1,
            start: 1,
            restart: "continuous".into(),
            distance_twips: None,
        };
        let mut counter = LineCounter::default();
        let suppressed = HashSet::new();
        let mut buf = CommandBuffer::new();
        let mut intern = Rec(Vec::new());
        {
            let mut dom = Dom::new(&mut buf, &mut intern);
            let cols = [&column];
            paint_line_number_labels(
                &mut dom,
                0,
                &page,
                0,
                &cols,
                &settings,
                &mut counter,
                &suppressed,
            );
            paint_line_number_labels(
                &mut dom,
                0,
                &page,
                0,
                &cols,
                &settings,
                &mut counter,
                &suppressed,
            );
        }
        assert!(
            intern.0.iter().any(|s| s == "1") && intern.0.iter().any(|s| s == "2"),
            "expected labels 1 then 2, got {:?}",
            intern.0
        );
        assert_eq!(counter.next, 3);
    }
}
