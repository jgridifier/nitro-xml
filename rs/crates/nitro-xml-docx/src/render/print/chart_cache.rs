// SPDX-License-Identifier: MIT
//! Chart SVG cache keys and layout walk for canvas measure probes.

use std::collections::HashSet;

use super::blocks::parse_block;
use crate::layout::{LayoutBlockBox, LayoutFragment, LayoutResult};

pub(crate) fn chart_svg_key(part_name: &str, width: u32, height: u32) -> String {
    format!("{}#{}x{}", part_name, width, height)
}

pub(crate) struct ChartMeasureSpec {
    pub key: String,
    pub part_name: String,
    pub width: u32,
    pub height: u32,
}

pub(crate) fn chart_measure_specs(result: &LayoutResult) -> Vec<ChartMeasureSpec> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for page in &result.pages {
        for column in &page.columns {
            for value in &column.blocks {
                walk_block(value, &mut seen, &mut out);
            }
        }
    }
    out
}

fn walk_block(
    value: &serde_json::Value,
    seen: &mut HashSet<String>,
    out: &mut Vec<ChartMeasureSpec>,
) {
    let Some(block) = parse_block(value) else {
        return;
    };
    match block {
        LayoutBlockBox::Paragraph(paragraph) => {
            for line in &paragraph.lines {
                for fragment in &line.fragments {
                    push_chart_fragment(fragment, seen, out);
                }
            }
        }
        LayoutBlockBox::Table(table) => {
            for row in &table.rows {
                for cell in &row.cells {
                    for paragraph in cell {
                        for line in &paragraph.lines {
                            for fragment in &line.fragments {
                                push_chart_fragment(fragment, seen, out);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn push_chart_fragment(
    fragment: &LayoutFragment,
    seen: &mut HashSet<String>,
    out: &mut Vec<ChartMeasureSpec>,
) {
    let Some(object) = fragment.object.as_ref() else {
        return;
    };
    if object.chart.is_none() || object.part_name.is_empty() {
        return;
    }
    let width = object.width_px.round().max(1.0) as u32;
    let height = object.height_px.round().max(1.0) as u32;
    let key = chart_svg_key(&object.part_name, width, height);
    if seen.insert(key.clone()) {
        out.push(ChartMeasureSpec {
            key,
            part_name: object.part_name.clone(),
            width,
            height,
        });
    }
}
