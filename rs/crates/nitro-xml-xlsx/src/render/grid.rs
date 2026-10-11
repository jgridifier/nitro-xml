// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/grid-view.ts (frame/quadrant shell);
// changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::intern::StringIntern;
use crate::model::ActiveWorksheet;
use crate::render::geometry::header_sizes;
use crate::render::GridPaintViewport;
use crate::view::metrics::GridMetrics;

/// Root node every painter attaches under.
pub const ROOT_ID: u32 = 1;
/// Main (unfrozen) quadrant box — clipped at the header inset.
pub const Q_MAIN_ID: u32 = 10;
/// Scroll plane (`translate3d(-scroll)`) hosting lines/cells/overlay.
pub const PLANE_ID: u32 = 11;
pub const LINES_ID: u32 = 12;
pub const CELLS_ID: u32 = 13;
pub const OVERLAY_ID: u32 = 14;

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
    for (prop, val) in [
        ("position", "relative"),
        ("width", "100%"),
        ("height", "100%"),
        ("overflow", "hidden"),
        ("outline", "none"),
        ("contain", "strict"),
        ("userSelect", "none"),
        ("WebkitUserSelect", "none"),
    ] {
        set_style(buf, intern, ROOT_ID, prop, val);
    }

    let sizes = header_sizes(sheet, metrics, metrics.last_row);
    let cells_w = (viewport.width - sizes.w).max(0);
    let cells_h = (viewport.height - sizes.h).max(0);

    // Main quadrant at the header inset; plane scrolls via translate3d (will-change in CSS).
    buf.create_element(Q_MAIN_ID, div, ROOT_ID);
    set_attr(buf, intern, Q_MAIN_ID, "class", "xg-q xg-q-main");
    place(buf, intern, Q_MAIN_ID, sizes.w, sizes.h, cells_w, cells_h);

    buf.create_element(PLANE_ID, div, Q_MAIN_ID);
    set_attr(buf, intern, PLANE_ID, "class", "xg-plane");
    set_style(
        buf,
        intern,
        PLANE_ID,
        "transform",
        &format!(
            "translate3d({}px,{}px,0)",
            -viewport.scroll_x, -viewport.scroll_y
        ),
    );

    buf.create_element(LINES_ID, div, PLANE_ID);
    set_attr(buf, intern, LINES_ID, "class", "xg-lines");
    buf.create_element(CELLS_ID, div, PLANE_ID);
    set_attr(buf, intern, CELLS_ID, "class", "xg-cells");
    buf.create_element(OVERLAY_ID, div, PLANE_ID);
    set_attr(buf, intern, OVERLAY_ID, "class", "xg-overlay");
}
