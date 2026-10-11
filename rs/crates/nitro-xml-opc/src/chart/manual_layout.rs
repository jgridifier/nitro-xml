// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/manual-layout.ts; changes listed in PROVENANCE.md.

use super::model::ChartManualLayout;

pub fn read_chart_manual_layout_values(
    value: impl Fn(&str) -> Option<String>,
) -> Option<ChartManualLayout> {
    let mut result = ChartManualLayout {
        layout_target: None,
        x_mode: None,
        y_mode: None,
        width_mode: None,
        height_mode: None,
        x: None,
        y: None,
        width: None,
        height: None,
    };
    let enumeration = |name: &str, values: &[&str]| -> Option<String> {
        let raw = value(name)?;
        if values.contains(&raw.as_str()) {
            Some(raw)
        } else {
            None
        }
    };
    result.layout_target = enumeration("layoutTarget", &["inner", "outer"]);
    for (xml, key) in [
        ("xMode", "x_mode"),
        ("yMode", "y_mode"),
        ("wMode", "width_mode"),
        ("hMode", "height_mode"),
    ] {
        let mode = enumeration(xml, &["edge", "factor"]);
        if let Some(m) = mode {
            match key {
                "x_mode" => result.x_mode = Some(m),
                "y_mode" => result.y_mode = Some(m),
                "width_mode" => result.width_mode = Some(m),
                "h_mode" => result.height_mode = Some(m),
                _ => {}
            }
        }
    }
    for (xml, assign) in [("x", 0), ("y", 1), ("w", 2), ("h", 3)] {
        if let Some(raw) = value(xml) {
            if !raw.trim().is_empty() {
                if let Ok(number) = raw.parse::<f64>() {
                    if number.is_finite() {
                        match assign {
                            0 => result.x = Some(number),
                            1 => result.y = Some(number),
                            2 => result.width = Some(number),
                            3 => result.height = Some(number),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    if result.layout_target.is_none()
        && result.x_mode.is_none()
        && result.y_mode.is_none()
        && result.width_mode.is_none()
        && result.height_mode.is_none()
        && result.x.is_none()
        && result.y.is_none()
        && result.width.is_none()
        && result.height.is_none()
    {
        None
    } else {
        Some(result)
    }
}
