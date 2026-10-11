// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/gridlines.ts; changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::model::ActiveWorksheet;
use crate::render::GridPaintViewport;
use crate::view::metrics::GridMetrics;

pub fn paint(
    buf: &mut CommandBuffer,
    _sheet: &ActiveWorksheet,
    _metrics: &GridMetrics,
    viewport: &GridPaintViewport,
) {
    buf.stroke_rect(
        -viewport.scroll_x,
        -viewport.scroll_y,
        viewport.width,
        viewport.height,
        0xffcccccc,
        1.0,
    );
}
