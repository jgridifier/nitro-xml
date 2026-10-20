// SPDX-License-Identifier: MIT
//! Text width for grid layout (canvas batch or estimate).

use crate::intern::StringIntern;
use crate::view::fake_measure::{estimate_width, fake_xlsx_measure};
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
}

impl<'a, I: StringIntern> CanvasTextMeasure<'a, I> {
    pub fn new(intern: &'a mut I, batch: &'a mut MeasureBatch) -> Self {
        Self {
            intern,
            batch,
            last_font: String::new(),
            last_font_id: 0,
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
            id
        };
        let text_id = self.intern.intern(text);
        if let Some(w) = self.batch.cached_width(font_id, text_id) {
            return w;
        }
        let _ = self.batch.push_unique(font_id, 0.0, text_id);
        estimate_width(text, font)
    }
}
