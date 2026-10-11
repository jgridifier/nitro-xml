// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/cell-layer.ts; changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::intern::StringIntern;
use crate::model::{ActiveWorksheet, WorkbookSlice};
use crate::render::GridPaintViewport;
use crate::sheet_access::CellIndex;
use crate::view::cell_items::{build_items, BuildInput};
use crate::view::cell_paint::css_font;
use crate::view::fake_measure::fake_xlsx_measure;

fn set_style_px<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    id: u32,
    prop: &str,
    val: &str,
) {
    let p = intern.intern(prop);
    let v = intern.intern(val);
    buf.set_style(id, p, v);
}

#[allow(clippy::too_many_arguments)]
pub fn paint<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
    sheet_index: usize,
    all_sheets: &[Option<ActiveWorksheet>],
    metrics: &crate::view::metrics::GridMetrics,
    viewport: &GridPaintViewport,
) {
    let last_row = metrics.last_row.min(200);
    let last_col = metrics.last_col.min(60);
    let rows: Vec<u32> = (0..=last_row).collect();
    let cols: Vec<u32> = (0..=last_col).collect();
    let index = CellIndex::build(sheet);
    let zoom = metrics.zoom / 100.0;
    let measure = |text: &str, font: &str| {
        let _ = font;
        fake_xlsx_measure(text)
    };
    let has_formula = |row: u32, col: u32| {
        index
            .get(sheet, row, col)
            .and_then(|c| c.formula.as_ref())
            .is_some()
    };
    let input = BuildInput {
        workbook,
        sheet,
        sheet_index,
        all_sheets,
        index: &index,
        metrics,
        zoom,
        rows: &rows,
        cols: &cols,
        measure: &measure,
        has_formula: &has_formula,
    };
    let items = build_items(&input);
    let div = intern.intern("div");
    set_style_px(buf, intern, 1, "position", "relative");
    set_style_px(buf, intern, 1, "width", "100%");
    set_style_px(buf, intern, 1, "height", "100%");
    set_style_px(buf, intern, 1, "overflow", "hidden");
    for (n, item) in items.iter().enumerate() {
        let id = 100u32 + n as u32;
        buf.create_element(id, div, 1);
        let left = item.x - viewport.scroll_x;
        let top = item.y - viewport.scroll_y;
        set_style_px(buf, intern, id, "position", "absolute");
        set_style_px(buf, intern, id, "left", &format!("{}px", left));
        set_style_px(buf, intern, id, "top", &format!("{}px", top));
        set_style_px(buf, intern, id, "width", &format!("{}px", item.w));
        set_style_px(buf, intern, id, "height", &format!("{}px", item.h));
        set_style_px(buf, intern, id, "overflow", "hidden");
        set_style_px(buf, intern, id, "boxSizing", "border-box");
        set_style_px(buf, intern, id, "display", "flex");
        set_style_px(buf, intern, id, "alignItems", "center");
        set_style_px(buf, intern, id, "whiteSpace", "pre");
        let font = css_font(&item.view.font, zoom);
        let _ = &font;
        let text_id = intern.intern(&item.text);
        buf.set_text(id, text_id);
        if let Some(fill) = &item.view.fill {
            use crate::view::types::FillView;
            if let FillView::Background { background } = fill {
                set_style_px(buf, intern, id, "backgroundColor", background);
            }
        }
        if !item.view.font.color.is_empty() {
            set_style_px(buf, intern, id, "color", &item.view.font.color);
        }
    }
    // TODO(#18 X8): drawings layer; TODO(#18 X11): sparklines in cell layer.
}
