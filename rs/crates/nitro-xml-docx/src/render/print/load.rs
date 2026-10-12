// SPDX-License-Identifier: MIT
//! Open a package and the inputs Print Layout needs for one viewer session.

use std::collections::{HashMap, HashSet};

use nitro_xml_opc::chart::{parse_chart_space_xml, ChartSpace};
use nitro_xml_render::{CommandBuffer, MeasureBatch};

use super::chart_cache::{chart_measure_specs, ChartMeasureSpec};
use super::dom::StringIntern;
use super::line_numbers::{
    line_numbering_from_section, suppressed_line_number_ids, PrintLineNumbering,
};
use super::options::{PrintLayoutOptions, PrintNow};
use super::page_borders::{page_borders_from_section, PageBorders};
use super::pages::paint_print_layout;
use crate::body::{read_package, DocumentModel, LoadedDocument};
use crate::layout::{
    measure_font_css, prepare_layout_document_input, CachedMeasurer, LayoutDocumentInput,
    LayoutResult, LayoutReviewOptions, LayoutSession, MeasureCache, MeasureSample, PendingKind,
    PendingMeasure,
};
use crate::render::chart::{docx_chart_svg, probe_docx_chart_measures};
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
    layout_input: Option<LayoutDocumentInput>,
    layout_session: Option<LayoutSession>,
    pub layout_ready: bool,
    layout_complete: bool,
    chart_svg_cache: HashMap<String, String>,
    chart_measure_jobs: Vec<ChartMeasureJob>,
    chart_measure_pending: usize,
    charts_measured: bool,
}

struct ChartMeasureJob {
    spec: ChartMeasureSpec,
    width_start: usize,
    width_count: usize,
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
            layout_input: None,
            layout_session: None,
            layout_ready: false,
            layout_complete: false,
            chart_svg_cache: HashMap::new(),
            chart_measure_jobs: Vec::new(),
            chart_measure_pending: 0,
            charts_measured: false,
        })
    }

    pub fn charts_measured(&self) -> bool {
        self.charts_measured
    }

    pub fn chart_svg_cache_len(&self) -> usize {
        self.chart_svg_cache.len()
    }

    pub fn layout_complete(&self) -> bool {
        self.layout_complete
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
            self.layout_session = None;
        }

        if measures.awaiting() > 0 || !measures.is_empty() {
            return self.layout_ready;
        }

        let options = LayoutReviewOptions {
            review_display_mode: None,
        };
        if self.layout_input.is_none() {
            let (input, review_notes) = prepare_layout_document_input(&self.model, options.clone());
            for note in review_notes {
                if !self.warnings.iter().any(|w| w == &note) {
                    self.warnings.push(note);
                }
            }
            self.layout_input = Some(input);
        }

        if self.layout_session.is_none() {
            let mut session = LayoutSession::new(self.layout_input.clone().unwrap());
            session.set_priority(0, 1);
            session.set_uax9(false);
            self.layout_session = Some(session);
        }

        // Match upstream Print Layout: no UAX #9 reorder on the paint path.
        let measurer = CachedMeasurer::new(&self.measure_cache);
        let status = self
            .layout_session
            .as_mut()
            .expect("layout session")
            .tick(&measurer, 1);
        let pending = measurer.take_pending();
        if !pending.is_empty() {
            self.layout_session = None;
            self.queue_pending(&pending, measures, intern);
            self.measure_pending = pending;
            return self.layout_ready;
        }

        let partial = self
            .layout_session
            .as_ref()
            .expect("layout session")
            .committed_layout_result();
        self.merge_layout_result(&partial);
        if !partial.pages.is_empty() {
            self.layout_ready = true;
            if chart_measure_specs(&self.result).is_empty() {
                self.charts_measured = true;
            }
        }

        if status.complete {
            if let Some(result) = self
                .layout_session
                .as_ref()
                .and_then(|session| session.result())
            {
                self.result = result;
            }
            self.layout_complete = true;
            self.layout_ready = true;
            if chart_measure_specs(&self.result).is_empty() {
                self.charts_measured = true;
            }
        }

        if self.layout_complete {
            self.tick_chart_measures(measures, intern);
        }

        self.layout_ready
    }

    fn tick_chart_measures<I: StringIntern>(
        &mut self,
        measures: &mut MeasureBatch,
        intern: &mut I,
    ) {
        if self.charts_measured {
            return;
        }

        if self.chart_measure_pending > 0
            && measures.awaiting() == 0
            && measures.widths.len() == self.chart_measure_pending
        {
            let widths = measures.widths.clone();
            measures.widths.clear();
            for job in &self.chart_measure_jobs {
                let slice = widths
                    .get(job.width_start..job.width_start + job.width_count)
                    .unwrap_or(&[]);
                if let Some(space) = self.chart_spaces.get(&job.spec.part_name) {
                    let svg = docx_chart_svg(space, job.spec.width, job.spec.height, slice);
                    self.chart_svg_cache.insert(job.spec.key.clone(), svg);
                }
            }
            self.chart_measure_jobs.clear();
            self.chart_measure_pending = 0;
            self.charts_measured = true;
            return;
        }

        if measures.awaiting() > 0 || self.chart_measure_pending > 0 {
            return;
        }
        if !measures.is_empty() || !self.measure_pending.is_empty() {
            return;
        }

        let specs = chart_measure_specs(&self.result);
        if specs.is_empty() {
            self.charts_measured = true;
            return;
        }

        let mut intern_fn = |text: &str| intern.intern(text);
        let mut jobs = Vec::with_capacity(specs.len());
        for spec in specs {
            let Some(space) = self.chart_spaces.get(&spec.part_name) else {
                continue;
            };
            let width_start = measures.len();
            probe_docx_chart_measures(space, spec.width, spec.height, measures, &mut intern_fn);
            let width_count = measures.len() - width_start;
            if width_count == 0 {
                continue;
            }
            jobs.push(ChartMeasureJob {
                spec,
                width_start,
                width_count,
            });
        }
        if jobs.is_empty() {
            self.charts_measured = true;
            return;
        }
        self.chart_measure_jobs = jobs;
        self.chart_measure_pending = measures.len();
    }

    fn merge_layout_result(&mut self, partial: &LayoutResult) {
        if partial.pages.len() > self.result.pages.len() {
            self.result.pages = partial.pages.clone();
        }
        for note in &partial.approximations {
            if !self.warnings.iter().any(|w| w == note) {
                self.warnings.push(note.clone());
            }
            if !self.result.approximations.iter().any(|a| a == note) {
                self.result.approximations.push(note.clone());
            }
        }
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
            chart_svg_cache: &self.chart_svg_cache,
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

#[cfg(test)]
mod incremental_layout_tests {
    use super::*;
    use crate::body::DocumentModel;
    use crate::render::print::dom::StringIntern;
    use nitro_xml_render::MeasureBatch;
    use serde_json::json;

    struct TestIntern {
        next: u32,
        map: std::collections::HashMap<String, u32>,
    }

    impl TestIntern {
        fn new() -> Self {
            Self {
                next: 1,
                map: std::collections::HashMap::new(),
            }
        }
    }

    impl StringIntern for TestIntern {
        fn intern(&mut self, s: &str) -> u32 {
            if let Some(id) = self.map.get(s) {
                return *id;
            }
            let id = self.next;
            self.next += 1;
            self.map.insert(s.to_string(), id);
            id
        }
    }

    fn multi_page_model() -> DocumentModel {
        let blocks = (0..4)
            .map(|index| {
                json!({
                    "type": "paragraph",
                    "id": format!("p{index}"),
                    "pageBreakBefore": index > 0,
                    "widowControl": false,
                    "runs": [{ "text": format!("Paragraph {index} on its own page") }]
                })
            })
            .collect();
        DocumentModel {
            format: "docx".into(),
            blocks,
            page: json!({
                "width": 612.0,
                "height": 792.0,
                "marginTop": 72.0,
                "marginRight": 72.0,
                "marginBottom": 72.0,
                "marginLeft": 72.0
            }),
            warnings: Vec::new(),
            paragraph_styles: None,
            character_styles: None,
            table_styles: None,
            numbering_catalog: None,
            theme: None,
            sections: None,
            footnotes: None,
            endnotes: None,
            footnote_num_fmt: None,
            endnote_num_fmt: None,
            even_and_odd_headers: false,
            auto_hyphenation: false,
            comments: None,
            track_changes: None,
            track_formatting: None,
            track_moves: None,
            media_part_names: Vec::new(),
        }
    }

    fn test_session(model: DocumentModel) -> PrintSession {
        PrintSession {
            model,
            result: LayoutResult {
                pages: Vec::new(),
                approximations: Vec::new(),
            },
            picture_urls: HashMap::new(),
            chart_spaces: HashMap::new(),
            line_numbers: Vec::new(),
            page_borders: Vec::new(),
            suppressed: HashSet::new(),
            warnings: Vec::new(),
            measure_cache: MeasureCache::new(),
            measure_pending: Vec::new(),
            layout_input: None,
            layout_session: None,
            layout_ready: false,
            layout_complete: false,
            chart_svg_cache: HashMap::new(),
            chart_measure_jobs: Vec::new(),
            chart_measure_pending: 0,
            charts_measured: false,
        }
    }

    fn resolve_measures(measures: &mut MeasureBatch) {
        if measures.is_empty() {
            return;
        }
        let n = measures.len();
        let _ = measures.take_requests();
        let widths: Vec<f64> = (0..n).map(|_| 48.0).collect();
        measures.set_results(&widths).expect("measure results");
    }

    #[test]
    fn poi_61745_chart_svg_cache_after_measure() {
        // Catches: print session never fills chart SVG cache after layout (charts stay on estimates).
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/real/docx/poi-61745.docx");
        let bytes = std::fs::read(&path).expect("fixture");
        let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let mut session = PrintSession::open(leaked).expect("open");
        let mut measures = MeasureBatch::new();
        let mut intern = TestIntern::new();
        for _ in 0..256 {
            session.tick_measure(&mut measures, &mut intern);
            if !measures.is_empty() {
                let n = measures.len();
                let _ = measures.take_requests();
                let widths: Vec<f64> = (0..n).map(|i| 10.0 + (i as f64) * 0.5).collect();
                measures.set_results(&widths).expect("measure");
            }
            if session.charts_measured() && session.chart_svg_cache_len() > 0 {
                return;
            }
        }
        assert!(
            session.charts_measured() && session.chart_svg_cache_len() > 0,
            "expected measured chart SVG cache after layout+measure ticks"
        );
    }

    #[test]
    fn first_page_paintable_before_full_document_layout() {
        // Catches: PrintSession never paints until full multi-page measure finishes (poi-bug65649 hang).
        let mut session = test_session(multi_page_model());
        let mut measures = MeasureBatch::new();
        let mut intern = TestIntern::new();
        let mut saw_first_page = false;
        for _ in 0..64 {
            let ready = session.tick_measure(&mut measures, &mut intern);
            if ready && !session.result.pages.is_empty() && !session.layout_complete {
                saw_first_page = true;
                break;
            }
            resolve_measures(&mut measures);
        }
        assert!(
            saw_first_page,
            "expected layout_ready with at least one page before layout_complete"
        );
    }
}
