// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/drawingml/drawing-color-css.ts

use nitro_xml_opc::drawingml::ResolvedDrawingColor;

pub fn drawing_color_css(value: Option<&ResolvedDrawingColor>) -> Option<String> {
    let value = value?;
    if value.alpha == 1.0 {
        return Some(value.hex.clone());
    }
    let hex = value.hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Some(value.hex.clone());
    }
    let r = i64::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = i64::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = i64::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    Some(format!("rgba({},{},{},{})", r, g, b, value.alpha))
}
