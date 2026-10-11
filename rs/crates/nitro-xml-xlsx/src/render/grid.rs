// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/grid-view.ts (frame/quadrant shell);
// changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::render::GridPaintViewport;
use crate::view::metrics::GridMetrics;

pub fn paint_frame(buf: &mut CommandBuffer, viewport: &GridPaintViewport, metrics: &GridMetrics) {
    let _ = (viewport, metrics);
    // TODO(#19): quadrant transforms, scroll extent, freeze lines (X1).
    buf.create_element(1, 1, 0);
}
