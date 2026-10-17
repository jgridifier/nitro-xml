// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/grid-view.ts (frame/quadrant shell)
// and grid-a11y.ts (role/aria counts + live region); changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::address::{MAX_COL, MAX_ROW};
use crate::intern::StringIntern;
use crate::model::ActiveWorksheet;
use crate::render::geometry::header_sizes;
use crate::render::GridPaintViewport;
use crate::view::metrics::GridMetrics;

/// Root node every painter attaches under.
pub const ROOT_ID: u32 = 1;
/// Polite live region for selection announcements (upstream `grid-a11y.ts`).
pub const LIVE_ID: u32 = 3;
/// Main (unfrozen) quadrant box — clipped at the header inset.
pub const Q_MAIN_ID: u32 = 10;
/// Scroll plane (`translate3d(-scroll)`) hosting lines/cells/overlay.
pub const PLANE_ID: u32 = 11;
pub const LINES_ID: u32 = 12;
pub const CELLS_ID: u32 = 13;
pub const OVERLAY_ID: u32 = 14;
/// Drawings layer on the main plane (upstream `xg-drawings`).
pub const DRAWINGS_ID: u32 = 17;
const FREEZE_H_ID: u32 = 15;
const FREEZE_V_ID: u32 = 16;
/// Frozen-rows quadrant (`xg-q-top`) and its plane/layers (upstream grid-view.ts).
pub const Q_TOP_ID: u32 = 20;
pub const PLANE_TOP_ID: u32 = 21;
pub const LINES_TOP_ID: u32 = 22;
pub const CELLS_TOP_ID: u32 = 23;
pub const OVERLAY_TOP_ID: u32 = 24;

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

fn place<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    id: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    set_style(buf, intern, id, "left", &format!("{x}px"));
    set_style(buf, intern, id, "top", &format!("{y}px"));
    set_style(buf, intern, id, "width", &format!("{w}px"));
    set_style(buf, intern, id, "height", &format!("{h}px"));
}

/// `xg-root` + main quadrant plane (upstream grid-view.ts, freeze panes TODO).
pub fn paint_frame<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    sheet: &ActiveWorksheet,
    viewport: &GridPaintViewport,
    metrics: &GridMetrics,
) {
    let div = intern.intern("div");
    buf.create_element(ROOT_ID, div, 0);
    set_attr(buf, intern, ROOT_ID, "class", "xg-root");
    // Upstream grid-view.ts + grid-a11y.ts (#42).
    set_attr(buf, intern, ROOT_ID, "role", "grid");
    set_attr(buf, intern, ROOT_ID, "part", "grid");
    set_attr(
        buf,
        intern,
        ROOT_ID,
        "aria-rowcount",
        &(MAX_ROW + 1).to_string(),
    );
    set_attr(
        buf,
        intern,
        ROOT_ID,
        "aria-colcount",
        &(MAX_COL + 1).to_string(),
    );
    set_attr(buf, intern, ROOT_ID, "aria-multiselectable", "true");
    set_attr(buf, intern, ROOT_ID, "aria-label", "Spreadsheet grid");
    for (prop, val) in [
        ("position", "relative"),
        ("width", "100%"),
        ("height", "100%"),
        ("overflow", "hidden"),
        ("outline", "none"),
        ("contain", "strict"),
        ("userSelect", "none"),
        ("WebkitUserSelect", "none"),
        // Upstream shell.css :host `font: 13px/1.35` — unitless LH inherits into .xg-hd.
        ("fontSize", "13px"),
        ("lineHeight", "1.35"),
        ("fontFamily", r#""Segoe UI", Arial, sans-serif"#),
    ] {
        set_style(buf, intern, ROOT_ID, prop, val);
    }

    // Polite status region; selection text is filled by the editor surface (E-text).
    buf.create_element(LIVE_ID, div, ROOT_ID);
    set_attr(buf, intern, LIVE_ID, "class", "xg-live");
    set_attr(buf, intern, LIVE_ID, "role", "status");
    set_attr(buf, intern, LIVE_ID, "aria-live", "polite");

    let sizes = header_sizes(sheet, metrics, metrics.last_row);
    let cells_w = (viewport.width - sizes.w).max(0);
    let cells_h = (viewport.height - sizes.h).max(0);
    let frozen_h = if viewport.frozen_rows > 0 {
        metrics.row_top(viewport.frozen_rows).min(cells_h)
    } else {
        0
    };
    let main_top = sizes.h + frozen_h;
    let main_h = (cells_h - frozen_h).max(0);

    // Frozen-rows quadrant (upstream `xg-q-top`); plane does not scroll vertically.
    if frozen_h > 0 {
        buf.create_element(Q_TOP_ID, div, ROOT_ID);
        set_attr(buf, intern, Q_TOP_ID, "class", "xg-q xg-q-top");
        place(buf, intern, Q_TOP_ID, sizes.w, sizes.h, cells_w, frozen_h);

        buf.create_element(PLANE_TOP_ID, div, Q_TOP_ID);
        set_attr(buf, intern, PLANE_TOP_ID, "class", "xg-plane");
        set_style(
            buf,
            intern,
            PLANE_TOP_ID,
            "transform",
            &format!("translate3d({}px,0px,0)", -viewport.scroll_x),
        );
        buf.create_element(LINES_TOP_ID, div, PLANE_TOP_ID);
        set_attr(buf, intern, LINES_TOP_ID, "class", "xg-lines");
        buf.create_element(CELLS_TOP_ID, div, PLANE_TOP_ID);
        set_attr(buf, intern, CELLS_TOP_ID, "class", "xg-cells");
        buf.create_element(OVERLAY_TOP_ID, div, PLANE_TOP_ID);
        set_attr(buf, intern, OVERLAY_TOP_ID, "class", "xg-overlay");
    }

    // Main (scrolling) quadrant below the freeze band.
    buf.create_element(Q_MAIN_ID, div, ROOT_ID);
    set_attr(buf, intern, Q_MAIN_ID, "class", "xg-q xg-q-main");
    place(buf, intern, Q_MAIN_ID, sizes.w, main_top, cells_w, main_h);

    buf.create_element(PLANE_ID, div, Q_MAIN_ID);
    set_attr(buf, intern, PLANE_ID, "class", "xg-plane");
    // Upstream geometry.origin('main'): ox = frozenW+scrollLeft, oy = frozenH+scrollTop.
    let frozen_w = if viewport.frozen_cols > 0 {
        metrics.col_left(viewport.frozen_cols)
    } else {
        0
    };
    set_style(
        buf,
        intern,
        PLANE_ID,
        "transform",
        &format!(
            "translate3d({}px,{}px,0)",
            -(frozen_w + viewport.scroll_x),
            -(frozen_h + viewport.scroll_y)
        ),
    );

    buf.create_element(LINES_ID, div, PLANE_ID);
    set_attr(buf, intern, LINES_ID, "class", "xg-lines");
    buf.create_element(CELLS_ID, div, PLANE_ID);
    set_attr(buf, intern, CELLS_ID, "class", "xg-cells");
    buf.create_element(OVERLAY_ID, div, PLANE_ID);
    set_attr(buf, intern, OVERLAY_ID, "class", "xg-overlay");

    // pane-chrome.ts freeze dividers (view space, not plane).
    if viewport.frozen_rows > 0 {
        let frozen_h_full = metrics.row_top(viewport.frozen_rows);
        buf.create_element(FREEZE_H_ID, div, ROOT_ID);
        set_attr(buf, intern, FREEZE_H_ID, "class", "xg-freeze-h");
        place(
            buf,
            intern,
            FREEZE_H_ID,
            0,
            sizes.h + frozen_h_full - 1,
            viewport.width,
            1,
        );
    }
    if viewport.frozen_cols > 0 {
        let frozen_w = metrics.col_left(viewport.frozen_cols);
        buf.create_element(FREEZE_V_ID, div, ROOT_ID);
        set_attr(buf, intern, FREEZE_V_ID, "class", "xg-freeze-v");
        place(
            buf,
            intern,
            FREEZE_V_ID,
            sizes.w + frozen_w - 1,
            0,
            1,
            viewport.height,
        );
    }
}
