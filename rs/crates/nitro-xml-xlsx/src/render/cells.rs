// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/cell-layer.ts; changes listed in PROVENANCE.md.

use std::collections::HashSet;

use nitro_xml_render::command_buffer::CommandBuffer;
use nitro_xml_render::measure::MeasureBatch;

use crate::cf::icon_body;
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
const DATA_BAR_AXIS_BASE: u32 = 56_000;
const STALE_BASE: u32 = 60_000;
const EDITOR_BOX: u32 = 70_000;
const EDITOR_BEFORE: u32 = 70_001;
const EDITOR_CARET: u32 = 70_002;
const EDITOR_AFTER: u32 = 70_003;

/// CSS / attr keys interned once per `paint()` frame (#30 encode cut).
struct PaintKeys {
    class: u32,
    style: u32,
    background_color: u32,
    background_image: u32,
    width: u32,
    height: u32,
    left: u32,
    right: u32,
    top: u32,
    bottom: u32,
    background: u32,
    border: u32,
    position: u32,
    border_left: u32,
    border_right: u32,
    border_top: u32,
    border_bottom: u32,
    color: u32,
    font: u32,
    text_decoration: u32,
    white_space: u32,
    transform: u32,
    vertical_align: u32,
    z_index: u32,
    data_stale: u32,
    title: u32,
    view_box: u32,
    aria_hidden: u32,
    svg: u32,
    box_sizing: u32,
    overflow: u32,
    padding: u32,
    display: u32,
}

impl PaintKeys {
    fn new<I: StringIntern>(intern: &mut I) -> Self {
        Self {
            class: intern.intern("class"),
            style: intern.intern("style"),
            background_color: intern.intern("backgroundColor"),
            background_image: intern.intern("backgroundImage"),
            width: intern.intern("width"),
            height: intern.intern("height"),
            left: intern.intern("left"),
            right: intern.intern("right"),
            top: intern.intern("top"),
            bottom: intern.intern("bottom"),
            background: intern.intern("background"),
            border: intern.intern("border"),
            position: intern.intern("position"),
            border_left: intern.intern("borderLeft"),
            border_right: intern.intern("borderRight"),
            border_top: intern.intern("borderTop"),
            border_bottom: intern.intern("borderBottom"),
            color: intern.intern("color"),
            font: intern.intern("font"),
            text_decoration: intern.intern("textDecoration"),
            white_space: intern.intern("whiteSpace"),
            transform: intern.intern("transform"),
            vertical_align: intern.intern("verticalAlign"),
            z_index: intern.intern("zIndex"),
            data_stale: intern.intern("data-stale"),
            title: intern.intern("title"),
            view_box: intern.intern("viewBox"),
            aria_hidden: intern.intern("aria-hidden"),
            svg: intern.intern("svg"),
            box_sizing: intern.intern("boxSizing"),
            overflow: intern.intern("overflow"),
            padding: intern.intern("padding"),
            display: intern.intern("display"),
        }
    }
}

#[inline]
fn set_style_v<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    id: u32,
    prop: u32,
    val: &str,
) {
    buf.set_style(id, prop, intern.intern(val));
}

#[inline]
fn set_attr_v<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    id: u32,
    key: u32,
    val: &str,
) {
    buf.set_attr(id, key, intern.intern(val));
}

/// Format `left:Xpx;top:Ypx;width:Wpx;height:Hpx` into a stack buffer (no `format!`).
fn intern_box_geometry<I: StringIntern>(
    intern: &mut I,
    left: f64,
    top: f64,
    w: f64,
    h: f64,
) -> u32 {
    fn push_bytes(buf: &mut [u8], o: &mut usize, s: &[u8]) {
        buf[*o..*o + s.len()].copy_from_slice(s);
        *o += s.len();
    }
    fn push_i32(buf: &mut [u8], o: &mut usize, n: i32) {
        let mut tmp = [0u8; 16];
        let mut x = n.unsigned_abs();
        let neg = n < 0;
        let mut i = tmp.len();
        if x == 0 {
            i -= 1;
            tmp[i] = b'0';
        } else {
            while x > 0 {
                i -= 1;
                tmp[i] = b'0' + (x % 10) as u8;
                x /= 10;
            }
        }
        if neg {
            i -= 1;
            tmp[i] = b'-';
        }
        push_bytes(buf, o, &tmp[i..]);
    }
    let mut buf = [0u8; 96];
    let mut o = 0usize;
    push_bytes(&mut buf, &mut o, b"left:");
    push_i32(&mut buf, &mut o, left as i32);
    push_bytes(&mut buf, &mut o, b"px;top:");
    push_i32(&mut buf, &mut o, top as i32);
    push_bytes(&mut buf, &mut o, b"px;width:");
    push_i32(&mut buf, &mut o, w as i32);
    push_bytes(&mut buf, &mut o, b"px;height:");
    push_i32(&mut buf, &mut o, h as i32);
    push_bytes(&mut buf, &mut o, b"px");
    // SAFETY: only ASCII digits and fixed prefixes written above.
    intern.intern(unsafe { std::str::from_utf8_unchecked(&buf[..o]) })
}

#[allow(clippy::too_many_arguments)]
fn paint_one_cell<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    keys: &PaintKeys,
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
    set_attr_v(buf, intern, cell_id, keys.class, &cell_class);
    if stale {
        set_attr_v(buf, intern, cell_id, keys.data_stale, "1");
    }
    // Geometry as one style attr (four SetStyle → one SetAttr) before fill/font styles.
    buf.set_attr(
        cell_id,
        keys.style,
        intern_box_geometry(
            intern,
            left as f64,
            top as f64,
            item.w as f64,
            item.h as f64,
        ),
    );

    let fill = fill_paint(item.view.fill.as_ref());
    if let Some(ref fp) = fill {
        if let Some(bg) = &fp.background {
            set_style_v(buf, intern, cell_id, keys.background_color, bg);
        }
        if let Some(img) = &fp.image {
            set_style_v(buf, intern, cell_id, keys.background_image, img);
        }
    } else if item.merged {
        set_style_v(
            buf,
            intern,
            cell_id,
            keys.background_color,
            "var(--xve-sheet-bg, #fff)",
        );
    }

    if let Some(bar) = &item.view.data_bar {
        let bar_id = DATA_BAR_BASE + n as u32;
        buf.create_element(bar_id, div, cell_id);
        set_attr_v(buf, intern, bar_id, keys.class, "xg-db");
        let width = (bar.fraction.clamp(0.0, 1.0) * (item.w as f64 - 4.0)).max(0.0);
        set_style_v(buf, intern, bar_id, keys.width, &format!("{}px", width));
        let rtl = bar.direction.as_deref() == Some("rightToLeft");
        if let Some(start) = bar.start {
            set_style_v(
                buf,
                intern,
                bar_id,
                keys.left,
                &format!("{}px", 2.0 + start * (item.w as f64 - 4.0)),
            );
            set_style_v(buf, intern, bar_id, keys.right, "auto");
        } else if rtl {
            set_style_v(buf, intern, bar_id, keys.left, "auto");
            set_style_v(buf, intern, bar_id, keys.right, "2px");
        } else {
            set_style_v(buf, intern, bar_id, keys.left, "2px");
            set_style_v(buf, intern, bar_id, keys.right, "auto");
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
        set_style_v(buf, intern, bar_id, keys.background, &bg);
        if let Some(border) = &bar.border_color {
            set_style_v(
                buf,
                intern,
                bar_id,
                keys.border,
                &format!("1px solid {border}"),
            );
        } else {
            set_style_v(buf, intern, bar_id, keys.border, "none");
        }
        if let Some((frac, axis_color)) = &bar.axis {
            let axis_id = DATA_BAR_AXIS_BASE + n as u32;
            buf.create_element(axis_id, div, cell_id);
            set_attr_v(buf, intern, axis_id, keys.class, "xg-db-axis");
            set_style_v(buf, intern, axis_id, keys.position, "absolute");
            set_style_v(buf, intern, axis_id, keys.top, "1px");
            set_style_v(buf, intern, axis_id, keys.bottom, "1px");
            set_style_v(
                buf,
                intern,
                axis_id,
                keys.left,
                &format!("{}px", 2.0 + *frac * (item.w as f64 - 4.0)),
            );
            set_style_v(
                buf,
                intern,
                axis_id,
                keys.border_left,
                &format!("1px dashed {axis_color}"),
            );
        }
    }

    if let Some(icon) = &item.view.icon {
        let icon_id = ICON_BASE + n as u32;
        let size = (16.0 * (zoom / 100.0)).round().max(1.0) as u32;
        // SVG host (createElementNS via loader) + path body — keeps loader SetInnerMarkup
        // as fragment-only so the JS brotli budget stays ≤17 KB.
        let svg = keys.svg;
        buf.create_element(icon_id, svg, cell_id);
        set_attr_v(buf, intern, icon_id, keys.class, "xg-ic");
        set_attr_v(buf, intern, icon_id, keys.width, &size.to_string());
        set_attr_v(buf, intern, icon_id, keys.height, &size.to_string());
        set_attr_v(buf, intern, icon_id, keys.view_box, "0 0 16 16");
        set_attr_v(buf, intern, icon_id, keys.aria_hidden, "true");
        buf.set_inner_markup(icon_id, intern.intern(&icon_body(&icon.set, icon.index)));
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
        set_attr_v(buf, intern, text_id, keys.class, &text_class);
        // One style attr (same encode pattern as cell geometry) — cuts ~10 SetStyle
        // opcodes per text cell on the WASM cmdbuf path (#30).
        // Upstream cell-layer.ts: pad rounded; indent keeps fractional CSS px.
        // Icon paddingLeft (upstream sets after indent) wins when icon + not right.
        let pad = ((2.0 * (zoom / 100.0)).round() as i32).max(1) as f64;
        let indent = pad + item.view.indent_px * (zoom / 100.0);
        let (mut pl, pr) = if item.view.h_align == "left" || item.view.h_align == "distributed" {
            (indent, pad)
        } else if item.view.h_align == "right" {
            (pad, indent)
        } else {
            (pad, pad)
        };
        if item.view.icon.is_some() && item.view.h_align != "right" {
            pl = 20.0 * (zoom / 100.0);
        }
        let mut text_style = format!(
            "left:{}px;width:{}px;font:{};color:{};text-decoration:{};justify-content:{};align-items:{};text-align:{};padding-left:{pl}px;padding-right:{pr}px",
            item.text_left,
            item.text_width,
            css_font(&item.view.font, zoom * item.font_scale),
            item.view.font.color,
            text_decoration(&item.view.font),
            justify(&item.view.h_align),
            align_items(&item.view.v_align),
            text_align(&item.view.h_align),
        );
        // Prior port only set whiteSpace when not wrapping (xg-wrap class handles wrap).
        if !item.view.wrap {
            text_style.push_str(";white-space:pre");
        }
        buf.set_attr(text_id, keys.style, intern.intern(&text_style));
        let span_id = TEXT_BASE + 50_000 + n as u32;
        // Upstream uses <span class="xg-tx"> (inline-block via CSS).
        buf.create_element(span_id, span, text_id);
        set_attr_v(buf, intern, span_id, keys.class, "xg-tx");
        if item.view.rotation != 0.0 {
            set_style_v(
                buf,
                intern,
                span_id,
                keys.transform,
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
                set_style_v(
                    buf,
                    intern,
                    run_id,
                    keys.font,
                    &css_font(&run.font, zoom * item.font_scale),
                );
                set_style_v(buf, intern, run_id, keys.color, &run.font.color);
                set_style_v(
                    buf,
                    intern,
                    run_id,
                    keys.text_decoration,
                    &text_decoration(&run.font),
                );
                if let Some(va) = run.font.vert_align.as_deref() {
                    let css_va = if va == "super" || va == "superscript" {
                        "super"
                    } else {
                        "sub"
                    };
                    set_style_v(buf, intern, run_id, keys.vertical_align, css_va);
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
                set_style_v(buf, intern, span_id, keys.vertical_align, css_va);
            }
            let text = intern.intern(&item.text);
            buf.set_text(span_id, text);
        }
    }

    if has_borders(&item.view.borders) {
        let border_id = BORDER_BASE + n as u32;
        buf.create_element(border_id, div, cell_id);
        set_attr_v(buf, intern, border_id, keys.class, "xg-b");
        set_style_v(
            buf,
            intern,
            border_id,
            keys.width,
            &format!("{}px", item.w + 1),
        );
        set_style_v(
            buf,
            intern,
            border_id,
            keys.height,
            &format!("{}px", item.h + 1),
        );
        let b = &item.view.borders;
        if b.top.is_some() {
            set_style_v(
                buf,
                intern,
                border_id,
                keys.border_top,
                &edge_css(b.top.as_ref()),
            );
        }
        if b.right.is_some() {
            set_style_v(
                buf,
                intern,
                border_id,
                keys.border_right,
                &edge_css(b.right.as_ref()),
            );
        }
        if b.bottom.is_some() {
            set_style_v(
                buf,
                intern,
                border_id,
                keys.border_bottom,
                &edge_css(b.bottom.as_ref()),
            );
        }
        if b.left.is_some() {
            set_style_v(
                buf,
                intern,
                border_id,
                keys.border_left,
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
        set_attr_v(buf, intern, cm_id, keys.class, "xg-cm");
    }

    if item.error_mark {
        let er_id = ERROR_BASE + n as u32;
        buf.create_element(er_id, div, cell_id);
        set_attr_v(buf, intern, er_id, keys.class, "xg-er");
    }

    if stale {
        let mark = STALE_BASE + n as u32;
        buf.create_element(mark, div, cell_id);
        set_attr_v(buf, intern, mark, keys.class, "xg-stale-mark");
        set_attr_v(buf, intern, mark, keys.title, "Stale formula");
        set_style_v(buf, intern, mark, keys.position, "absolute");
        set_style_v(buf, intern, mark, keys.left, "0");
        set_style_v(buf, intern, mark, keys.bottom, "0");
        set_style_v(buf, intern, mark, keys.width, "0");
        set_style_v(buf, intern, mark, keys.height, "0");
        set_style_v(
            buf,
            intern,
            mark,
            keys.border_right,
            "8px solid transparent",
        );
        set_style_v(buf, intern, mark, keys.border_bottom, "8px solid #c45911");
        set_style_v(buf, intern, mark, keys.z_index, "3");
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
    let keys = PaintKeys::new(intern);
    let sizes = header_sizes(sheet, metrics, metrics.last_row);
    let vis = visible_cells_for_host(metrics, viewport, sizes.w, sizes.h);
    let (rows, cols) = paint_indices(&vis);
    if rows.is_empty() || cols.is_empty() {
        paint_edit_overlay(buf, intern, &keys, metrics, hints);
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
            &keys,
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
    paint_edit_overlay(buf, intern, &keys, metrics, hints);
}

fn paint_edit_overlay<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    keys: &PaintKeys,
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
    set_attr_v(buf, intern, EDITOR_BOX, keys.class, "xg-edit");
    set_style_v(buf, intern, EDITOR_BOX, keys.position, "absolute");
    set_style_v(buf, intern, EDITOR_BOX, keys.left, &format!("{left}px"));
    set_style_v(buf, intern, EDITOR_BOX, keys.top, &format!("{top}px"));
    set_style_v(buf, intern, EDITOR_BOX, keys.width, &format!("{w}px"));
    set_style_v(buf, intern, EDITOR_BOX, keys.height, &format!("{h}px"));
    set_style_v(buf, intern, EDITOR_BOX, keys.background, "#fff");
    set_style_v(buf, intern, EDITOR_BOX, keys.border, "2px solid #217346");
    set_style_v(buf, intern, EDITOR_BOX, keys.box_sizing, "border-box");
    set_style_v(buf, intern, EDITOR_BOX, keys.z_index, "6");
    set_style_v(buf, intern, EDITOR_BOX, keys.overflow, "hidden");
    set_style_v(buf, intern, EDITOR_BOX, keys.white_space, "pre");
    set_style_v(
        buf,
        intern,
        EDITOR_BOX,
        keys.font,
        "13px Calibri,Carlito,sans-serif",
    );
    set_style_v(buf, intern, EDITOR_BOX, keys.padding, "0 2px");
    let (before, after) = split_caret(&edit.text, edit.caret);
    buf.create_element(EDITOR_BEFORE, span, EDITOR_BOX);
    buf.set_text(EDITOR_BEFORE, intern.intern(before));
    buf.create_element(EDITOR_CARET, span, EDITOR_BOX);
    set_attr_v(buf, intern, EDITOR_CARET, keys.class, "xg-edit-caret");
    set_style_v(buf, intern, EDITOR_CARET, keys.display, "inline-block");
    set_style_v(buf, intern, EDITOR_CARET, keys.width, "1px");
    set_style_v(buf, intern, EDITOR_CARET, keys.height, "1em");
    set_style_v(buf, intern, EDITOR_CARET, keys.background, "#111");
    set_style_v(
        buf,
        intern,
        EDITOR_CARET,
        keys.vertical_align,
        "text-bottom",
    );
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
