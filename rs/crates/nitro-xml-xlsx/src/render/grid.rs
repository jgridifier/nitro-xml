// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/grid-view.ts (frame/quadrant shell);
// changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::intern::StringIntern;
use crate::render::GridPaintViewport;
use crate::view::metrics::GridMetrics;

pub fn paint_frame<I: StringIntern>(
    buf: &mut CommandBuffer,
    intern: &mut I,
    viewport: &GridPaintViewport,
    metrics: &GridMetrics,
) {
    let _ = (viewport, metrics);
    let div = intern.intern("div");
    buf.create_element(1, div, 0);
    for (prop, val) in [
        ("position", "relative"),
        ("width", "100%"),
        ("height", "100%"),
        ("overflow", "hidden"),
        ("background", "#fff"),
        ("color", "#000"),
        ("fontFamily", "\"Segoe UI\", system-ui, sans-serif"),
    ] {
        let p = intern.intern(prop);
        let v = intern.intern(val);
        buf.set_style(1, p, v);
    }
}
