// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/grid-view.ts (frame/quadrant shell)
// and grid-a11y.ts (role/aria counts + live region); changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::address::{MAX_COL, MAX_ROW};
use crate::intern::StringIntern;
use crate::model::ActiveWorksheet;
use crate::render::geometry::header_sizes;
use crate::render::paint_encode::{
    intern_translate3d, place_style_attr, set_attr_v, set_style_v, ChromeKeys,
};
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

/// `xg-root` + main quadrant plane (upstream grid-view.ts, freeze panes TODO).
pub fn paint_frame<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    sheet: &ActiveWorksheet,
    viewport: &GridPaintViewport,
    metrics: &GridMetrics,
) {
    let div = intern.intern("div");
    let keys = ChromeKeys::new(intern);
    buf.create_element(ROOT_ID, div, 0);
    buf.set_attr(ROOT_ID, keys.class, intern.intern("xg-root"));
    // Upstream grid-view.ts + grid-a11y.ts (#42).
    buf.set_attr(ROOT_ID, keys.role, intern.intern("grid"));
    buf.set_attr(ROOT_ID, keys.part, intern.intern("grid"));
    set_attr_v(
        buf,
        intern,
        ROOT_ID,
        keys.aria_rowcount,
        &(MAX_ROW + 1).to_string(),
    );
    set_attr_v(
        buf,
        intern,
        ROOT_ID,
        keys.aria_colcount,
        &(MAX_COL + 1).to_string(),
    );
    buf.set_attr(ROOT_ID, keys.aria_multiselectable, intern.intern("true"));
    buf.set_attr(ROOT_ID, keys.aria_label, intern.intern("Spreadsheet grid"));
    for (prop, val) in [
        (keys.position, "relative"),
        (keys.width, "100%"),
        (keys.height, "100%"),
        (keys.overflow, "hidden"),
        (keys.outline, "none"),
        (keys.contain, "strict"),
        (keys.user_select, "none"),
        (keys.webkit_user_select, "none"),
        // Upstream shell.css :host `font: 13px/1.35` — unitless LH inherits into .xg-hd.
        (keys.font_size, "13px"),
        (keys.line_height, "1.35"),
        (keys.font_family, r#""Segoe UI", Arial, sans-serif"#),
    ] {
        set_style_v(buf, intern, ROOT_ID, prop, val);
    }

    // Polite status region; selection text is filled by the editor surface (E-text).
    buf.create_element(LIVE_ID, div, ROOT_ID);
    buf.set_attr(LIVE_ID, keys.class, intern.intern("xg-live"));
    buf.set_attr(LIVE_ID, keys.role, intern.intern("status"));
    buf.set_attr(LIVE_ID, keys.aria_live, intern.intern("polite"));

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

    let class_q_top = intern.intern("xg-q xg-q-top");
    let class_plane = intern.intern("xg-plane");
    let class_lines = intern.intern("xg-lines");
    let class_cells = intern.intern("xg-cells");
    let class_overlay = intern.intern("xg-overlay");
    let class_q_main = intern.intern("xg-q xg-q-main");
    let class_freeze_h = intern.intern("xg-freeze-h");
    let class_freeze_v = intern.intern("xg-freeze-v");

    // Frozen-rows quadrant (upstream `xg-q-top`); plane does not scroll vertically.
    if frozen_h > 0 {
        buf.create_element(Q_TOP_ID, div, ROOT_ID);
        buf.set_attr(Q_TOP_ID, keys.class, class_q_top);
        place_style_attr(
            buf, intern, &keys, Q_TOP_ID, sizes.w, sizes.h, cells_w, frozen_h,
        );

        buf.create_element(PLANE_TOP_ID, div, Q_TOP_ID);
        buf.set_attr(PLANE_TOP_ID, keys.class, class_plane);
        buf.set_style(
            PLANE_TOP_ID,
            keys.transform,
            intern_translate3d(intern, -viewport.scroll_x, 0),
        );
        buf.create_element(LINES_TOP_ID, div, PLANE_TOP_ID);
        buf.set_attr(LINES_TOP_ID, keys.class, class_lines);
        buf.create_element(CELLS_TOP_ID, div, PLANE_TOP_ID);
        buf.set_attr(CELLS_TOP_ID, keys.class, class_cells);
        buf.create_element(OVERLAY_TOP_ID, div, PLANE_TOP_ID);
        buf.set_attr(OVERLAY_TOP_ID, keys.class, class_overlay);
    }

    // Main (scrolling) quadrant below the freeze band.
    buf.create_element(Q_MAIN_ID, div, ROOT_ID);
    buf.set_attr(Q_MAIN_ID, keys.class, class_q_main);
    place_style_attr(
        buf, intern, &keys, Q_MAIN_ID, sizes.w, main_top, cells_w, main_h,
    );

    buf.create_element(PLANE_ID, div, Q_MAIN_ID);
    buf.set_attr(PLANE_ID, keys.class, class_plane);
    // Upstream geometry.origin('main'): ox = frozenW+scrollLeft, oy = frozenH+scrollTop.
    let frozen_w = if viewport.frozen_cols > 0 {
        metrics.col_left(viewport.frozen_cols)
    } else {
        0
    };
    buf.set_style(
        PLANE_ID,
        keys.transform,
        intern_translate3d(
            intern,
            -(frozen_w + viewport.scroll_x),
            -(frozen_h + viewport.scroll_y),
        ),
    );

    buf.create_element(LINES_ID, div, PLANE_ID);
    buf.set_attr(LINES_ID, keys.class, class_lines);
    buf.create_element(CELLS_ID, div, PLANE_ID);
    buf.set_attr(CELLS_ID, keys.class, class_cells);
    buf.create_element(OVERLAY_ID, div, PLANE_ID);
    buf.set_attr(OVERLAY_ID, keys.class, class_overlay);

    // pane-chrome.ts freeze dividers (view space, not plane).
    if viewport.frozen_rows > 0 {
        let frozen_h_full = metrics.row_top(viewport.frozen_rows);
        buf.create_element(FREEZE_H_ID, div, ROOT_ID);
        buf.set_attr(FREEZE_H_ID, keys.class, class_freeze_h);
        place_style_attr(
            buf,
            intern,
            &keys,
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
        buf.set_attr(FREEZE_V_ID, keys.class, class_freeze_v);
        place_style_attr(
            buf,
            intern,
            &keys,
            FREEZE_V_ID,
            sizes.w + frozen_w - 1,
            0,
            1,
            viewport.height,
        );
    }
}
