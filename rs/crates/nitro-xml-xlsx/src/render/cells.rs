// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/cell-layer.ts; changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;
use nitro_xml_render::measure::MeasureBatch;

use crate::intern::StringIntern;
use crate::model::{ActiveWorksheet, WorkbookSlice};
use crate::render::geometry::header_sizes;
use crate::render::viewport::{paint_indices, visible_cells_for_host};
use crate::render::GridPaintViewport;
use crate::sheet_access::CellIndex;
use crate::view::cell_items::{build_items, BuildInput};
use crate::view::cell_paint::{
    align_items, css_font, diagonal_svg, edge_css, fill_paint, has_borders, justify, text_align,
    text_decoration,
};
use crate::view::text_measure::CanvasTextMeasure;
use crate::view::types::CellItem;

const CELL_BASE: u32 = 100;
const TEXT_BASE: u32 = 10_000;
const BORDER_BASE: u32 = 20_000;
const COMMENT_BASE: u32 = 30_000;
const ERROR_BASE: u32 = 40_000;

fn set_style<I: StringIntern>(
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
fn paint_one_cell<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    div: u32,
    n: usize,
    item: &CellItem,
    zoom: f64,
    ox: i32,
    oy: i32,
    scroll_x: i32,
    scroll_y: i32,
) {
    let cell_id = CELL_BASE + n as u32;
    let left = ox + item.x - scroll_x;
    let top = oy + item.y - scroll_y;
    buf.create_element(cell_id, div, 1);
    set_style(buf, intern, cell_id, "position", "absolute");
    set_style(buf, intern, cell_id, "left", &format!("{}px", left));
    set_style(buf, intern, cell_id, "top", &format!("{}px", top));
    set_style(buf, intern, cell_id, "width", &format!("{}px", item.w));
    set_style(buf, intern, cell_id, "height", &format!("{}px", item.h));
    set_style(buf, intern, cell_id, "boxSizing", "border-box");
    set_style(buf, intern, cell_id, "overflow", "visible");

    let fill = fill_paint(item.view.fill.as_ref());
    if let Some(ref fp) = fill {
        if let Some(bg) = &fp.background {
            set_style(buf, intern, cell_id, "backgroundColor", bg);
        }
        if let Some(img) = &fp.image {
            set_style(buf, intern, cell_id, "backgroundImage", img);
        }
    } else if item.merged {
        set_style(buf, intern, cell_id, "backgroundColor", "#fff");
    }

    if item.view.has_hyperlink {
        set_style(buf, intern, cell_id, "cursor", "pointer");
    }

    if !item.text.is_empty() {
        let text_id = TEXT_BASE + n as u32;
        buf.create_element(text_id, div, cell_id);
        set_style(buf, intern, text_id, "position", "absolute");
        set_style(buf, intern, text_id, "top", "0");
        set_style(buf, intern, text_id, "bottom", "0");
        set_style(
            buf,
            intern,
            text_id,
            "left",
            &format!("{}px", item.text_left),
        );
        set_style(
            buf,
            intern,
            text_id,
            "width",
            &format!("{}px", item.text_width),
        );
        set_style(buf, intern, text_id, "display", "flex");
        set_style(buf, intern, text_id, "boxSizing", "border-box");
        set_style(buf, intern, text_id, "overflow", "hidden");
        set_style(buf, intern, text_id, "lineHeight", "1.15");
        set_style(
            buf,
            intern,
            text_id,
            "font",
            &css_font(&item.view.font, zoom * item.font_scale),
        );
        set_style(buf, intern, text_id, "color", &item.view.font.color);
        set_style(
            buf,
            intern,
            text_id,
            "textDecoration",
            &text_decoration(&item.view.font),
        );
        set_style(
            buf,
            intern,
            text_id,
            "justifyContent",
            justify(&item.view.h_align),
        );
        set_style(
            buf,
            intern,
            text_id,
            "alignItems",
            align_items(&item.view.v_align),
        );
        set_style(
            buf,
            intern,
            text_id,
            "textAlign",
            text_align(&item.view.h_align),
        );
        set_style(
            buf,
            intern,
            text_id,
            "whiteSpace",
            if item.view.wrap { "pre-wrap" } else { "pre" },
        );
        let pad = ((2.0 * (zoom / 100.0)).round() as i32).max(1);
        let indent = pad + (item.view.indent_px * (zoom / 100.0)).round() as i32;
        let (pl, pr) = if item.view.h_align == "left" || item.view.h_align == "distributed" {
            (indent, pad)
        } else if item.view.h_align == "right" {
            (pad, indent)
        } else {
            (pad, pad)
        };
        set_style(buf, intern, text_id, "paddingLeft", &format!("{}px", pl));
        set_style(buf, intern, text_id, "paddingRight", &format!("{}px", pr));
        if item.overflowing {
            set_style(buf, intern, text_id, "background", "#fff");
        }
        if item.view.wrap {
            set_style(buf, intern, text_id, "overflowWrap", "anywhere");
        }
        if item.view.vertical_text {
            set_style(buf, intern, text_id, "writingMode", "vertical-rl");
            set_style(buf, intern, text_id, "textOrientation", "upright");
        }
        let span_id = TEXT_BASE + 50_000 + n as u32;
        buf.create_element(span_id, div, text_id);
        set_style(buf, intern, span_id, "display", "inline-block");
        set_style(buf, intern, span_id, "whiteSpace", "inherit");
        if item.view.rotation != 0.0 {
            set_style(
                buf,
                intern,
                span_id,
                "transform",
                &format!("rotate({}deg)", -item.view.rotation),
            );
        }
        let text = intern.intern(&item.text);
        buf.set_text(span_id, text);
    }

    if has_borders(&item.view.borders) {
        let border_id = BORDER_BASE + n as u32;
        buf.create_element(border_id, div, cell_id);
        set_style(buf, intern, border_id, "position", "absolute");
        set_style(buf, intern, border_id, "left", "-1px");
        set_style(buf, intern, border_id, "top", "-1px");
        set_style(
            buf,
            intern,
            border_id,
            "width",
            &format!("{}px", item.w + 1),
        );
        set_style(
            buf,
            intern,
            border_id,
            "height",
            &format!("{}px", item.h + 1),
        );
        set_style(buf, intern, border_id, "boxSizing", "border-box");
        set_style(buf, intern, border_id, "pointerEvents", "none");
        let b = &item.view.borders;
        if b.top.is_some() {
            set_style(
                buf,
                intern,
                border_id,
                "borderTop",
                &edge_css(b.top.as_ref()),
            );
        }
        if b.right.is_some() {
            set_style(
                buf,
                intern,
                border_id,
                "borderRight",
                &edge_css(b.right.as_ref()),
            );
        }
        if b.bottom.is_some() {
            set_style(
                buf,
                intern,
                border_id,
                "borderBottom",
                &edge_css(b.bottom.as_ref()),
            );
        }
        if b.left.is_some() {
            set_style(
                buf,
                intern,
                border_id,
                "borderLeft",
                &edge_css(b.left.as_ref()),
            );
        }
        if let Some(svg) = diagonal_svg(b, item.w, item.h) {
            let markup = intern.intern(&svg);
            buf.set_inner_markup(cell_id, markup);
        }
    }

    if item.view.has_comment {
        let cm_id = COMMENT_BASE + n as u32;
        buf.create_element(cm_id, div, cell_id);
        set_style(buf, intern, cm_id, "position", "absolute");
        set_style(buf, intern, cm_id, "right", "0");
        set_style(buf, intern, cm_id, "top", "0");
        set_style(buf, intern, cm_id, "width", "0");
        set_style(buf, intern, cm_id, "height", "0");
        set_style(buf, intern, cm_id, "borderLeft", "6px solid transparent");
        set_style(buf, intern, cm_id, "borderTop", "6px solid #d13438");
        set_style(buf, intern, cm_id, "zIndex", "2");
    }

    if item.error_mark {
        let er_id = ERROR_BASE + n as u32;
        buf.create_element(er_id, div, cell_id);
        set_style(buf, intern, er_id, "position", "absolute");
        set_style(buf, intern, er_id, "left", "0");
        set_style(buf, intern, er_id, "top", "0");
        set_style(buf, intern, er_id, "width", "0");
        set_style(buf, intern, er_id, "height", "0");
        set_style(buf, intern, er_id, "borderRight", "6px solid transparent");
        set_style(buf, intern, er_id, "borderTop", "6px solid #107c41");
        set_style(buf, intern, er_id, "zIndex", "2");
    }
}

#[allow(clippy::too_many_arguments)]
pub fn paint<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    measures: &mut MeasureBatch,
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
    sheet_index: usize,
    all_sheets: &[Option<ActiveWorksheet>],
    metrics: &crate::view::metrics::GridMetrics,
    viewport: &GridPaintViewport,
) {
    let sizes = header_sizes(sheet, metrics, metrics.last_row);
    let vis = visible_cells_for_host(metrics, viewport, sizes.w, sizes.h);
    let (rows, cols) = paint_indices(&vis);
    if rows.is_empty() || cols.is_empty() {
        return;
    }
    let index = CellIndex::build(sheet);
    let zoom = metrics.zoom / 100.0;
    let has_formula = |row: u32, col: u32| {
        index
            .get(sheet, row, col)
            .and_then(|c| c.formula.as_ref())
            .is_some()
    };
    let items = {
        let mut canvas_measure = CanvasTextMeasure::new(intern, measures);
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
            has_formula: &has_formula,
        };
        build_items(&input, &mut canvas_measure)
    };
    let div = intern.intern("div");
    let ox = sizes.w;
    let oy = sizes.h;
    for (n, item) in items.iter().enumerate() {
        paint_one_cell(
            buf,
            intern,
            div,
            n,
            item,
            metrics.zoom,
            ox,
            oy,
            viewport.scroll_x,
            viewport.scroll_y,
        );
    }
}
