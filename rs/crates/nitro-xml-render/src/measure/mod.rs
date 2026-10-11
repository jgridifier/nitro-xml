// SPDX-License-Identifier: MIT
//! Types for the browser measurement and post-apply layout read-back paths.
//!
//! Measurement runs on the main thread via the loader's batched `measureText`
//! (`packages/nitro-xml/loader/measure.js`). Post-apply node-layout queries
//! (span rect, scrollHeight, computed style, naturalWidth) are answered in one
//! batch on the next frame (`packages/nitro-xml/loader/layout-readback.js`).
//! Never one wasm call per string or per element.

use std::collections::HashMap;

/// Batched text-measurement requests (fonts/measurement follow upstream).
///
/// The loader applies `measureText` for each pending request and calls
/// [`MeasureBatch::set_results`].
#[derive(Debug, Default)]
pub struct MeasureBatch {
    pub font_id: Vec<u32>,
    pub size_px: Vec<f32>,
    pub text_id: Vec<u32>,
    pub widths: Vec<f64>,
    /// Last resolved widths keyed by `(font_id, text_id)` for the next paint pass.
    pub cache: HashMap<(u32, u32), f64>,
}

/// Length mismatch between pending requests and loader widths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasureError {
    pub expected: usize,
    pub got: usize,
}

impl std::fmt::Display for MeasureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "expected {} widths, got {}", self.expected, self.got)
    }
}

impl std::error::Error for MeasureError {}

impl MeasureBatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.font_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.font_id.is_empty()
    }

    pub fn push(&mut self, font_id: u32, size_px: f32, text_id: u32) {
        self.font_id.push(font_id);
        self.size_px.push(size_px);
        self.text_id.push(text_id);
    }

    pub fn set_results(&mut self, widths: &[f64]) -> Result<(), MeasureError> {
        if widths.len() != self.font_id.len() {
            return Err(MeasureError {
                expected: self.font_id.len(),
                got: widths.len(),
            });
        }
        for ((&font_id, &text_id), &width) in self.font_id.iter().zip(&self.text_id).zip(widths) {
            self.cache.insert((font_id, text_id), width);
        }
        self.widths.clear();
        self.widths.extend_from_slice(widths);
        self.font_id.clear();
        self.size_px.clear();
        self.text_id.clear();
        Ok(())
    }

    pub fn cached_width(&self, font_id: u32, text_id: u32) -> Option<f64> {
        self.cache.get(&(font_id, text_id)).copied()
    }
}

/// Post-apply layout read-back kinds (loader README / `docs/boundary.md`).
///
/// Requests are keyed by command-buffer node id. The loader answers the whole
/// batch on the **next** frame after `applyCommandBuffer` — never one call per
/// element. Used by docx span probes, pptx autofit, and P5 `naturalWidth`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutReadKind {
    /// `getBoundingClientRect` → x, y, w, h (docx `canvas-measurer.ts` span).
    SpanRect = 1,
    /// `element.scrollHeight` → height (pptx autofit content height).
    ScrollHeight = 2,
    /// `parseFloat(getComputedStyle(el)[prop])` → value (`font-size` / `line-height`).
    ComputedStyle = 3,
    /// `HTMLImageElement.naturalWidth/Height` → w, h (P5 `image-native-size.ts`).
    NaturalWidth = 4,
}

impl LayoutReadKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::SpanRect),
            2 => Some(Self::ScrollHeight),
            3 => Some(Self::ComputedStyle),
            4 => Some(Self::NaturalWidth),
            _ => None,
        }
    }
}

/// Number of `f64` lanes written per layout read-back request.
pub const LAYOUT_RESULT_LANES: usize = 4;

/// Length mismatch between awaiting layout requests and loader values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutReadError {
    pub expected: usize,
    pub got: usize,
}

impl std::fmt::Display for LayoutReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "expected {} layout values, got {}",
            self.expected, self.got
        )
    }
}

impl std::error::Error for LayoutReadError {}

/// Batched post-apply DOM layout queries (node-id keyed).
///
/// Requests are **taken** when the frame leaves wasm (so the next layout pass can
/// queue a fresh batch). The loader answers on the following frame with a flat
/// `f64` SoA of length `taken * LAYOUT_RESULT_LANES`.
#[derive(Debug, Default)]
pub struct LayoutReadBatch {
    pub node_id: Vec<u32>,
    pub kind: Vec<u8>,
    /// String-table id of the CSS property name; used only by [`LayoutReadKind::ComputedStyle`].
    pub prop_id: Vec<u32>,
    pub values: Vec<f64>,
    /// Requests taken and awaiting a reply (0 when idle).
    awaiting: usize,
}

impl LayoutReadBatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.node_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.node_id.is_empty()
    }

    pub fn awaiting(&self) -> usize {
        self.awaiting
    }

    pub fn push(&mut self, node_id: u32, kind: LayoutReadKind, prop_id: u32) {
        self.node_id.push(node_id);
        self.kind.push(kind as u8);
        self.prop_id.push(prop_id);
    }

    /// Move pending requests out for the loader; record how many replies are due.
    pub fn take_requests(&mut self) -> (Vec<u32>, Vec<u8>, Vec<u32>) {
        let node_id = std::mem::take(&mut self.node_id);
        let kind = std::mem::take(&mut self.kind);
        let prop_id = std::mem::take(&mut self.prop_id);
        self.awaiting = node_id.len();
        (node_id, kind, prop_id)
    }

    pub fn set_results(&mut self, values: &[f64]) -> Result<(), LayoutReadError> {
        let expected = self.awaiting * LAYOUT_RESULT_LANES;
        if values.len() != expected {
            return Err(LayoutReadError {
                expected,
                got: values.len(),
            });
        }
        self.values.clear();
        self.values.extend_from_slice(values);
        self.awaiting = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_results_reject_length_mismatch() {
        // Catches: a short measure reply must error, not silently truncate layout widths.
        let mut m = MeasureBatch::new();
        m.push(1, 11.0, 2);
        m.push(1, 11.0, 3);
        assert!(m.set_results(&[1.0]).is_err());
        assert!(m.set_results(&[1.0, 2.0]).is_ok());
        assert_eq!(m.widths, vec![1.0, 2.0]);
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn layout_read_results_reject_length_mismatch() {
        // Catches: a short post-apply layout reply must error (4 lanes per request).
        let mut b = LayoutReadBatch::new();
        b.push(7, LayoutReadKind::ScrollHeight, 0);
        b.push(8, LayoutReadKind::NaturalWidth, 0);
        let (nodes, kinds, props) = b.take_requests();
        assert_eq!(nodes.len(), 2);
        assert_eq!(kinds.len(), 2);
        assert_eq!(props.len(), 2);
        assert_eq!(b.len(), 0);
        assert_eq!(b.awaiting(), 2);
        assert!(b.set_results(&[1.0]).is_err());
        let ok = [10.0, 0.0, 0.0, 0.0, 64.0, 48.0, 0.0, 0.0];
        assert!(b.set_results(&ok).is_ok());
        assert_eq!(b.values, ok);
        assert_eq!(b.awaiting(), 0);
    }
}
