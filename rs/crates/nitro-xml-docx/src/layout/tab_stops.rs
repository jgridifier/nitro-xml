// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/tab-stops.ts; changes listed in PROVENANCE.md.

use super::input::{LayoutTabStop, TabLeader};
use super::units::DEFAULT_TAB_STOP_PX;

pub struct TabPlacement {
    pub width_px: f64,
    pub leader: Option<TabLeader>,
}

pub fn place_tab(
    position_px: f64,
    stops: &[LayoutTabStop],
    hanging_stop_px: Option<f64>,
    following_px: f64,
    before_decimal_px: f64,
) -> TabPlacement {
    let epsilon = 0.5;
    let mut filtered: Vec<&LayoutTabStop> = stops
        .iter()
        .filter(|stop| stop.align != "clear" && stop.align != "bar")
        .collect();
    filtered.sort_by(|a, b| a.pos_px.partial_cmp(&b.pos_px).unwrap_or(std::cmp::Ordering::Equal));
    let custom = filtered
        .into_iter()
        .find(|stop| stop.pos_px > position_px + epsilon);
    if let Some(custom) = custom {
        let distance = custom.pos_px - position_px;
        let shift = match custom.align.as_str() {
            "right" | "end" => following_px,
            "center" => following_px / 2.0,
            "decimal" => before_decimal_px,
            _ => 0.0,
        };
        return TabPlacement {
            width_px: (distance - shift).max(0.0),
            leader: custom.leader.as_ref().and_then(|l| {
                if l.as_str() == "none" {
                    None
                } else {
                    Some(l.clone())
                }
            }),
        };
    }
    if let Some(hanging) = hanging_stop_px {
        if hanging > position_px + epsilon {
            return TabPlacement {
                width_px: hanging - position_px,
                leader: None,
            };
        }
    }
    let next = (position_px / DEFAULT_TAB_STOP_PX).floor() + 1.0;
    TabPlacement {
        width_px: next * DEFAULT_TAB_STOP_PX - position_px,
        leader: None,
    }
}
