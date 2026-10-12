// SPDX-License-Identifier: MIT
//! P26 raster export. M1 has no bitmap encoder; the view is the DOM stage.

use crate::presentation::Presentation;

/// Confirm the package opens, then report that PNG bytes are not produced.
pub fn export_slide_png(bytes: &[u8], slide_index: usize) -> Result<Vec<u8>, String> {
    let mut pres = Presentation::open(bytes).map_err(|err| format!("P26 raster export: {err}"))?;
    let (w, h) = pres.canvas_size();
    let _slide = pres
        .slide(slide_index)
        .map_err(|err| format!("P26 raster export: {err}"))?;
    Err(format!(
        "P26 raster export: slide {slide_index} ({w:.0}x{h:.0}) has no PNG encoder"
    ))
}
