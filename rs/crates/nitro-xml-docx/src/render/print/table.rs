// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/docx/print-table.ts; changes listed in PROVENANCE.md.

use super::dom::{js_num, Dom, StringIntern};
use crate::layout::{LayoutBorder, LayoutCellGeometry, LayoutParagraphBox, LayoutTableBox};

fn css_border(border: &Option<LayoutBorder>) -> String {
    match border {
        Some(b) => format!("{}px {} {}", js_num(b.width_px), b.style, b.color),
        None => String::new(),
    }
}

fn geometry_of(
    row: &crate::layout::LayoutTableRowBox,
    index: usize,
    width_px: f64,
) -> LayoutCellGeometry {
    if let Some(given) = row.geometry.as_ref().and_then(|g| g.get(index)) {
        return given.clone();
    }
    let share = width_px / (row.cells.len().max(1) as f64);
    LayoutCellGeometry {
        x_px: share * index as f64,
        width_px: share,
        padding_left_px: 0.0,
        padding_right_px: 0.0,
        borders: None,
        shading: None,
        vertical_align: None,
        content_height_px: 0.0,
    }
}

pub fn paint_table<I: StringIntern>(
    dom: &mut Dom<'_, I>,
    parent: u32,
    box_: &LayoutTableBox,
    column_width_px: f64,
    mut paint_paragraph: impl FnMut(&mut Dom<'_, I>, u32, &LayoutParagraphBox),
) -> u32 {
    let table = dom.el(parent, "div");
    dom.class(table, "dve-print-block dve-print-table");
    dom.px(table, "top", box_.y_px);
    dom.px(table, "height", box_.height_px);
    if let Some(x) = box_.x_px {
        if x != 0.0 {
            dom.px(table, "left", x);
        }
    }
    let last_row = box_.rows.len().saturating_sub(1);
    for (row_index, row) in box_.rows.iter().enumerate() {
        let row_el = dom.el(table, "div");
        dom.class(row_el, "dve-print-row");
        dom.px(row_el, "top", row.y_px);
        dom.px(row_el, "height", row.height_px);
        if row.repeated {
            dom.attr(row_el, "data-repeated", "true");
        }
        let last = row_index == last_row;
        for (cell_index, paragraphs) in row.cells.iter().enumerate() {
            let geometry = geometry_of(row, cell_index, column_width_px);
            let cell = dom.el(row_el, "div");
            dom.class(cell, "dve-print-cell");
            dom.px(cell, "left", geometry.x_px);
            dom.px(cell, "width", geometry.width_px);
            dom.px(cell, "height", row.height_px);
            if let Some(shading) = &geometry.shading {
                dom.style(cell, "background", shading);
            }
            let borders = geometry.borders.as_ref();
            dom.style(
                cell,
                "border-top",
                &css_border(&borders.and_then(|b| b.top.clone())),
            );
            dom.style(
                cell,
                "border-left",
                &css_border(&borders.and_then(|b| b.left.clone())),
            );
            if cell_index + 1 == row.cells.len() {
                dom.style(
                    cell,
                    "border-right",
                    &css_border(&borders.and_then(|b| b.right.clone())),
                );
            }
            if last {
                dom.style(
                    cell,
                    "border-bottom",
                    &css_border(&borders.and_then(|b| b.bottom.clone())),
                );
            }
            let content = dom.el(cell, "div");
            dom.class(content, "dve-print-cell-content");
            let free = (row.height_px - geometry.content_height_px).max(0.0);
            let offset = match geometry.vertical_align.as_deref() {
                Some("center") => free / 2.0,
                Some("bottom") => free,
                _ => 0.0,
            };
            dom.px(content, "left", geometry.padding_left_px);
            dom.px(content, "right", geometry.padding_right_px);
            dom.px(content, "top", offset);
            for paragraph in paragraphs {
                paint_paragraph(dom, content, paragraph);
            }
        }
    }
    table
}
