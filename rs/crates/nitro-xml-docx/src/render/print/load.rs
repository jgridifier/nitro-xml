// SPDX-License-Identifier: MIT
//! Open a package and the inputs Print Layout needs for one viewer session.

use std::collections::{HashMap, HashSet};

use nitro_xml_opc::chart::{parse_chart_space_xml, ChartSpace};
use nitro_xml_opc::xml::parse_xml_str;
use nitro_xml_render::{CommandBuffer, MeasureBatch};

use super::chart_cache::chart_measure_specs;
use super::dom::StringIntern;
use super::line_numbers::{
    line_numbering_from_section, suppressed_line_number_ids, PrintLineNumbering,
};
use super::options::{PrintLayoutOptions, PrintNow};
use super::page_borders::{page_borders_from_section, PageBorders};
use super::pages::paint_print_layout;
use crate::body::{
    read_package_first_view, read_package_with_opts, BodyParseStats, BodyViewOpts, DocumentModel,
    LoadedDocument,
};
use crate::layout::{
    measure_font_css, prepare_layout_document_input, CachedMeasurer, LayoutDocumentInput,
    LayoutResult, LayoutReviewOptions, LayoutSession, MeasureCache, MeasureSample, PendingKind,
    PendingMeasure,
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
    pub chart_color_palettes: HashMap<String, i32>,
    pub line_numbers: Vec<Option<PrintLineNumbering>>,
    pub page_borders: Vec<Option<PageBorders>>,
    pub suppressed: HashSet<String>,
    pub warnings: Vec<String>,
    /// True when [`Self::open_first_view`] stopped before the end of `w:body`.
    pub body_truncated: bool,
    /// Top-level body blocks parsed on the last open / ensure.
    pub blocks_parsed: usize,
    measure_cache: MeasureCache,
    measure_pending: Vec<PendingMeasure>,
    layout_input: Option<LayoutDocumentInput>,
    layout_session: Option<LayoutSession>,
    pub layout_ready: bool,
    layout_complete: bool,
    chart_svg_cache: HashMap<String, String>,
    charts_measured: bool,
}

impl PrintSession {
    /// Full-body open (oracle / bench / ensure path).
    /// Callers may free or reuse `bytes` after this returns — the session does
    /// not retain the zip buffer (avoids a leaked second package copy).
    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        let (loaded, stats) =
            read_package_with_opts(bytes, BodyViewOpts::default()).map_err(|e| e.to_string())?;
        Self::from_loaded(loaded, stats)
    }

    /// Parse only enough body blocks for the first N hard page breaks (viewer TTFV).
    /// Same ownership contract as [`Self::open`]: no retained zip buffer.
    pub fn open_first_view(bytes: &[u8], max_hard_page_breaks: usize) -> Result<Self, String> {
        let (loaded, stats) =
            read_package_first_view(bytes, max_hard_page_breaks).map_err(|e| e.to_string())?;
        Self::from_loaded(loaded, stats)
    }

    /// Replace a truncated model with a full `read_package` (deferred SeekTail analogue).
    pub fn ensure_body_complete(&mut self, bytes: &[u8]) -> Result<bool, String> {
        if !self.body_truncated {
            return Ok(false);
        }
        let full = Self::open(bytes)?;
        *self = full;
        Ok(true)
    }

    fn from_loaded(mut loaded: LoadedDocument<'_>, stats: BodyParseStats) -> Result<Self, String> {
        let picture_urls = collect_pictures(&mut loaded);
        let (chart_spaces, chart_color_palettes) = collect_charts(&mut loaded);
        let warnings = loaded.model.warnings.clone();
        let model = loaded.model;
        let blocks_parsed = if stats.blocks_parsed > 0 {
            stats.blocks_parsed
        } else {
            model.blocks.len()
        };
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
            chart_color_palettes,
            warnings,
            body_truncated: stats.body_truncated,
            blocks_parsed,
            measure_cache: MeasureCache::new(),
            measure_pending: Vec::new(),
            layout_input: None,
            layout_session: None,
            layout_ready: false,
            layout_complete: false,
            chart_svg_cache: HashMap::new(),
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
            self.tick_chart_measures(measures);
        }

        self.layout_ready
    }

    fn tick_chart_measures(&mut self, measures: &mut MeasureBatch) {
        if self.charts_measured {
            return;
        }
        if measures.awaiting() > 0 {
            return;
        }
        if !measures.is_empty() || !self.measure_pending.is_empty() {
            return;
        }
        // Upstream `docxChartPaint` calls `renderChartSpaceSvg` without `measureText`; chart
        // labels use the S1 width estimates. Canvas measureText (Liberation via @font-face)
        // diverges from that oracle and regresses soft docx chart fixtures (#23).
        self.charts_measured = true;
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
            chart_color_palettes: &self.chart_color_palettes,
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

const CHART_COLOR_STYLE_REL: &str =
    "http://schemas.microsoft.com/office/2011/relationships/chartColorStyle";

fn collect_charts(
    loaded: &mut LoadedDocument<'_>,
) -> (HashMap<String, ChartSpace>, HashMap<String, i32>) {
    let names: Vec<String> = loaded.package.part_names().map(str::to_string).collect();
    let mut spaces = HashMap::new();
    let mut palettes = HashMap::new();
    for name in names {
        if !(name.contains("/charts/") && name.ends_with(".xml")) {
            continue;
        }
        let xml = match loaded.package.text(&name) {
            Ok(Some(xml)) => xml.to_string(),
            _ => continue,
        };
        spaces.insert(name.clone(), parse_chart_space_xml(&xml).chart_space);
        if let Some(id) = chart_color_palette_for_part(loaded, &name) {
            palettes.insert(name, id);
        }
    }
    (spaces, palettes)
}

fn chart_color_palette_for_part(loaded: &mut LoadedDocument<'_>, chart_part: &str) -> Option<i32> {
    let colors_part = loaded
        .package
        .target_of_type(chart_part, CHART_COLOR_STYLE_REL)
        .ok()
        .flatten()?;
    let xml = loaded.package.text(&colors_part).ok().flatten()?;
    parse_chart_color_style_id(xml)
}

/// `cs:colorStyle/@id` from chart color-style parts (upstream `chartColorStyleId`).
fn parse_chart_color_style_id(xml: &str) -> Option<i32> {
    let doc = parse_xml_str(xml).ok()?;
    if doc.root.local != "colorStyle" {
        return None;
    }
    let id = doc
        .root
        .attrs
        .iter()
        .find(|a| a.local == "id")
        .map(|a| a.value.as_str())?;
    let id = id.parse::<i32>().ok()?;
    if id >= 0 {
        Some(id)
    } else {
        None
    }
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
            source_document_xml: None,
        }
    }

    fn test_session(model: DocumentModel) -> PrintSession {
        let blocks_parsed = model.blocks.len();
        PrintSession {
            model,
            result: LayoutResult {
                pages: Vec::new(),
                approximations: Vec::new(),
            },
            picture_urls: HashMap::new(),
            chart_spaces: HashMap::new(),
            chart_color_palettes: HashMap::new(),
            line_numbers: Vec::new(),
            page_borders: Vec::new(),
            suppressed: HashSet::new(),
            warnings: Vec::new(),
            body_truncated: false,
            blocks_parsed,
            measure_cache: MeasureCache::new(),
            measure_pending: Vec::new(),
            layout_input: None,
            layout_session: None,
            layout_ready: false,
            layout_complete: false,
            chart_svg_cache: HashMap::new(),
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
    fn open_first_view_stops_after_first_page_break() {
        // Catches: PrintSession::open_first_view still parsing every body block so
        // first view stays O(document) instead of O(visible page breaks).
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/generated/docx/pages_10.docx");
        let bytes = std::fs::read(&path).expect("pages_10 fixture");
        let first = PrintSession::open_first_view(&bytes, 1).expect("first view");
        assert!(first.body_truncated, "expected truncated body");
        let full = PrintSession::open(&bytes).expect("full");
        assert!(!full.body_truncated);
        assert!(
            first.blocks_parsed < full.blocks_parsed,
            "first={} full={}",
            first.blocks_parsed,
            full.blocks_parsed
        );
        let mut ensured = PrintSession::open_first_view(&bytes, 1).expect("first");
        assert!(ensured.ensure_body_complete(&bytes).expect("ensure"));
        assert!(!ensured.body_truncated);
        assert_eq!(ensured.blocks_parsed, full.blocks_parsed);
    }

    #[test]
    fn docx_print_skips_chart_canvas_measure() {
        // Catches: print layout probing chart measureText (canvas) instead of upstream docxChartPaint estimates.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/real/docx/poi-61745.docx");
        let bytes = std::fs::read(&path).expect("fixture");
        let mut session = PrintSession::open(&bytes).expect("open");
        let mut measures = MeasureBatch::new();
        let mut intern = TestIntern::new();
        for _ in 0..256 {
            session.tick_measure(&mut measures, &mut intern);
            if !measures.is_empty() {
                resolve_measures(&mut measures);
            }
            if session.charts_measured() {
                break;
            }
        }
        assert!(
            session.charts_measured(),
            "expected charts_measured after layout without chart measure probes"
        );
        assert_eq!(
            session.chart_svg_cache_len(),
            0,
            "docx print charts paint from estimates, not a measured SVG cache"
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
