// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/shape-autofit-resize.ts; changes in PROVENANCE.md.

use nitro_xml_render::measure::{LayoutReadBatch, LayoutReadKind, LAYOUT_RESULT_LANES};

use crate::text_layout::TextAutoFitMode;

/// Matches upstream `MIN_ELEMENT_SIZE` / `AUTOFIT_MIN_SHAPE_HEIGHT_PX`.
pub const AUTOFIT_MIN_SHAPE_HEIGHT_PX: f64 = 12.0;

const RESIZE_EPSILON_PX: f64 = 1.0;

/// Pure decision from measured content height (upstream `computeAutoFitShapeHeightPx`).
pub fn compute_auto_fit_shape_height_px(
    auto_fit_mode: Option<TextAutoFitMode>,
    measured_content_height_px: f64,
    current_height_px: f64,
) -> Option<f64> {
    if auto_fit_mode != Some(TextAutoFitMode::Shrink) {
        return None;
    }
    if measured_content_height_px <= 0.0 || measured_content_height_px.is_nan() {
        return None;
    }
    let next = measured_content_height_px
        .max(AUTOFIT_MIN_SHAPE_HEIGHT_PX)
        .round();
    if (next - current_height_px).abs() < RESIZE_EPSILON_PX {
        return None;
    }
    Some(next)
}

#[derive(Debug, Clone)]
pub(crate) struct LayoutReadRecord {
    pub autofit: Option<(String, f64, TextAutoFitMode)>,
}

/// Per-slide `a:spAutoFit` paint convergence (layout read-back on next frame).
#[derive(Debug, Default)]
pub struct SpAutofitSlideState {
    pub height_overrides: std::collections::HashMap<String, f64>,
    last_read_records: Vec<LayoutReadRecord>,
}

impl SpAutofitSlideState {
    pub fn painted_height(&self, element_id: &str, authored_height: f64) -> f64 {
        // Do not apply viewer-time autofit growth when authored height is 0 (cy=0).
        // Matches upstream TS oracle (renderSlideStage), which keeps authored height 0px
        // and allows overflow rather than growing the shape at view time.
        if authored_height <= 0.0 {
            return authored_height;
        }
        self.height_overrides
            .get(element_id)
            .copied()
            .unwrap_or(authored_height)
    }

    pub(crate) fn finish_frame(&mut self, records: Vec<LayoutReadRecord>) {
        self.last_read_records = records;
    }

    pub fn apply_layout_results(&mut self, batch: &mut LayoutReadBatch) {
        if self.last_read_records.is_empty() || batch.values.is_empty() {
            return;
        }
        let expected = self.last_read_records.len() * LAYOUT_RESULT_LANES;
        if batch.values.len() != expected {
            return;
        }
        for (i, rec) in self.last_read_records.iter().enumerate() {
            let Some((element_id, current_height, mode)) = rec.autofit.as_ref() else {
                continue;
            };
            let measured = batch.values[i * LAYOUT_RESULT_LANES];
            if let Some(next) =
                compute_auto_fit_shape_height_px(Some(*mode), measured, *current_height)
            {
                self.height_overrides.insert(element_id.clone(), next);
            }
        }
        self.last_read_records.clear();
        batch.values.clear();
    }
}

#[derive(Debug, Default)]
pub struct SpAutofitPaintState {
    by_slide: std::collections::HashMap<usize, SpAutofitSlideState>,
}

impl SpAutofitPaintState {
    pub fn slide_mut(&mut self, slide_index: usize) -> &mut SpAutofitSlideState {
        self.by_slide.entry(slide_index).or_default()
    }
}

pub fn queue_sp_autofit_scroll_height(
    layout_reads: &mut LayoutReadBatch,
    records: &mut Vec<LayoutReadRecord>,
    element_id: &str,
    auto_fit_mode: Option<TextAutoFitMode>,
    text_block_node_id: u32,
    current_height_px: f64,
) {
    if auto_fit_mode != Some(TextAutoFitMode::Shrink) {
        return;
    }
    // Do not queue layout reads for shapes with authored height 0 (cy=0).
    // Matches upstream TS oracle, which keeps authored height and allows overflow.
    if current_height_px <= 0.0 {
        return;
    }
    layout_reads.push(text_block_node_id, LayoutReadKind::ScrollHeight, 0);
    records.push(LayoutReadRecord {
        autofit: Some((
            element_id.to_owned(),
            current_height_px,
            TextAutoFitMode::Shrink,
        )),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: normAutofit / noAutofit spuriously resize the shape box instead of only spAutoFit.
    #[test]
    fn compute_height_only_for_shrink_mode() {
        assert_eq!(
            compute_auto_fit_shape_height_px(Some(TextAutoFitMode::Normal), 400.0, 100.0),
            None
        );
        assert_eq!(
            compute_auto_fit_shape_height_px(Some(TextAutoFitMode::None), 400.0, 100.0),
            None
        );
        assert_eq!(compute_auto_fit_shape_height_px(None, 400.0, 100.0), None);
    }

    // Catches: spAutoFit title bands stay short when measured text is taller than a:ext/@cy.
    #[test]
    fn compute_height_grows_to_measured_content() {
        assert_eq!(
            compute_auto_fit_shape_height_px(Some(TextAutoFitMode::Shrink), 250.0, 100.0),
            Some(250.0)
        );
        assert_eq!(
            compute_auto_fit_shape_height_px(Some(TextAutoFitMode::Shrink), 40.0, 200.0),
            Some(40.0)
        );
    }

    // Catches: spAutoFit with cy=0 spuriously grows text to fit instead of overflowing
    // from authored 0 height (matches TS oracle renderSlideStage).
    #[test]
    fn painted_height_skips_override_when_authored_height_zero() {
        let state = SpAutofitSlideState {
            height_overrides: [("elem1".to_string(), 150.0)].into_iter().collect(),
            last_read_records: vec![],
        };
        // With authored_height 0, override should be ignored
        assert_eq!(state.painted_height("elem1", 0.0), 0.0);
        // With positive authored_height, override applies
        assert_eq!(state.painted_height("elem1", 100.0), 150.0);
        // Non-existent element returns authored_height as-is
        assert_eq!(state.painted_height("elem2", 0.0), 0.0);
        assert_eq!(state.painted_height("elem2", 100.0), 100.0);
    }
}
