// SPDX-License-Identifier: MIT
//! Layout-side chart paint stub (full SVG paint is #28).
//!
//! Upstream `docxChartPaint` returns `{ svg?, label }`. Until the shared chart
//! painter lands, we emit the same label strings (title / undrawn family) so
//! LayoutResult line metrics stay comparable after SVG is stripped in normalize.

use serde_json::{json, Value};

pub fn docx_chart_paint(chart: &Value, _width_px: f64, _height_px: f64, _theme: Option<&Value>) -> Value {
    // TODO(#28): renderChartSpaceSvg via nitro-xml-render::chart.
    let space = chart.get("chartSpace");
    if space.is_none() {
        return json!({ "label": "Chart" });
    }
    let kind = space
        .and_then(|s| s.get("type"))
        .and_then(|v| v.as_str())
        .or_else(|| chart.get("kind").and_then(|v| v.as_str()));
    let drawn = space
        .and_then(|s| s.get("drawn"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    if !drawn {
        return json!({
            "label": kind.map(|t| format!("Chart ({t})")).unwrap_or_else(|| "Chart".into())
        });
    }
    if let Some(title) = chart.get("title").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
        return json!({ "label": format!("Chart: {title}") });
    }
    json!({ "label": "Chart" })
}
