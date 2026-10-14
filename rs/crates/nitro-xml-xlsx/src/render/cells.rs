// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/cell-layer.ts; changes listed in PROVENANCE.md.

use std::collections::HashSet;

use nitro_xml_render::command_buffer::CommandBuffer;
use nitro_xml_render::measure::MeasureBatch;

use crate::cf::icon_svg;
use crate::editor::mark_stale_class;
use crate::intern::StringIntern;
use crate::model::{ActiveWorksheet, WorkbookSlice};
use crate::render::geometry::header_sizes;
use crate::render::grid::{CELLS_ID, CELLS_TOP_ID, OVERLAY_ID};
use crate::render::viewport::{paint_indices, visible_cells_for_host};
use crate::render::{EditorPaintHints, GridPaintViewport};
use crate::sheet_access::CellIndex;
use crate::view::cell_items::{build_items, BuildInput};
use crate::view::cell_paint::{
    align_items, css_font, diagonal_svg, edge_css, fill_paint, has_borders, justify, text_align,
    text_decoration,
};
use crate::view::cell_view::is_blank_view;
use crate::view::text_measure::CanvasTextMeasure;
use crate::view::types::CellItem;

const CELL_BASE: u32 = 100;
const TEXT_BASE: u32 = 10_000;
const BORDER_BASE: u32 = 20_000;
const COMMENT_BASE: u32 = 30_000;
const ERROR_BASE: u32 = 40_000;
const ICON_BASE: u32 = 50_000;
const DATA_BAR_BASE: u32 = 55_000;
const STALE_BASE: u32 = 60_000;
const EDITOR_BOX: u32 = 70_000;
const EDITOR_BEFORE: u32 = 70_001;
const EDITOR_CARET: u32 = 70_002;
const EDITOR_AFTER: u32 = 70_003;

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

fn set_attr<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    id: u32,
    key: &str,
    val: &str,
) {
    let k = intern.intern(key);
    let v = intern.intern(val);
    buf.set_attr(id, k, v);
}

#[allow(clippy::too_many_arguments)]
fn paint_one_cell<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    div: u32,
    span: u32,
    n: usize,
    item: &CellItem,
    zoom: f64,
    stale: bool,
    parent: u32,
) {
    let cell_id = CELL_BASE + n as u32;
    // Plane-relative coords; scroll is on `.xg-plane` translate3d (upstream cell-layer.ts).
    let left = item.x;
    let top = item.y;
    buf.create_element(cell_id, div, parent);
    let mut cell_class = String::from("xg-c");
    if item.overflowing {
        cell_class.push_str(" xg-c-over");
    }
    if item.view.has_hyperlink {
        cell_class.push_str(" xg-link");
    }
    if item.merged {
        cell_class.push_str(" xg-merged");
    }
    mark_stale_class(&mut cell_class, stale);
    set_attr(buf, intern, cell_id, "class", &cell_class);
    if stale {
        set_attr(buf, intern, cell_id, "data-stale", "1");
    }
    set_style(buf, intern, cell_id, "left", &format!("{}px", left));
    set_style(buf, intern, cell_id, "top", &format!("{}px", top));
    set_style(buf, intern, cell_id, "width", &format!("{}px", item.w));
    set_style(buf, intern, cell_id, "height", &format!("{}px", item.h));

    let fill = fill_paint(item.view.fill.as_ref());
    if let Some(ref fp) = fill {
        if let Some(bg) = &fp.background {
            set_style(buf, intern, cell_id, "backgroundColor", bg);
        }
        if let Some(img) = &fp.image {
            set_style(buf, intern, cell_id, "backgroundImage", img);
        }
    } else if item.merged {
        set_style(
            buf,
            intern,
            cell_id,
            "backgroundColor",
            "var(--xve-sheet-bg, #fff)",
        );
    }

    if let Some(bar) = &item.view.data_bar {
        let bar_id = DATA_BAR_BASE + n as u32;
        buf.create_element(bar_id, div, cell_id);
        set_attr(buf, intern, bar_id, "class", "xg-db");
        let width = (bar.fraction.clamp(0.0, 1.0) * (item.w as f64 - 4.0)).max(0.0);
        set_style(buf, intern, bar_id, "width", &format!("{}px", width));
        let rtl = bar.direction.as_deref() == Some("rightToLeft");
        if let Some(start) = bar.start {
            set_style(
                buf,
                intern,
                bar_id,
                "left",
                &format!("{}px", 2.0 + start * (item.w as f64 - 4.0)),
            );
            set_style(buf, intern, bar_id, "right", "auto");
        } else if rtl {
            set_style(buf, intern, bar_id, "left", "auto");
            set_style(buf, intern, bar_id, "right", "2px");
        } else {
            set_style(buf, intern, bar_id, "left", "2px");
            set_style(buf, intern, bar_id, "right", "auto");
        }
        let bg = if bar.gradient == Some(false) {
            bar.color.clone()
        } else {
            let deg = if rtl { 270 } else { 90 };
            format!(
                "linear-gradient({deg}deg, {}, color-mix(in srgb, {} 15%, #fff))",
                bar.color, bar.color
            )
        };
        set_style(buf, intern, bar_id, "background", &bg);
        if let Some(border) = &bar.border_color {
            set_style(
                buf,
                intern,
                bar_id,
                "border",
                &format!("1px solid {border}"),
            );
        } else {
            set_style(buf, intern, bar_id, "border", "none");
        }
    }

    if let Some(icon) = &item.view.icon {
        let icon_id = ICON_BASE + n as u32;
        let size = (16.0 * (zoom / 100.0)).round().max(1.0) as u32;
        let markup = icon_svg(&icon.set, icon.index, size);
        buf.create_element(icon_id, div, cell_id);
        set_attr(buf, intern, icon_id, "class", "xg-ic");
        // Collapse the wrapper to the SVG box (inline SVG baseline would be ~21px tall).
        set_style(buf, intern, icon_id, "width", &format!("{size}px"));
        set_style(buf, intern, icon_id, "height", &format!("{size}px"));
        set_style(buf, intern, icon_id, "lineHeight", "0");
        set_style(buf, intern, icon_id, "overflow", "hidden");
        set_style(buf, intern, icon_id, "display", "block");
        buf.set_inner_markup(icon_id, intern.intern(&markup));
    }

    if !item.text.is_empty() {
        let text_id = TEXT_BASE + n as u32;
        buf.create_element(text_id, div, cell_id);
        let mut text_class = String::from("xg-t");
        if item.view.wrap {
            text_class.push_str(" xg-wrap");
        }
        if item.view.vertical_text {
            text_class.push_str(" xg-vert");
        }
        if item.overflowing {
            text_class.push_str(" xg-over");
        }
        set_attr(buf, intern, text_id, "class", &text_class);
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
        if item.view.icon.is_some() && item.view.h_align != "right" {
            set_style(
                buf,
                intern,
                text_id,
                "paddingLeft",
                &format!("{}px", 20.0 * (zoom / 100.0)),
            );
        }
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
        if !item.view.wrap {
            set_style(buf, intern, text_id, "whiteSpace", "pre");
        }
        // Upstream cell-layer.ts: pad rounded; indent keeps fractional CSS px.
        let pad = ((2.0 * (zoom / 100.0)).round() as i32).max(1) as f64;
        let indent = pad + item.view.indent_px * (zoom / 100.0);
        let (pl, pr) = if item.view.h_align == "left" || item.view.h_align == "distributed" {
            (indent, pad)
        } else if item.view.h_align == "right" {
            (pad, indent)
        } else {
            (pad, pad)
        };
        set_style(buf, intern, text_id, "paddingLeft", &format!("{pl}px"));
        set_style(buf, intern, text_id, "paddingRight", &format!("{pr}px"));
        let span_id = TEXT_BASE + 50_000 + n as u32;
        // Upstream uses <span class="xg-tx"> (inline-block via CSS).
        buf.create_element(span_id, span, text_id);
        set_attr(buf, intern, span_id, "class", "xg-tx");
        if item.view.rotation != 0.0 {
            set_style(
                buf,
                intern,
                span_id,
                "transform",
                &format!("rotate({}deg)", -item.view.rotation),
            );
        }
        // Upstream cell-layer.ts: rich runs get per-run font / superscript spans.
        let paint_rich = item
            .view
            .rich
            .as_ref()
            .is_some_and(|r| !r.is_empty() && item.text == item.view.text);
        if paint_rich {
            let runs = item.view.rich.as_ref().unwrap();
            for (ri, run) in runs.iter().enumerate() {
                let run_id = TEXT_BASE + 70_000 + (n as u32) * 64 + ri as u32;
                buf.create_element(run_id, span, span_id);
                set_style(
                    buf,
                    intern,
                    run_id,
                    "font",
                    &css_font(&run.font, zoom * item.font_scale),
                );
                set_style(buf, intern, run_id, "color", &run.font.color);
                set_style(
                    buf,
                    intern,
                    run_id,
                    "textDecoration",
                    &text_decoration(&run.font),
                );
                if let Some(va) = run.font.vert_align.as_deref() {
                    let css_va = if va == "super" || va == "superscript" {
                        "super"
                    } else {
                        "sub"
                    };
                    set_style(buf, intern, run_id, "verticalAlign", css_va);
                }
                let t = intern.intern(&run.text);
                buf.set_text(run_id, t);
            }
        } else {
            if let Some(va) = item.view.font.vert_align.as_deref() {
                let css_va = if va == "super" || va == "superscript" {
                    "super"
                } else {
                    "sub"
                };
                set_style(buf, intern, span_id, "verticalAlign", css_va);
            }
            let text = intern.intern(&item.text);
            buf.set_text(span_id, text);
        }
    }

    if has_borders(&item.view.borders) {
        let border_id = BORDER_BASE + n as u32;
        buf.create_element(border_id, div, cell_id);
        set_attr(buf, intern, border_id, "class", "xg-b");
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
        set_attr(buf, intern, cm_id, "class", "xg-cm");
    }

    if item.error_mark {
        let er_id = ERROR_BASE + n as u32;
        buf.create_element(er_id, div, cell_id);
        set_attr(buf, intern, er_id, "class", "xg-er");
    }

    if stale {
        let mark = STALE_BASE + n as u32;
        buf.create_element(mark, div, cell_id);
        set_attr(buf, intern, mark, "class", "xg-stale-mark");
        set_attr(buf, intern, mark, "title", "Stale formula");
        set_style(buf, intern, mark, "position", "absolute");
        set_style(buf, intern, mark, "left", "0");
        set_style(buf, intern, mark, "bottom", "0");
        set_style(buf, intern, mark, "width", "0");
        set_style(buf, intern, mark, "height", "0");
        set_style(buf, intern, mark, "borderRight", "8px solid transparent");
        set_style(buf, intern, mark, "borderBottom", "8px solid #c45911");
        set_style(buf, intern, mark, "zIndex", "3");
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
    hints: &EditorPaintHints,
) {
    let sizes = header_sizes(sheet, metrics, metrics.last_row);
    let vis = visible_cells_for_host(metrics, viewport, sizes.w, sizes.h);
    let (rows, cols) = paint_indices(&vis);
    if rows.is_empty() || cols.is_empty() {
        paint_edit_overlay(buf, intern, metrics, hints);
        return;
    }
    let index = CellIndex::build(sheet);
    // Live grid matches upstream grid-view.ts: metrics.zoom is percent (e.g. 100).
    // View-IR dumps still pass zoom/100 for FakeTextMeasure — do not change that path here.
    let zoom = metrics.zoom;
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
            // Match upstream grid-view.ts `fitNumber` injection for live paint only.
            enable_number_fit: true,
        };
        build_items(&input, &mut canvas_measure)
    };
    let div = intern.intern("div");
    let span = intern.intern("span");
    // Upstream CellViewCache returns null for isBlankView — no DOM node.
    // View-IR keeps blanks (oracle view() never null); only the painter skips.
    let stale: HashSet<(u32, u32)> = hints.stale.iter().copied().collect();
    let frozen_rows = viewport.frozen_rows;
    let mut n = 0usize;
    for item in &items {
        let is_stale = stale.contains(&(item.row, item.col));
        if !item.merged && is_blank_view(&item.view) && !is_stale {
            continue;
        }
        // Frozen rows live in `xg-q-top`; scrolling rows in `xg-q-main` (grid-view.ts).
        let parent = if frozen_rows > 0 && item.row < frozen_rows {
            CELLS_TOP_ID
        } else {
            CELLS_ID
        };
        paint_one_cell(
            buf,
            intern,
            div,
            span,
            n,
            item,
            metrics.zoom,
            is_stale,
            parent,
        );
        n += 1;
    }
    paint_edit_overlay(buf, intern, metrics, hints);
}

fn paint_edit_overlay<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    metrics: &crate::view::metrics::GridMetrics,
    hints: &EditorPaintHints,
) {
    let Some(edit) = &hints.edit else {
        return;
    };
    let div = intern.intern("div");
    let span = intern.intern("span");
    let left = metrics.col_left(edit.col);
    let top = metrics.row_top(edit.row);
    let w = metrics.col_width(edit.col).max(8);
    let h = metrics.row_height(edit.row).max(8);
    buf.create_element(EDITOR_BOX, div, OVERLAY_ID);
    set_attr(buf, intern, EDITOR_BOX, "class", "xg-edit");
    set_style(buf, intern, EDITOR_BOX, "position", "absolute");
    set_style(buf, intern, EDITOR_BOX, "left", &format!("{left}px"));
    set_style(buf, intern, EDITOR_BOX, "top", &format!("{top}px"));
    set_style(buf, intern, EDITOR_BOX, "width", &format!("{w}px"));
    set_style(buf, intern, EDITOR_BOX, "height", &format!("{h}px"));
    set_style(buf, intern, EDITOR_BOX, "background", "#fff");
    set_style(buf, intern, EDITOR_BOX, "border", "2px solid #217346");
    set_style(buf, intern, EDITOR_BOX, "boxSizing", "border-box");
    set_style(buf, intern, EDITOR_BOX, "zIndex", "6");
    set_style(buf, intern, EDITOR_BOX, "overflow", "hidden");
    set_style(buf, intern, EDITOR_BOX, "whiteSpace", "pre");
    set_style(
        buf,
        intern,
        EDITOR_BOX,
        "font",
        "13px Calibri,Carlito,sans-serif",
    );
    set_style(buf, intern, EDITOR_BOX, "padding", "0 2px");
    let (before, after) = split_caret(&edit.text, edit.caret);
    buf.create_element(EDITOR_BEFORE, span, EDITOR_BOX);
    buf.set_text(EDITOR_BEFORE, intern.intern(before));
    buf.create_element(EDITOR_CARET, span, EDITOR_BOX);
    set_attr(buf, intern, EDITOR_CARET, "class", "xg-edit-caret");
    set_style(buf, intern, EDITOR_CARET, "display", "inline-block");
    set_style(buf, intern, EDITOR_CARET, "width", "1px");
    set_style(buf, intern, EDITOR_CARET, "height", "1em");
    set_style(buf, intern, EDITOR_CARET, "background", "#111");
    set_style(buf, intern, EDITOR_CARET, "verticalAlign", "text-bottom");
    buf.create_element(EDITOR_AFTER, span, EDITOR_BOX);
    buf.set_text(EDITOR_AFTER, intern.intern(after));
}

fn split_caret(text: &str, caret: usize) -> (&str, &str) {
    let mut c = caret.min(text.len());
    while c > 0 && !text.is_char_boundary(c) {
        c -= 1;
    }
    text.split_at(c)
}
