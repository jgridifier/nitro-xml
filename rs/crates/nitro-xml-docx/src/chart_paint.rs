// SPDX-License-Identifier: MIT
//! Stub chart paint for layout adapter (full paint in #28).

use serde_json::{json, Value};

pub fn docx_chart_paint(_chart: &Value, _width_px: f64, _height_px: f64, _theme: Option<&Value>) -> Value {
    json!({ "label": "Chart" })
}
