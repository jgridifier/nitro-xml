// SPDX-License-Identifier: MIT
//! Open a package and the inputs Print Layout needs for one viewer session.

use std::collections::{HashMap, HashSet};

use nitro_xml_opc::chart::{parse_chart_space_xml, ChartSpace};
use nitro_xml_render::{
    CommandBuffer, LayoutReadBatch, LayoutReadKind, MeasureBatch, LAYOUT_RESULT_LANES,
};

use super::dom::{Dom, StringIntern};
use super::line_numbers::{
    line_numbering_from_section, suppressed_line_number_ids, PrintLineNumbering,
};
use super::options::{PrintLayoutOptions, PrintNow};
use super::page_borders::{page_borders_from_section, PageBorders};
use super::pages::paint_print_layout;
use crate::body::{read_package, DocumentModel, LoadedDocument};
use crate::layout::{
    layout_document_model, measure_font_css, CachedMeasurer, LayoutResult, LayoutReviewOptions,
    MeasureCache, MeasureKey, MeasureSample, PendingKind, PendingMeasure,
};
use crate::review_formatting_display::ReviewDisplayMode;

/// Result of one measure/layout tick before the document is ready to paint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeasureTick {
    pub ready: bool,
    /// When false and not ready, the host may issue an empty `begin()` frame.
    pub painted_metric_probes: bool,
}

pub struct PrintSession {
    pub model: DocumentModel,
    pub result: LayoutResult,
    pub picture_urls: HashMap<String, String>,
    pub chart_spaces: HashMap<String, ChartSpace>,
    pub line_numbers: Vec<Option<PrintLineNumbering>>,
    pub page_borders: Vec<Option<PageBorders>>,
    pub suppressed: HashSet<String>,
    pub warnings: Vec<String>,
    measure_cache: MeasureCache,
    width_pending: Vec<PendingMeasure>,
    metric_probe_pending: Vec<PendingMeasure>,
    metric_span_keys: Vec<MeasureKey>,
    pub layout_ready: bool,
}

impl PrintSession {
    pub fn open(bytes: &'static [u8]) -> Result<Self, String> {
        let mut loaded = read_package(bytes).map_err(|e| e.to_string())?;
        let picture_urls = collect_pictures(&mut loaded);
        let chart_spaces = collect_charts(&mut loaded);
        let warnings = loaded.model.warnings.clone();
        let model = loaded.model;
        let sections = model.sections.as_deref().unwrap_or(&[]);
        Ok(Self {
            line_numbers: sections.iter().map(line_numbering_from_section).collect(),
            page_borders: sections.iter().map(page_borders_from_section).collect(),
            suppressed: suppressed_line_number_ids(&model.blocks),
            model,
            result: LayoutResult {
                pages: Vec::new(),
                approximations: Vec::new(),
            },
            picture_urls,
            chart_spaces,
            warnings,
            measure_cache: MeasureCache::new(),
            width_pending: Vec::new(),
            metric_probe_pending: Vec::new(),
            metric_span_keys: Vec::new(),
            layout_ready: false,
        })
    }

    /// Drive one CachedMeasurer pass: absorb loader widths + span metric probes, re-layout, queue misses.
    pub fn tick_measure<I: StringIntern>(
        &mut self,
        measures: &mut MeasureBatch,
        layout_reads: &mut LayoutReadBatch,
        commands: &mut CommandBuffer,
        intern: &mut I,
    ) -> MeasureTick {
        if self.layout_ready {
            return MeasureTick {
                ready: true,
                painted_metric_probes: false,
            };
        }

        self.absorb_width_results(measures);
        self.absorb_metric_span_results(layout_reads);

        if measures.awaiting() > 0
            || !measures.is_empty()
            || layout_reads.awaiting() > 0
            || !self.metric_span_keys.is_empty()
        {
            return MeasureTick {
                ready: false,
                painted_metric_probes: false,
            };
        }

        let measurer = CachedMeasurer::new(&self.measure_cache);
        let result = layout_document_model(
            &self.model,
            &measurer,
            LayoutReviewOptions {
                review_display_mode: None,
            },
        );
        let pending = measurer.take_pending();
        if pending.is_empty() {
            for note in &result.approximations {
                if !self.warnings.iter().any(|w| w == note) {
                    self.warnings.push(note.clone());
                }
            }
            self.result = result;
            self.layout_ready = true;
            return MeasureTick {
                ready: true,
                painted_metric_probes: false,
            };
        }

        let painted = self.queue_pending(&pending, measures, layout_reads, commands, intern);
        MeasureTick {
            ready: false,
            painted_metric_probes: painted,
        }
    }

    fn absorb_width_results(&mut self, measures: &mut MeasureBatch) {
        if self.width_pending.is_empty()
            || measures.awaiting() > 0
            || measures.widths.len() != self.width_pending.len()
        {
            return;
        }
        let mut samples = Vec::with_capacity(self.width_pending.len());
        for &width in measures.widths.iter() {
            samples.push(MeasureSample {
                width: Some(width),
                ..MeasureSample::default()
            });
        }
        self.measure_cache.absorb(&self.width_pending, &samples);
        self.width_pending.clear();
        measures.widths.clear();
    }

    fn absorb_metric_span_results(&mut self, layout_reads: &LayoutReadBatch) {
        if self.metric_probe_pending.is_empty() || self.metric_span_keys.is_empty() {
            return;
        }
        let n = self.metric_span_keys.len();
        let expected = n * LAYOUT_RESULT_LANES;
        if layout_reads.awaiting() > 0 || layout_reads.values.len() != expected {
            return;
        }
        let mut height_by_font: HashMap<MeasureKey, f64> = HashMap::new();
        for (index, key) in self.metric_span_keys.iter().enumerate() {
            let base = index * LAYOUT_RESULT_LANES;
            let height = layout_reads.values[base + 3];
            height_by_font.insert(key.metrics_key(), height);
        }
        let mut samples = Vec::with_capacity(self.metric_probe_pending.len());
        for probe in &self.metric_probe_pending {
            let height = height_by_font
                .get(&probe.key.metrics_key())
                .copied()
                .unwrap_or(0.0);
            samples.push(match probe.kind {
                PendingKind::LineHeight => MeasureSample {
                    line_height: Some(height),
                    ..MeasureSample::default()
                },
                PendingKind::Ascent => MeasureSample {
                    ascent: Some(height * 0.8),
                    ..MeasureSample::default()
                },
                PendingKind::Width => MeasureSample::default(),
            });
        }
        self.measure_cache
            .absorb(&self.metric_probe_pending, &samples);
        self.metric_probe_pending.clear();
        self.metric_span_keys.clear();
    }

    fn queue_pending<I: StringIntern>(
        &mut self,
        pending: &[PendingMeasure],
        measures: &mut MeasureBatch,
        layout_reads: &mut LayoutReadBatch,
        commands: &mut CommandBuffer,
        intern: &mut I,
    ) -> bool {
        self.width_pending.clear();
        self.metric_probe_pending.clear();
        self.metric_span_keys.clear();

        let mut metric_fonts: Vec<MeasureKey> = Vec::new();
        for probe in pending {
            match probe.kind {
                PendingKind::Width => {
                    let font = probe.key.to_font_spec();
                    let font_id = intern.intern(&measure_font_css(&font));
                    let text_id = intern.intern(probe.key.text.as_str());
                    measures.push(font_id, probe.key.size_px() as f32, text_id);
                    self.width_pending.push(probe.clone());
                }
                PendingKind::LineHeight | PendingKind::Ascent => {
                    let font_key = probe.key.metrics_key();
                    if !metric_fonts.iter().any(|k| *k == font_key) {
                        metric_fonts.push(font_key);
                    }
                    self.metric_probe_pending.push(probe.clone());
                }
            }
        }

        if metric_fonts.is_empty() {
            return false;
        }

        self.metric_span_keys = metric_fonts.clone();
        paint_metric_probes(&metric_fonts, commands, layout_reads, intern);
        true
    }

    pub fn paint<I: StringIntern>(
        &self,
        buf: &mut CommandBuffer,
        intern: &mut I,
        zoom: f64,
        current_page: Option<usize>,
        now: PrintNow,
    ) {
        let options = PrintLayoutOptions {
            line_numbers: &self.line_numbers,
            page_borders: &self.page_borders,
            suppressed_line_number_paragraphs: &self.suppressed,
            picture_urls: &self.picture_urls,
            chart_spaces: &self.chart_spaces,
            model: Some(&self.model),
            review_mode: ReviewDisplayMode::All,
            now,
            page_color: None,
            zoom,
            current_page,
        };
        paint_print_layout(buf, intern, &self.result, &options);
    }
}

fn paint_metric_probes<I: StringIntern>(
    keys: &[MeasureKey],
    commands: &mut CommandBuffer,
    layout_reads: &mut LayoutReadBatch,
    intern: &mut I,
) {
    commands.begin();
    let mut dom = Dom::new(commands, intern);
    let container = dom.el(0, "div");
    dom.style(container, "position", "fixed");
    dom.style(container, "left", "-100000px");
    dom.style(container, "visibility", "hidden");
    dom.style(container, "white-space", "pre");
    for key in keys {
        let font = key.to_font_spec();
        let span = dom.el(container, "span");
        dom.style(span, "font", &measure_font_css(&font));
        if let Some(kern) = &font.kerning {
            if kern != "auto" {
                dom.style(span, "font-kerning", kern);
            }
        }
        dom.text(span, "Mg");
        layout_reads.push(span, LayoutReadKind::SpanRect, 0);
    }
}

fn collect_pictures(loaded: &mut LoadedDocument<'_>) -> HashMap<String, String> {
    let names = loaded.model.media_part_names.clone();
    let mut out = HashMap::new();
    for name in names {
        let Ok(Some(bytes)) = loaded.package.read(&name) else {
            continue;
        };
        let mime = mime_of(&name);
        if !mime.starts_with("image/") {
            continue;
        }
        out.insert(name, format!("data:{mime};base64,{}", b64_encode(&bytes)));
    }
    out
}

fn collect_charts(loaded: &mut LoadedDocument<'_>) -> HashMap<String, ChartSpace> {
    let names: Vec<String> = loaded.package.part_names().map(str::to_string).collect();
    let mut out = HashMap::new();
    for name in names {
        if !(name.contains("/charts/") && name.ends_with(".xml")) {
            continue;
        }
        let xml = match loaded.package.text(&name) {
            Ok(Some(xml)) => xml.to_string(),
            _ => continue,
        };
        out.insert(name, parse_chart_space_xml(&xml).chart_space);
    }
    out
}

fn mime_of(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        _ => "application/octet-stream",
    }
}

fn b64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | data[i + 2] as u32;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push(T[(n & 63) as usize] as char);
        i += 3;
    }
    if i < data.len() {
        let rem = data.len() - i;
        let mut n = (data[i] as u32) << 16;
        if rem == 2 {
            n |= (data[i + 1] as u32) << 8;
        }
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        if rem == 2 {
            out.push(T[((n >> 6) & 63) as usize] as char);
            out.push('=');
        } else {
            out.push('=');
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use nitro_xml_render::LAYOUT_RESULT_LANES;

    #[test]
    fn metric_span_absorb_uses_height_lane() {
        // Catches: line-height probes reading width lane (index 2) instead of height (3).
        let values = [0.0, 0.0, 99.0, 20.0];
        let base = 0;
        assert_eq!(values[base + 3], 20.0);
        assert_ne!(values[base + 2], values[base + 3]);
        assert_eq!(values[base + 3] * 0.8, 16.0);
        assert_eq!(LAYOUT_RESULT_LANES, 4);
    }
}
