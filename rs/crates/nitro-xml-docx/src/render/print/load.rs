// SPDX-License-Identifier: MIT
//! Open a package and the inputs Print Layout needs for one viewer session.

use std::collections::{HashMap, HashSet};

use nitro_xml_opc::chart::{parse_chart_space_xml, ChartSpace};
use nitro_xml_render::{CommandBuffer, MeasureBatch};

use super::dom::StringIntern;
use super::line_numbers::{
    line_numbering_from_section, suppressed_line_number_ids, PrintLineNumbering,
};
use super::options::{PrintLayoutOptions, PrintNow};
use super::page_borders::{page_borders_from_section, PageBorders};
use super::pages::paint_print_layout;
use crate::body::{read_package, DocumentModel, LoadedDocument};
use crate::layout::{
    layout_document_model, measure_font_css, CachedMeasurer, LayoutResult, LayoutReviewOptions,
    MeasureCache, MeasureSample, PendingKind, PendingMeasure,
};
use crate::review_formatting_display::ReviewDisplayMode;

/// MeasureBatch sizePx sentinels (loader `fillMeasureWidths`; createCanvasMeasurer).
const SIZE_LINE_HEIGHT: f32 = -1.0;
const SIZE_ASCENT: f32 = -2.0;

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
    measure_pending: Vec<PendingMeasure>,
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
            measure_pending: Vec::new(),
            layout_ready: false,
        })
    }

    /// Drive one CachedMeasurer pass: absorb loader widths/metrics, re-layout, queue misses.
    ///
    /// Widths use `measure_font_css` (bold/italic). Line-height/ascent use sizePx sentinels
    /// `-1`/`-2` so the shared loader `fillMeasureWidths` reads fontBoundingBox like
    /// upstream `createCanvasMeasurer` — no docx-only loader branch.
    pub fn tick_measure<I: StringIntern>(
        &mut self,
        measures: &mut MeasureBatch,
        intern: &mut I,
    ) -> bool {
        if self.layout_ready {
            return true;
        }

        if !self.measure_pending.is_empty()
            && measures.awaiting() == 0
            && measures.widths.len() == self.measure_pending.len()
        {
            let mut samples = Vec::with_capacity(self.measure_pending.len());
            for (probe, &width) in self.measure_pending.iter().zip(measures.widths.iter()) {
                samples.push(match probe.kind {
                    PendingKind::Width => MeasureSample {
                        width: Some(width),
                        ..MeasureSample::default()
                    },
                    PendingKind::LineHeight => MeasureSample {
                        line_height: Some(width),
                        ..MeasureSample::default()
                    },
                    PendingKind::Ascent => MeasureSample {
                        ascent: Some(width),
                        ..MeasureSample::default()
                    },
                });
            }
            self.measure_cache.absorb(&self.measure_pending, &samples);
            self.measure_pending.clear();
            measures.widths.clear();
        }

        if measures.awaiting() > 0 || !measures.is_empty() {
            return false;
        }

        // Match upstream Print Layout: no UAX #9 reorder on the paint path.
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
            return true;
        }

        self.queue_pending(&pending, measures, intern);
        self.measure_pending = pending;
        false
    }

    fn queue_pending<I: StringIntern>(
        &self,
        pending: &[PendingMeasure],
        measures: &mut MeasureBatch,
        intern: &mut I,
    ) {
        for probe in pending {
            let font = probe.key.to_font_spec();
            let font_id = intern.intern(&measure_font_css(&font));
            let (size, text) = match probe.kind {
                PendingKind::Width => (probe.key.size_px() as f32, probe.key.text.as_str()),
                PendingKind::LineHeight => (SIZE_LINE_HEIGHT, ""),
                PendingKind::Ascent => (SIZE_ASCENT, ""),
            };
            let text_id = intern.intern(text);
            measures.push(font_id, size, text_id);
        }
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
