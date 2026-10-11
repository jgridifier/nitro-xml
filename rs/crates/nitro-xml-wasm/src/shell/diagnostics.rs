// SPDX-License-Identifier: MIT
//! Diagnostics panel data (#29), including #121 font-substitution records.

use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Default)]
pub struct DiagnosticItem {
    pub code: String,
    pub message: String,
    pub part: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DiagnosticsState {
    pub parse_ms: f64,
    pub paint_ms: f64,
    pub warnings: Vec<DiagnosticItem>,
    pub font_substitutions: Vec<DiagnosticItem>,
    pub unsupported: Vec<DiagnosticItem>,
}

impl DiagnosticsState {
    pub fn clear_runtime(&mut self) {
        self.warnings.clear();
        self.font_substitutions.clear();
        self.unsupported.clear();
    }

    pub fn ingest_warning(&mut self, code: &str, message: &str, part: Option<&str>) {
        let item = DiagnosticItem {
            code: code.into(),
            message: message.into(),
            part: part.map(str::to_owned),
        };
        if code == "font_substitution" {
            self.font_substitutions.push(item);
        } else if code == "unsupported_content" || code == "unsupported_format" {
            self.unsupported.push(item);
        } else {
            self.warnings.push(item);
        }
    }

    pub fn to_json(&self) -> Value {
        let map_item = |i: &DiagnosticItem| {
            let mut m = Map::new();
            m.insert("code".into(), Value::String(i.code.clone()));
            m.insert("message".into(), Value::String(i.message.clone()));
            if let Some(p) = &i.part {
                m.insert("part".into(), Value::String(p.clone()));
            }
            Value::Object(m)
        };
        json!({
            "parseMs": self.parse_ms,
            "paintMs": self.paint_ms,
            "warnings": self.warnings.iter().map(map_item).collect::<Vec<_>>(),
            "fontSubstitutions": self.font_substitutions.iter().map(map_item).collect::<Vec<_>>(),
            "unsupported": self.unsupported.iter().map(map_item).collect::<Vec<_>>(),
        })
    }
}
