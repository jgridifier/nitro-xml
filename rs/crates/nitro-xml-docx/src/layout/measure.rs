// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/measure.ts; changes listed in PROVENANCE.md.

use nitro_xml_render::{LayoutReadBatch, LayoutReadKind, MeasureBatch};

use super::fonts::css_font_stack;

#[derive(Clone, Debug)]
pub struct LayoutFontSpec {
    pub family: String,
    pub size_px: f64,
    pub bold: bool,
    pub italic: bool,
    pub kerning: Option<String>,
    pub ligatures: Option<String>,
    pub small_caps: bool,
}

pub trait TextMeasurer {
    fn width_of(&self, text: &str, font: &LayoutFontSpec) -> f64;
    fn line_height_of(&self, font: &LayoutFontSpec) -> f64;
    fn ascent_of(&self, _font: &LayoutFontSpec) -> Option<f64> {
        None
    }
}

pub fn font_metrics(
    measurer: &dyn TextMeasurer,
    font: &LayoutFontSpec,
) -> (f64, f64) {
    let height = measurer.line_height_of(font);
    let ascent = measurer
        .ascent_of(font)
        .map(|a| a.min(height))
        .unwrap_or(height * 0.8);
    (ascent, height - ascent)
}

#[derive(Clone)]
pub struct FakeMeasurer {
    char_width_factor: f64,
    line_height_factor: f64,
}

impl FakeMeasurer {
    pub fn new(char_width_factor: f64, line_height_factor: f64) -> Self {
        Self {
            char_width_factor,
            line_height_factor,
        }
    }
}

impl TextMeasurer for FakeMeasurer {
    fn width_of(&self, text: &str, font: &LayoutFontSpec) -> f64 {
        let mut width = 0.0;
        for ch in text.chars() {
            if ch == '\t' || ch == '\n' {
                continue;
            }
            width += font.size_px
                * self.char_width_factor
                * if font.bold { 1.08 } else { 1.0 }
                * if ch == ' ' { 0.6 } else { 1.0 };
        }
        width
    }

    fn line_height_of(&self, font: &LayoutFontSpec) -> f64 {
        font.size_px * self.line_height_factor
    }
}

pub fn create_fake_measurer() -> FakeMeasurer {
    FakeMeasurer::new(0.6, 1.15)
}

/// CSS `font` family list for batched `measureText` (matches upstream canvas measurer).
pub fn measure_font_family(font: &LayoutFontSpec) -> String {
    css_font_stack(Some(&font.family))
}

/// Queues measure requests into a [`MeasureBatch`]; ligature/small-caps widths use [`LayoutReadBatch`] span probes.
pub struct BatchedMeasurer<'a> {
    pub measures: &'a mut MeasureBatch,
    pub layout_reads: &'a mut LayoutReadBatch,
    pub fallback: FakeMeasurer,
    span_node_id: u32,
    pending_widths: std::collections::HashMap<u32, f64>,
    next_request: u32,
    span_requests: Vec<u32>,
}

impl<'a> BatchedMeasurer<'a> {
    pub fn new(
        measures: &'a mut MeasureBatch,
        layout_reads: &'a mut LayoutReadBatch,
        span_node_id: u32,
    ) -> Self {
        Self {
            measures,
            layout_reads,
            fallback: create_fake_measurer(),
            span_node_id,
            pending_widths: std::collections::HashMap::new(),
            next_request: 0,
            span_requests: Vec::new(),
        }
    }

    pub fn set_span_width(&mut self, request_id: u32, width: f64) {
        self.pending_widths.insert(request_id, width);
    }

    /// Apply [`LayoutReadBatch`] span widths (lane 2 = width) after the loader answers the batch.
    pub fn apply_span_results(&mut self, values: &[f64]) {
        for (index, request_id) in self.span_requests.iter().enumerate() {
            let base = index * nitro_xml_render::LAYOUT_RESULT_LANES;
            if base + 2 < values.len() {
                self.pending_widths.insert(*request_id, values[base + 2]);
            }
        }
        self.span_requests.clear();
    }
}

impl TextMeasurer for BatchedMeasurer<'_> {
    fn width_of(&self, text: &str, font: &LayoutFontSpec) -> f64 {
        if font.ligatures.is_some() || font.small_caps {
            let id = self.span_requests.last();
            if let Some(id) = id {
                if let Some(w) = self.pending_widths.get(id) {
                    return *w;
                }
            }
        }
        self.fallback.width_of(text, font)
    }

    fn line_height_of(&self, font: &LayoutFontSpec) -> f64 {
        self.fallback.line_height_of(font)
    }
}

impl BatchedMeasurer<'_> {
    pub fn queue_width(
        &mut self,
        text: &str,
        font: &LayoutFontSpec,
        font_id: u32,
        text_id: u32,
    ) -> f64 {
        let _family = measure_font_family(font);
        if font.ligatures.is_some() || font.small_caps {
            let id = self.next_request;
            self.next_request += 1;
            self.layout_reads.push(self.span_node_id, LayoutReadKind::SpanRect, 0);
            self.span_requests.push(id);
            return self
                .pending_widths
                .get(&id)
                .copied()
                .unwrap_or_else(|| self.fallback.width_of(text, font));
        }
        self.measures.push(font_id, font.size_px as f32, text_id);
        self.fallback.width_of(text, font)
    }
}
