// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/core/runtime/PptxGraphicFrameParser.ts; changes in PROVENANCE.md.

use nitro_xml_opc::xml::{ns, Element};

use crate::inheritance::children_local;
pub fn table_row_count(table: &Element) -> usize {
    children_local(table, "tr", ns::A).len()
}
