// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/bar-cluster-geometry.ts

pub const DEFAULT_BAR_GAP_WIDTH: i32 = 150;

#[derive(Clone, Copy, Debug)]
pub struct ClusteredBarGeometry {
    pub single_bar_width: f64,
    pub step: f64,
    pub cluster_width: f64,
}

pub fn clustered_bar_geometry(
    bar_group_width: f64,
    series_count: usize,
    bar_gap_width: i32,
    bar_overlap: i32,
) -> ClusteredBarGeometry {
    let overlap = bar_overlap as f64;
    let overlap_span = 1.0 + (series_count as f64 - 1.0) * (1.0 - overlap / 100.0);
    let gap_width = bar_gap_width.max(0) as f64;
    let single_bar_width = bar_group_width / (overlap_span + gap_width / 100.0);
    let step = single_bar_width * (1.0 - overlap / 100.0);
    ClusteredBarGeometry {
        single_bar_width,
        step,
        cluster_width: single_bar_width + step * (series_count as f64 - 1.0),
    }
}
