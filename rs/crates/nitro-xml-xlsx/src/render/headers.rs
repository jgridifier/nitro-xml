// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/headers.ts; changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::model::ActiveWorksheet;
use crate::render::GridPaintViewport;
use crate::view::metrics::GridMetrics;

pub fn paint(
    _buf: &mut CommandBuffer,
    _sheet: &ActiveWorksheet,
    _metrics: &GridMetrics,
    _viewport: &GridPaintViewport,
) {
    // TODO(#18): row/column header chrome (X5).
}
