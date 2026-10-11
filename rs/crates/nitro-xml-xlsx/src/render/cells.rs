// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/cell-layer.ts; changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::model::{ActiveWorksheet, WorkbookSlice};
use crate::render::GridPaintViewport;
use crate::sheet_access::CellIndex;
use crate::view::cell_items::{build_items, BuildInput};
use crate::view::cell_paint::css_font;
use crate::view::fake_measure::fake_xlsx_measure;

pub fn paint(
    buf: &mut CommandBuffer,
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
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
        sheet_index: 0,
        all_sheets: &[],
        index: &index,
        metrics,
        zoom,
        rows: &rows,
        cols: &cols,
        measure: &measure,
        has_formula: &has_formula,
    };
    let items = build_items(&input);
    for (n, item) in items.iter().enumerate() {
        let id = 100u32 + n as u32;
        buf.create_element(id, 2, 1);
        let font = css_font(&item.view.font, zoom);
        let _ = &font;
        buf.set_text(id, id + 10000);
        buf.set_style(id, 1, 2);
        buf.draw_text(
            item.x - viewport.scroll_x,
            item.y - viewport.scroll_y,
            id,
            item.view.font.size_px as f32,
            id + 10000,
            0xff000000,
        );
    }
    // TODO(#18 X8): drawings layer; TODO(#18 X11): sparklines in cell layer.
}
