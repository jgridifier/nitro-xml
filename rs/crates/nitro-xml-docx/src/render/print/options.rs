// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/docx/print-layout.ts (`PrintLayoutOptions`);
// changes listed in PROVENANCE.md.

use std::collections::{HashMap, HashSet};

use nitro_xml_opc::chart::ChartSpace;

use super::line_numbers::PrintLineNumbering;
use super::page_borders::PageBorders;
use crate::body::DocumentModel;
use crate::review_formatting_display::ReviewDisplayMode;

/// Wall-clock parts for DATE/TIME fields in headers and footers.
#[derive(Clone, Copy, Debug)]
pub struct PrintNow {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

impl Default for PrintNow {
    fn default() -> Self {
        Self {
            year: 1980,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
        }
    }
}

/// Inputs besides [`crate::LayoutResult`] that Print Layout paints (headers, borders, pictures).
pub struct PrintLayoutOptions<'a> {
    pub line_numbers: &'a [Option<PrintLineNumbering>],
    pub page_borders: &'a [Option<PageBorders>],
    pub suppressed_line_number_paragraphs: &'a HashSet<String>,
    pub picture_urls: &'a HashMap<String, String>,
    pub chart_spaces: &'a HashMap<String, ChartSpace>,
    pub chart_color_palettes: &'a HashMap<String, i32>,
    pub model: Option<&'a DocumentModel>,
    pub review_mode: ReviewDisplayMode,
    pub now: PrintNow,
    pub page_color: Option<&'a str>,
    /// CSS `zoom` on the page stack (1 = 100%).
    pub zoom: f64,
    /// Page index marked `data-nitro-current` for shell page navigation.
    pub current_page: Option<usize>,
    /// Measured chart SVG keyed by part name and pixel size.
    pub chart_svg_cache: &'a HashMap<String, String>,
}
