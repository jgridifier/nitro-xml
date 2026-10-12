// SPDX-License-Identifier: MIT
//! P25 print. M1 paints the slide as a DOM stage; there is no print document.

use crate::presentation::Presentation;

/// Confirm the package opens, then report that print HTML is not produced.
pub fn print_slide_html(bytes: &[u8], slide_index: usize) -> Result<String, String> {
    let mut pres = Presentation::open(bytes).map_err(|err| format!("P25 print: {err}"))?;
    let (w, h) = pres.canvas_size();
    let _slide = pres
        .slide(slide_index)
        .map_err(|err| format!("P25 print: {err}"))?;
    Err(format!(
        "P25 print: slide {slide_index} ({w:.0}x{h:.0}) is a DOM stage; no print HTML serializer"
    ))
}
