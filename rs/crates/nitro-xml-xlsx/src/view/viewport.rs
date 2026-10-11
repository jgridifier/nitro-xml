// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/viewport.ts (scroll bounds helpers);
// changes listed in PROVENANCE.md.
//! Viewport helpers for grid painters (#18). Full visible-cell iteration is TODO(#19) wasm handle wiring.

use crate::view::metrics::GridMetrics;

/// Clamp dump bounds to the oracle's view-IR limits (used range margins, max 200 x 60).
pub fn view_ir_bounds(metrics: &GridMetrics, used_end_row: u32, used_end_col: u32) -> (u32, u32) {
    let last_row = metrics.last_row.min(used_end_row + 5).min(200);
    let last_col = metrics.last_col.min(used_end_col + 2).min(60);
    (last_row, last_col)
}
