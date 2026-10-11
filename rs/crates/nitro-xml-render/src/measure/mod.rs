// SPDX-License-Identifier: MIT
//! Types for the browser measurement path behind the command buffer.
//!
//! Measurement runs on the main thread via the loader's batched `measureText`
//! / DOM span probes (`packages/nitro-xml/loader/measure.js`). Never one wasm
//! call per string. Layout for post-apply node-layout read-back is TODO (HoP
//! R0 follow-up on #33).

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
        self.widths.clear();
        self.widths.extend_from_slice(widths);
        self.font_id.clear();
        self.size_px.clear();
        self.text_id.clear();
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
}
