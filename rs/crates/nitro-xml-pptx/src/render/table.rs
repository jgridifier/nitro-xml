// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/table.ts; changes in PROVENANCE.md.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_style, set_text_content};
use crate::slide::DumpElement;
use crate::table::TableData;

/// HTML `<table>` from parsed `a:tbl` cells. Merged continuation cells are skipped.
pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "table");
    set_style(ctx, id, "overflow", "hidden");
    if let Some(data) = &el.table_data {
        paint_grid(ctx, id, data);
    } else {
        paint_row_count_fallback(ctx, id, el.table_row_count.unwrap_or(1));
    }
    ctx.view_ir.push_painted("table", el, None);
}

fn paint_grid<I: StringIntern>(ctx: &mut PaintContext<'_, I>, parent: u32, data: &TableData) {
    let table = create_child(ctx, parent, "table");
    set_style(ctx, table, "width", "100%");
    set_style(ctx, table, "height", "100%");
    set_style(ctx, table, "border-collapse", "collapse");
    set_style(ctx, table, "table-layout", "fixed");
    // Match shape text paint / visual Liberation pin (test_slides soft band).
    set_style(
        ctx,
        table,
        "font-family",
        "\"Liberation Sans\", Arial, sans-serif",
    );
    // Upstream `defaultCellFontSize` from master `p:otherStyle` lvl1 (commonly 18pt).
    if let Some(pt) = data.default_cell_font_size_pt.filter(|p| *p > 0.0) {
        let s = format!("{pt:.4}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned();
        set_style(ctx, table, "font-size", &format!("{s}pt"));
    }
    set_style(ctx, table, "color", "#111827");
    append_colgroup(ctx, table, &data.column_widths);
    let tbody = create_child(ctx, table, "tbody");
    for (row_index, row) in data.rows.iter().enumerate() {
        let tr = create_child(ctx, tbody, "tr");
        if let Some(height) = row.height_px.filter(|h| *h > 0.0) {
            set_style(ctx, tr, "height", &px(height));
        }
        for (cell_index, cell) in row.cells.iter().enumerate() {
            if cell.h_merge || cell.v_merge {
                continue;
            }
            let td = create_child(ctx, tr, "td");
            set_attr(ctx, td, "data-row-index", &row_index.to_string());
            set_attr(ctx, td, "data-cell-index", &cell_index.to_string());
            if cell.grid_span > 1 {
                set_attr(ctx, td, "colspan", &cell.grid_span.to_string());
            }
            if cell.row_span > 1 {
                set_attr(ctx, td, "rowspan", &cell.row_span.to_string());
            }
            set_style(ctx, td, "position", "relative");
            set_style(ctx, td, "padding", "4.8px 9.6px");
            set_style(ctx, td, "vertical-align", "top");
            set_style(ctx, td, "white-space", "pre-wrap");
            set_style(ctx, td, "word-break", "break-word");
            set_style(ctx, td, "overflow-wrap", "break-word");
            set_style(ctx, td, "line-height", "1.2");
            set_style(ctx, td, "box-sizing", "border-box");
            if let Some(fill) = &cell.fill {
                set_style(ctx, td, "background-color", fill);
            }
            if let Some(color) = &cell.text_color {
                set_style(ctx, td, "color", color);
            }
            if cell.font_bold {
                set_style(ctx, td, "font-weight", "700");
            }
            let border = cell.border.as_deref().unwrap_or("1px solid #999999");
            set_style(ctx, td, "border", border);
            let span = create_child(ctx, td, "span");
            let text = if cell.text.is_empty() {
                " "
            } else {
                cell.text.as_str()
            };
            set_text_content(ctx, span, text);
        }
    }
}

fn append_colgroup<I: StringIntern>(ctx: &mut PaintContext<'_, I>, table: u32, widths: &[f64]) {
    if widths.is_empty() {
        return;
    }
    let colgroup = create_child(ctx, table, "colgroup");
    for width in widths {
        let col = create_child(ctx, colgroup, "col");
        set_style(ctx, col, "width", &format!("{:.2}%", width * 100.0));
    }
}

fn paint_row_count_fallback<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    parent: u32,
    rows: usize,
) {
    let table = create_child(ctx, parent, "table");
    set_style(ctx, table, "width", "100%");
    set_style(ctx, table, "height", "100%");
    set_style(ctx, table, "border-collapse", "collapse");
    for r in 0..rows.clamp(1, 12) {
        let tr = create_child(ctx, table, "tr");
        let td = create_child(ctx, tr, "td");
        set_style(ctx, td, "border", "1px solid #999");
        set_attr(ctx, td, "data-row", &r.to_string());
    }
}

fn px(v: f64) -> String {
    if (v - v.round()).abs() < 0.001 {
        format!("{}px", v.round() as i64)
    } else {
        format!("{v:.2}px")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use nitro_xml_render::measure::{LayoutReadBatch, MeasureBatch};
    use nitro_xml_render::CommandBuffer;

    use super::super::context::{PaintContext, StringIntern};
    use super::super::view_ir::ViewIrCollector;
    use super::paint;
    use crate::presentation::Presentation;

    struct Intern(Vec<String>);

    impl StringIntern for Intern {
        fn intern(&mut self, s: &str) -> u32 {
            if let Some(i) = self.0.iter().position(|existing| existing == s) {
                return i as u32;
            }
            self.0.push(s.to_owned());
            (self.0.len() - 1) as u32
        }
    }

    #[test]
    fn gallery_table_paints_cell_labels() {
        // Catches: pptx table paints empty rows so gallery table cells never show 1A..3C.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/upstream/pptx/b2aa81e78d15_ribbon-galleries.pptx");
        let bytes = std::fs::read(&path).unwrap_or_else(|err| panic!("read {path:?}: {err}"));
        let mut pres = Presentation::open(&bytes).unwrap();
        let slide = pres.slide(0).unwrap();
        let table = slide
            .elements
            .iter()
            .find(|el| el.type_ == "table")
            .expect("table element");
        let data = table.table_data.as_ref().expect("parsed a:tbl");
        let labels: Vec<&str> = data
            .rows
            .iter()
            .flat_map(|row| row.cells.iter().map(|cell| cell.text.as_str()))
            .collect();
        assert_eq!(
            labels,
            ["1A", "1B", "1C", "2A", "2B", "2C", "3A", "3B", "3C"]
        );
        assert!(
            data.rows
                .iter()
                .flat_map(|row| &row.cells)
                .all(|cell| cell.fill.as_deref() == Some("#DAE3F3")),
            "pale blue whole-table fill, got {:?}",
            data.rows
                .first()
                .and_then(|row| row.cells.first())
                .and_then(|cell| cell.fill.clone())
        );
        assert!(
            slide.elements.iter().any(|el| el.type_ == "chart"),
            "chart graphic frame is an element; other_kinds={:?}",
            slide.other_kinds
        );

        let mut buffer = CommandBuffer::new();
        let mut intern = Intern(Vec::new());
        let mut measure = MeasureBatch::default();
        let mut layout_reads = LayoutReadBatch::default();
        let mut view_ir = ViewIrCollector::default();
        let scheme = BTreeMap::new();
        let mut sp_autofit = crate::render::SpAutofitPaintState::default();
        let slide_autofit = sp_autofit.slide_mut(0);
        let mut ctx = PaintContext::new(
            &mut buffer,
            &mut intern,
            &mut measure,
            &mut layout_reads,
            &mut view_ir,
            1.0,
            960.0,
            540.0,
            &scheme,
            0,
            crate::render::FieldSubstitutionContext::default(),
            slide_autofit,
        );
        paint(&mut ctx, table, 1);
        for label in ["1A", "1B", "1C", "2A", "2B", "2C", "3A", "3B", "3C"] {
            assert!(
                intern.0.iter().any(|s| s == label),
                "painted table missing {label}"
            );
        }
        assert!(
            intern.0.iter().any(|s| s == "#DAE3F3"),
            "cells missing pale blue fill"
        );
        assert!(intern.0.iter().any(|s| s == "33.33%"));
        assert!(intern.0.iter().any(|s| s == "66px"));
        assert!(intern.0.iter().any(|s| s == "colgroup"));
    }
}
