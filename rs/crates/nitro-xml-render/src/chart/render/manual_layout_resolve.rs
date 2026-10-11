// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/manual-layout.ts (resolve subset)

use nitro_xml_opc::chart::ChartManualLayout;

pub fn has_manual_layout_fields(layout: Option<&ChartManualLayout>) -> bool {
    layout
        .is_some_and(|l| l.x.is_some() || l.y.is_some() || l.width.is_some() || l.height.is_some())
}

#[derive(Clone, Copy, Debug)]
pub struct ChartLayoutRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn resolve_manual_layout_rect(
    layout: Option<&ChartManualLayout>,
    frame_w: f64,
    frame_h: f64,
    auto: ChartLayoutRect,
) -> Option<ChartLayoutRect> {
    let layout = layout?;
    if layout.x.is_none() && layout.y.is_none() && layout.width.is_none() && layout.height.is_none()
    {
        return None;
    }
    let x = layout
        .x
        .map(|v| {
            if layout.x_mode.as_deref() == Some("edge") {
                v * frame_w
            } else {
                auto.x + v * frame_w
            }
        })
        .unwrap_or(auto.x);
    let y = layout
        .y
        .map(|v| {
            if layout.y_mode.as_deref() == Some("edge") {
                v * frame_h
            } else {
                auto.y + v * frame_h
            }
        })
        .unwrap_or(auto.y);
    let width = layout
        .width
        .map(|v| {
            if layout.width_mode.as_deref() == Some("edge") {
                v * frame_w - x
            } else {
                v * frame_w
            }
        })
        .unwrap_or(auto.width);
    let height = layout
        .height
        .map(|v| {
            if layout.height_mode.as_deref() == Some("edge") {
                v * frame_h - y
            } else {
                v * frame_h
            }
        })
        .unwrap_or(auto.height);
    Some(ChartLayoutRect {
        x,
        y,
        width: width.max(1.0),
        height: height.max(1.0),
    })
}
