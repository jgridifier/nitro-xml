// SPDX-License-Identifier: MIT
//! P27 SVG export. M1 paints DOM opcodes, not a standalone SVG document.

use crate::presentation::Presentation;

/// Confirm the package opens, then report that an SVG document is not produced.
pub fn export_slide_svg(bytes: &[u8], slide_index: usize) -> Result<String, String> {
    let mut pres = Presentation::open(bytes).map_err(|err| format!("P27 SVG export: {err}"))?;
    let (w, h) = pres.canvas_size();
    let _slide = pres
        .slide(slide_index)
        .map_err(|err| format!("P27 SVG export: {err}"))?;
    Err(format!(
        "P27 SVG export: slide {slide_index} ({w:.0}x{h:.0}) is a DOM stage, not an SVG document"
    ))
}
