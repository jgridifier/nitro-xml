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
    // TODO(#19): quadrant transforms, scroll extent, freeze lines (X1).
    let div = intern.intern("div");
    buf.create_element(1, div, 0);
}
