// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/wrap.ts; changes listed in PROVENANCE.md.

use std::collections::HashMap;

use super::paragraph_layout::LineBoxInsets;
use super::result::LayoutPageBox;

const SIDE_DISTANCE_PX: f64 = 12.0;
const MIN_SIDE_WIDTH_PX: f64 = 48.0;

#[derive(Clone, Debug)]
pub struct Exclusion {
    pub x_px: f64,
    pub y_px: f64,
    pub width_px: f64,
    pub height_px: f64,
    pub top_and_bottom: bool,
}

pub fn wrap_exclusions(pages: &[LayoutPageBox]) -> HashMap<usize, Vec<Exclusion>> {
    let mut exclusions = HashMap::new();
    for page in pages {
        let list = page
            .floats
            .as_ref()
            .map(|floats| {
                floats
                    .iter()
                    .filter(|float| float.wrap.as_deref().is_some_and(|w| w != "none"))
                    .map(|float| Exclusion {
                        x_px: float.x_px - SIDE_DISTANCE_PX,
                        y_px: float.y_px,
                        width_px: float.width_px + SIDE_DISTANCE_PX * 2.0,
                        height_px: float.height_px,
                        top_and_bottom: float.wrap.as_deref() == Some("topAndBottom"),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !list.is_empty() {
            exclusions.insert(page.index, list);
        }
    }
    exclusions
}

pub fn same_exclusions(a: &HashMap<usize, Vec<Exclusion>>, b: &HashMap<usize, Vec<Exclusion>>) -> bool {
    fn key(map: &HashMap<usize, Vec<Exclusion>>) -> String {
        let mut entries: Vec<_> = map.iter().collect();
        entries.sort_by_key(|(k, _)| *k);
        serde_json::to_string(
            &entries
                .iter()
                .map(|(page, list)| {
                    (
                        page,
                        list
                            .iter()
                            .map(|item| {
                                (
                                    item.x_px.round() as i64,
                                    item.y_px.round() as i64,
                                    item.top_and_bottom,
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default()
    }
    key(a) == key(b)
}

pub fn line_boxes_for(
    exclusions: &[Exclusion],
    top_px: f64,
    column_left_px: f64,
    column_width_px: f64,
) -> Box<dyn Fn(f64, f64) -> LineBoxInsets + '_> {
    Box::new(move |y_px, height_px| {
        let top = top_px + y_px;
        let mut left_inset_px = 0.0f64;
        let mut right_inset_px = 0.0f64;
        let mut gap_before_px = 0.0f64;
        for exclusion in exclusions {
            let overlaps_line =
                exclusion.y_px < top + height_px && exclusion.y_px + exclusion.height_px > top;
            let start = exclusion.x_px - column_left_px;
            let end = start + exclusion.width_px;
            if !overlaps_line || end <= 0.0 || start >= column_width_px {
                continue;
            }
            let left: f64 = start.max(0.0);
            let right: f64 = (column_width_px - end).max(0.0);
            if exclusion.top_and_bottom || left.max(right) < MIN_SIDE_WIDTH_PX {
                gap_before_px = gap_before_px.max(exclusion.y_px + exclusion.height_px - top);
                continue;
            }
            if left >= right {
                right_inset_px = right_inset_px.max(column_width_px - start);
            } else {
                left_inset_px = left_inset_px.max(end);
            }
        }
        LineBoxInsets {
            left_inset_px,
            right_inset_px,
            gap_before_px,
        }
    })
}
