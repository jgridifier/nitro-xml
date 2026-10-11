// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/measure.ts; changes listed in PROVENANCE.md.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use nitro_xml_render::{LayoutReadBatch, LayoutReadKind, MeasureBatch, LAYOUT_RESULT_LANES};
use serde_json::{json, Value};

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

pub fn font_metrics(measurer: &dyn TextMeasurer, font: &LayoutFontSpec) -> (f64, f64) {
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

/// Full CSS `font` shorthand matching upstream `createCanvasMeasurer` `fontString`.
///
/// Includes italic/bold so the loader's `measureText` path matches Print Layout widths
/// (family-only strings measured as regular and drifted fragment x by ~25%).
pub fn measure_font_css(font: &LayoutFontSpec) -> String {
    let mut out = String::new();
    if font.italic {
        out.push_str("italic ");
    }
    if font.bold {
        out.push_str("bold ");
    }
    out.push_str(&format!(
        "{}px {}",
        font.size_px,
        css_font_stack(Some(&font.family))
    ));
    out
}

/// Sentinel texts for line-height / ascent probes over the width-only MeasureBatch lane.
/// Loader `measure.js` maps these to fontBoundingBox ascent(+descent) like the canvas measurer.
pub const MEASURE_LINE_HEIGHT_TEXT: &str = "\u{0001}";
pub const MEASURE_ASCENT_TEXT: &str = "\u{0002}";

/// Font id payload: CSS font shorthand plus optional kerning (`font\0kerning`).
pub fn measure_font_payload(font: &LayoutFontSpec) -> String {
    let css = measure_font_css(font);
    let kern = font.kerning.as_deref().unwrap_or("auto");
    if kern == "auto" {
        css
    } else {
        format!("{css}\0{kern}")
    }
}

/// Stable identity of one width or font-metric probe.
///
/// `text` is empty for line-height and ascent entries. `size_bits` is `f64::to_bits`
/// so equal sizes compare equal after a JSON round-trip.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MeasureKey {
    pub family: String,
    pub size_bits: u64,
    pub bold: bool,
    pub italic: bool,
    pub kerning: Option<String>,
    pub ligatures: Option<String>,
    pub small_caps: bool,
    pub text: String,
}

impl MeasureKey {
    pub fn width(font: &LayoutFontSpec, text: &str) -> Self {
        let mut key = Self::font_only(font);
        key.text = text.to_string();
        key
    }

    pub fn font_only(font: &LayoutFontSpec) -> Self {
        Self {
            family: font.family.clone(),
            size_bits: font.size_px.to_bits(),
            bold: font.bold,
            italic: font.italic,
            kerning: font.kerning.clone(),
            ligatures: font.ligatures.clone(),
            small_caps: font.small_caps,
            text: String::new(),
        }
    }

    pub fn size_px(&self) -> f64 {
        f64::from_bits(self.size_bits)
    }

    /// Font-only key (empty text) used for line-height and ascent.
    pub fn metrics_key(&self) -> Self {
        let mut key = self.clone();
        key.text.clear();
        key
    }

    pub fn cache_key(&self) -> String {
        // `size_bits` is a decimal string so the key survives JSON without
        // changing the f64 (a printed size like 9.533… does not round-trip).
        json!([
            self.family,
            self.size_bits.to_string(),
            self.bold,
            self.italic,
            self.kerning,
            self.ligatures,
            self.small_caps,
            self.text,
        ])
        .to_string()
    }

    pub fn from_cache_key(raw: &str) -> Option<Self> {
        let values: Vec<Value> = serde_json::from_str(raw).ok()?;
        if values.len() != 8 {
            return None;
        }
        let family = values[0].as_str()?.to_string();
        let size_bits = match &values[1] {
            Value::String(bits) => bits.parse::<u64>().ok()?,
            Value::Number(number) => number
                .as_u64()
                .or_else(|| number.as_f64().map(f64::to_bits))?,
            _ => return None,
        };
        let bold = values[2].as_bool()?;
        let italic = values[3].as_bool()?;
        let kerning = match &values[4] {
            Value::Null => None,
            Value::String(s) => Some(s.clone()),
            _ => return None,
        };
        let ligatures = match &values[5] {
            Value::Null => None,
            Value::String(s) => Some(s.clone()),
            _ => return None,
        };
        let small_caps = values[6].as_bool()?;
        let text = values[7].as_str()?.to_string();
        Some(Self {
            family,
            size_bits,
            bold,
            italic,
            kerning,
            ligatures,
            small_caps,
            text,
        })
    }

    pub fn to_font_spec(&self) -> LayoutFontSpec {
        LayoutFontSpec {
            family: self.family.clone(),
            size_px: self.size_px(),
            bold: self.bold,
            italic: self.italic,
            kerning: self.kerning.clone(),
            ligatures: self.ligatures.clone(),
            small_caps: self.small_caps,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PendingKind {
    Width,
    LineHeight,
    Ascent,
}

impl PendingKind {
    fn tag(self) -> u8 {
        match self {
            Self::Width => 0,
            Self::LineHeight => 1,
            Self::Ascent => 2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Width => "width",
            Self::LineHeight => "lineHeight",
            Self::Ascent => "ascent",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PendingMeasure {
    pub key: MeasureKey,
    pub kind: PendingKind,
}

impl PendingMeasure {
    pub fn to_json(&self) -> Value {
        json!({
            "key": self.key.cache_key(),
            "family": self.key.family,
            "sizePx": self.key.size_px(),
            "bold": self.key.bold,
            "italic": self.key.italic,
            "kerning": self.key.kerning,
            "ligatures": self.key.ligatures,
            "smallCaps": self.key.small_caps,
            "text": self.key.text,
            "kind": self.kind.as_str(),
        })
    }
}

/// One resolved probe. Unset fields are left unchanged in the cache.
#[derive(Clone, Debug, Default)]
pub struct MeasureSample {
    pub width: Option<f64>,
    pub line_height: Option<f64>,
    pub ascent: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct MeasureCache {
    widths: HashMap<MeasureKey, f64>,
    line_heights: HashMap<MeasureKey, f64>,
    ascents: HashMap<MeasureKey, f64>,
}

impl MeasureCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_json(&mut self, text: &str) -> Result<(), String> {
        let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let obj = value
            .as_object()
            .ok_or_else(|| "measure cache must be a JSON object".to_string())?;
        for (raw_key, entry) in obj {
            let key = MeasureKey::from_cache_key(raw_key)
                .ok_or_else(|| format!("bad measure key: {raw_key}"))?;
            if let Some(width) = entry.get("width").and_then(Value::as_f64) {
                self.widths.insert(key.clone(), width);
            }
            let font = key.metrics_key();
            if let Some(height) = entry.get("lineHeight").and_then(Value::as_f64) {
                self.line_heights.insert(font.clone(), height);
            }
            if let Some(ascent) = entry.get("ascent").and_then(Value::as_f64) {
                self.ascents.insert(font, ascent);
            }
        }
        Ok(())
    }

    pub fn width(&self, key: &MeasureKey) -> Option<f64> {
        self.widths.get(key).copied()
    }

    pub fn line_height(&self, key: &MeasureKey) -> Option<f64> {
        self.line_heights.get(key).copied()
    }

    pub fn ascent(&self, key: &MeasureKey) -> Option<f64> {
        self.ascents.get(key).copied()
    }

    pub fn insert_width(&mut self, key: MeasureKey, width: f64) {
        self.widths.insert(key, width);
    }

    /// Absorb resolver output aligned with `pending` (same index).
    pub fn absorb(&mut self, pending: &[PendingMeasure], samples: &[MeasureSample]) {
        for (probe, sample) in pending.iter().zip(samples.iter()) {
            if let Some(width) = sample.width {
                self.widths.insert(probe.key.clone(), width);
            }
            let font = probe.key.metrics_key();
            if let Some(height) = sample.line_height {
                self.line_heights.insert(font.clone(), height);
            }
            if let Some(ascent) = sample.ascent {
                self.ascents.insert(font, ascent);
            }
        }
    }
}

/// Cache-backed measurer. Hits return stored values. Misses are recorded and
/// answered with [`create_fake_measurer`] for this pass only.
pub struct CachedMeasurer<'a> {
    cache: &'a MeasureCache,
    pending: RefCell<Vec<PendingMeasure>>,
    seen: RefCell<HashSet<(u8, MeasureKey)>>,
    fake: FakeMeasurer,
}

impl<'a> CachedMeasurer<'a> {
    pub fn new(cache: &'a MeasureCache) -> Self {
        Self {
            cache,
            pending: RefCell::new(Vec::new()),
            seen: RefCell::new(HashSet::new()),
            fake: create_fake_measurer(),
        }
    }

    pub fn take_pending(&self) -> Vec<PendingMeasure> {
        self.seen.borrow_mut().clear();
        std::mem::take(&mut *self.pending.borrow_mut())
    }

    fn record(&self, kind: PendingKind, key: MeasureKey) {
        let tag = kind.tag();
        if self.seen.borrow_mut().insert((tag, key.clone())) {
            self.pending.borrow_mut().push(PendingMeasure { key, kind });
        }
    }
}

impl TextMeasurer for CachedMeasurer<'_> {
    fn width_of(&self, text: &str, font: &LayoutFontSpec) -> f64 {
        let key = MeasureKey::width(font, text);
        if let Some(width) = self.cache.width(&key) {
            return width;
        }
        self.record(PendingKind::Width, key);
        self.fake.width_of(text, font)
    }

    fn line_height_of(&self, font: &LayoutFontSpec) -> f64 {
        let key = MeasureKey::font_only(font);
        if let Some(height) = self.cache.line_height(&key) {
            return height;
        }
        self.record(PendingKind::LineHeight, key);
        self.fake.line_height_of(font)
    }

    fn ascent_of(&self, font: &LayoutFontSpec) -> Option<f64> {
        let key = MeasureKey::font_only(font);
        if let Some(ascent) = self.cache.ascent(&key) {
            return Some(ascent);
        }
        self.record(PendingKind::Ascent, key);
        None
    }
}

struct BatchInner<'a> {
    cache: &'a mut MeasureCache,
    measures: &'a mut MeasureBatch,
    layout_reads: &'a mut LayoutReadBatch,
    fallback: FakeMeasurer,
    span_node_id: u32,
    /// Parallel to [`MeasureBatch`] pushes since the last absorb.
    pending_widths: Vec<MeasureKey>,
    span_keys: Vec<MeasureKey>,
    strings: Vec<String>,
    string_ids: HashMap<String, u32>,
}

impl<'a> BatchInner<'a> {
    fn intern(&mut self, text: &str) -> u32 {
        if let Some(id) = self.string_ids.get(text) {
            return *id;
        }
        let id = self.strings.len() as u32;
        self.strings.push(text.to_string());
        self.string_ids.insert(text.to_string(), id);
        id
    }

    fn queue_width(&mut self, key: MeasureKey, font: &LayoutFontSpec, text: &str) {
        if self.pending_widths.iter().any(|existing| existing == &key) {
            return;
        }
        let font_id = self.intern(&measure_font_payload(font));
        let text_id = self.intern(text);
        self.measures.push(font_id, font.size_px as f32, text_id);
        self.pending_widths.push(key);
    }

    fn queue_span(&mut self, key: MeasureKey) {
        if self.span_keys.iter().any(|existing| existing == &key) {
            return;
        }
        self.layout_reads
            .push(self.span_node_id, LayoutReadKind::SpanRect, 0);
        self.span_keys.push(key);
    }
}

/// Queues measure misses into a [`MeasureBatch`] (ligature / small-caps use span probes).
///
/// Width hits come from [`MeasureCache`]. After the loader calls
/// [`MeasureBatch::set_results`], [`BatchedMeasurer::absorb_measure_results`]
/// writes those widths back by pending index.
pub struct BatchedMeasurer<'a> {
    inner: RefCell<BatchInner<'a>>,
}

impl<'a> BatchedMeasurer<'a> {
    pub fn new(
        cache: &'a mut MeasureCache,
        measures: &'a mut MeasureBatch,
        layout_reads: &'a mut LayoutReadBatch,
        span_node_id: u32,
    ) -> Self {
        Self {
            inner: RefCell::new(BatchInner {
                cache,
                measures,
                layout_reads,
                fallback: create_fake_measurer(),
                span_node_id,
                pending_widths: Vec::new(),
                span_keys: Vec::new(),
                strings: Vec::new(),
                string_ids: HashMap::new(),
            }),
        }
    }

    pub fn measures_mut(&mut self) -> &mut MeasureBatch {
        self.inner.get_mut().measures
    }

    pub fn layout_reads_mut(&mut self) -> &mut LayoutReadBatch {
        self.inner.get_mut().layout_reads
    }

    /// String table for `font_id` / `text_id` pushed into the batch.
    pub fn resolve(&self, id: u32) -> Option<String> {
        self.inner.borrow().strings.get(id as usize).cloned()
    }

    /// Copy [`MeasureBatch::widths`] into the cache, paired with pending keys.
    pub fn absorb_measure_results(&mut self) {
        let inner = self.inner.get_mut();
        let widths = inner.measures.widths.clone();
        for (index, key) in inner.pending_widths.iter().enumerate() {
            if let Some(width) = widths.get(index) {
                inner.cache.insert_width(key.clone(), *width);
            }
        }
        inner.pending_widths.clear();
    }

    /// Copy span-probe widths (lane 2) into the cache after layout read-back.
    pub fn absorb_span_results(&mut self) {
        let inner = self.inner.get_mut();
        let values = inner.layout_reads.values.clone();
        for (index, key) in inner.span_keys.iter().enumerate() {
            let base = index * LAYOUT_RESULT_LANES;
            if base + 2 < values.len() {
                inner.cache.insert_width(key.clone(), values[base + 2]);
            }
        }
        inner.span_keys.clear();
    }
}

impl TextMeasurer for BatchedMeasurer<'_> {
    fn width_of(&self, text: &str, font: &LayoutFontSpec) -> f64 {
        let key = MeasureKey::width(font, text);
        let mut inner = self.inner.borrow_mut();
        if let Some(width) = inner.cache.width(&key) {
            return width;
        }
        let estimate = inner.fallback.width_of(text, font);
        if font.ligatures.is_some() || font.small_caps {
            inner.queue_span(key);
        } else {
            inner.queue_width(key, font, text);
        }
        estimate
    }

    fn line_height_of(&self, font: &LayoutFontSpec) -> f64 {
        let key = MeasureKey::font_only(font);
        let inner = self.inner.borrow();
        inner
            .cache
            .line_height(&key)
            .unwrap_or_else(|| inner.fallback.line_height_of(font))
    }

    fn ascent_of(&self, font: &LayoutFontSpec) -> Option<f64> {
        let key = MeasureKey::font_only(font);
        self.inner.borrow().cache.ascent(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_font_css_includes_bold_italic() {
        // Catches: measure batch font ids omitting bold/italic so canvas widths
        // drift from createCanvasMeasurer and Print Layout fragment x positions.
        let font = LayoutFontSpec {
            family: "Calibri".into(),
            size_px: 14.666666666666666,
            bold: true,
            italic: true,
            kerning: Some("none".into()),
            ligatures: None,
            small_caps: false,
        };
        let css = measure_font_css(&font);
        assert!(css.starts_with("italic bold "), "{css}");
        assert!(css.contains("px "), "{css}");
        assert!(css.contains("Calibri"), "{css}");
        let payload = measure_font_payload(&font);
        assert!(payload.contains('\0'), "{payload}");
        assert!(payload.ends_with("none"), "{payload}");
    }
}
