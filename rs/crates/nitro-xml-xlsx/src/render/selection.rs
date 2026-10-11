// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/grid/selection-layer.ts; changes listed in PROVENANCE.md.

use nitro_xml_render::command_buffer::CommandBuffer;

use crate::render::GridPaintViewport;

pub fn paint(_buf: &mut CommandBuffer, _viewport: &GridPaintViewport) {
    // TODO(#150): selection overlay when viewport exposes active selection.
    // TODO(#29): sheet chrome selection sync with tabs/status shell.
}
