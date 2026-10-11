// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/selection-layer.ts; changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::render::GridPaintViewport;

pub fn paint(_buf: &mut CommandBuffer, _viewport: &GridPaintViewport) {
    // TODO(#18 X6): selection overlay when editor session exists.
}
