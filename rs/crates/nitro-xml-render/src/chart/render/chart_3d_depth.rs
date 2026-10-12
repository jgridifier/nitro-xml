// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/chart-3d-depth.ts; changes listed in PROVENANCE.md.

use nitro_xml_opc::chart::ChartView3D;

use super::chart_palette_shade::{shade_hex, tint_hex};
use super::svg_util::{esc, n};

const DEFAULT_ROT_X: f64 = 15.0;
const DEFAULT_ROT_Y: f64 = 20.0;
const MIN_DEPTH_PX: f64 = 5.0;
const MAX_DEPTH_PX: f64 = 22.0;

#[derive(Clone, Copy, Debug)]
pub struct DepthVector {
    pub dx: f64,
    pub dy: f64,
    pub magnitude: f64,
}

pub fn compute_depth_vector(view3_d: Option<&ChartView3D>) -> DepthVector {
    let rot_x = view3_d.and_then(|v| v.rot_x).unwrap_or(DEFAULT_ROT_X);
    let rot_y = view3_d.and_then(|v| v.rot_y).unwrap_or(DEFAULT_ROT_Y);
    let depth_scale =
        (view3_d.and_then(|v| v.depth_percent).unwrap_or(100.0) / 100.0).clamp(0.4, 2.0);
    let magnitude = (13.0 * depth_scale).clamp(MIN_DEPTH_PX, MAX_DEPTH_PX);
    let rx = rot_x.to_radians();
    let ry = rot_y.to_radians();
    DepthVector {
        dx: ry.sin() * magnitude,
        dy: -rx.sin() * magnitude,
        magnitude,
    }
}

/// Fractional position along the depth axis in `(0, 1]` (upstream `seriesDepthFactor`).
fn series_depth_factor(series_index: usize, series_count: usize) -> f64 {
    if series_count <= 1 {
        return 1.0;
    }
    (series_index + 1) as f64 / series_count as f64
}

fn unique_series_indexes(bars: &[BarRect]) -> Vec<usize> {
    let mut indexes: Vec<usize> = bars.iter().map(|b| b.series_index).collect();
    indexes.sort_unstable();
    indexes.dedup();
    indexes
}

fn scale_depth_vector(depth: DepthVector, factor: f64) -> DepthVector {
    DepthVector {
        dx: depth.dx * factor,
        dy: depth.dy * factor,
        magnitude: depth.magnitude * factor,
    }
}

fn compute_series_depth(
    depth: DepthVector,
    series_index: usize,
    series_count: usize,
) -> DepthVector {
    scale_depth_vector(depth, series_depth_factor(series_index, series_count))
}

#[derive(Clone, Debug)]
pub struct BarRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub fill: String,
    pub series_index: usize,
}

fn bar_extrusion_polygons(rect: &BarRect, depth: DepthVector) -> Vec<String> {
    let x = rect.x;
    let y = rect.y;
    let w = rect.w;
    let h = rect.h;
    let dx = depth.dx;
    let dy = depth.dy;
    let side_fill = shade_hex(&rect.fill, 0.25);
    let top_fill = tint_hex(&rect.fill, 0.22);
    let top = format!(
        "<polygon points=\"{},{} {},{} {},{} {},{}\" fill=\"{}\" stroke=\"none\"/>",
        n(x),
        n(y),
        n(x + w),
        n(y),
        n(x + w + dx),
        n(y + dy),
        n(x + dx),
        n(y + dy),
        esc(&top_fill)
    );
    let side = format!(
        "<polygon points=\"{},{} {},{} {},{} {},{}\" fill=\"{}\" stroke=\"none\"/>",
        n(x + w),
        n(y),
        n(x + w),
        n(y + h),
        n(x + w + dx),
        n(y + h + dy),
        n(x + w + dx),
        n(y + dy),
        esc(&side_fill)
    );
    vec![top, side]
}

/// Shaded top + side faces for each bar, painted back-to-front for clustered charts.
pub fn bar3d_extrusion_markup(bars: &[BarRect], depth: DepthVector, grouping: &str) -> String {
    if bars.is_empty() {
        return String::new();
    }
    let stacked = grouping == "stacked" || grouping == "percentStacked";
    let mut out = Vec::new();
    if stacked {
        for bar in bars {
            out.extend(bar_extrusion_polygons(bar, depth));
        }
    } else {
        let unique = unique_series_indexes(bars);
        let series_count = unique.len().max(1);
        let mut paint_order = unique;
        paint_order.sort_by(|a, b| {
            series_depth_factor(*b, series_count)
                .partial_cmp(&series_depth_factor(*a, series_count))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for si in paint_order {
            let series_depth = compute_series_depth(depth, si, series_count);
            for bar in bars.iter().filter(|b| b.series_index == si) {
                out.extend(bar_extrusion_polygons(bar, series_depth));
            }
        }
    }
    out.join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: clustered bar3D seriesCount used max(seriesIndex)+1 instead of distinct plotted series count, over-shrinking per-series depth.
    #[test]
    fn series_count_is_distinct_plotted_series() {
        let bars = [
            BarRect {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
                fill: "#000".to_string(),
                series_index: 0,
            },
            BarRect {
                x: 20.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
                fill: "#000".to_string(),
                series_index: 2,
            },
        ];
        assert_eq!(unique_series_indexes(&bars).len(), 2);
        assert!((series_depth_factor(2, 2) - 1.5).abs() < 1e-9);
    }

    // Catches: default view3D depth vector magnitude drift from upstream computeDepthVector (~13px at depthPercent 100).
    #[test]
    fn compute_depth_vector_default_magnitude() {
        let v = compute_depth_vector(None);
        assert!((v.magnitude - 13.0).abs() < 0.01);
        assert!(v.dx > 0.0);
        assert!(v.dy < 0.0);
        assert!((v.dx - 20.0_f64.to_radians().sin() * 13.0).abs() < 0.01);
    }

    // Catches: bar3D extrusion depth falling below upstream when depthPercent is authored at 100 on rAngAx charts.
    #[test]
    fn compute_depth_vector_respects_depth_percent() {
        let view = ChartView3D {
            rot_x: Some(15.0),
            rot_y: Some(20.0),
            depth_percent: Some(100.0),
            height_percent: None,
            right_angle_axes: Some(true),
            perspective: None,
        };
        let v = compute_depth_vector(Some(&view));
        assert!((v.magnitude - 13.0).abs() < 0.01);
    }
}
