// SPDX-License-Identifier: MIT
//! Text width for grid layout (canvas batch or estimate).

use crate::intern::StringIntern;
use crate::view::fake_measure::{estimate_width_sized, fake_xlsx_measure, font_size_px};
use nitro_xml_render::measure::MeasureBatch;

pub trait TextMeasure {
    fn measure(&mut self, text: &str, font: &str) -> f64;
}

pub struct FnTextMeasure<'a> {
    f: &'a dyn Fn(&str, &str) -> f64,
}

impl<'a> FnTextMeasure<'a> {
    pub fn new(f: &'a dyn Fn(&str, &str) -> f64) -> Self {
        Self { f }
    }
}

impl TextMeasure for FnTextMeasure<'_> {
    fn measure(&mut self, text: &str, font: &str) -> f64 {
        (self.f)(text, font)
    }
}

pub struct FakeTextMeasure;

impl TextMeasure for FakeTextMeasure {
    fn measure(&mut self, text: &str, _font: &str) -> f64 {
        fake_xlsx_measure(text)
    }
}

pub struct CanvasTextMeasure<'a, I: StringIntern> {
    intern: &'a mut I,
    batch: &'a mut MeasureBatch,
    /// Last font CSS + id — cells share Calibri/11px; skip StringTable re-hash (#30).
    last_font: String,
    last_font_id: u32,
    /// Parsed once when `last_font` changes — avoid per-cell `find("px")` / `contains("bold")`.
    last_size_px: f64,
    last_bold: bool,
}

impl<'a, I: StringIntern> CanvasTextMeasure<'a, I> {
    pub fn new(intern: &'a mut I, batch: &'a mut MeasureBatch) -> Self {
        Self {
            intern,
            batch,
            last_font: String::new(),
            last_font_id: 0,
            last_size_px: 14.67,
            last_bold: false,
        }
    }
}

impl<I: StringIntern> TextMeasure for CanvasTextMeasure<'_, I> {
    fn measure(&mut self, text: &str, font: &str) -> f64 {
        if text.is_empty() {
            return 0.0;
        }
        let font_id = if font == self.last_font {
            self.last_font_id
        } else {
            let id = self.intern.intern(font);
            self.last_font.clear();
            self.last_font.push_str(font);
            self.last_font_id = id;
            self.last_size_px = font_size_px(font);
            // Same predicate as upstream estimateWidth (`font.includes("bold")`).
            self.last_bold = font.contains("bold");
            id
        };
        let text_id = self.intern.intern(text);
        if let Some(w) = self.batch.cached_width(font_id, text_id) {
            return w;
        }
        let _ = self
            .batch
            .push_unique(font_id, self.last_size_px as f32, text_id);
        estimate_width_sized(text, self.last_size_px, self.last_bold)
    }
}
