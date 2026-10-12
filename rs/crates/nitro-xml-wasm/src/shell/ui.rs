// SPDX-License-Identifier: MIT
//! Compact shell UI snapshot for the thin loader chrome binder (#29).

use serde_json::{json, Value};

use super::chrome::combined_chrome_html;
use super::state::ShellState;
use crate::chrome::EditorChrome;

pub struct ShellUi;

impl ShellUi {
    pub fn to_json(state: &ShellState, editor: &EditorChrome) -> Value {
        let tabs: Vec<Value> = state
            .tabs
            .iter()
            .map(|(index, name)| {
                json!({
                    "index": index,
                    "name": name,
                    "active": *index == state.nav_index,
                })
            })
            .collect();
        let mut find = json!({
            "open": state.find.open,
            "query": state.find.query,
            "status": state.find.status(),
            "index": state.find.index.map(|i| i as u64),
            "count": state.find.matches.len(),
            "caseSensitive": state.find.case_sensitive,
            "matchEntire": state.find.match_entire,
        });
        if let Some(m) = state.find.current() {
            find.as_object_mut().unwrap().insert(
                "current".into(),
                json!({
                    "sheet": m.sheet,
                    "row": m.row,
                    "col": m.col,
                    "text": m.text,
                }),
            );
        }
        let mut diag = state.diagnostics.to_json();
        if let Some(obj) = diag.as_object_mut() {
            obj.insert("open".into(), json!(state.diagnostics_open));
        }
        let pending = state.pending_hyperlink.as_ref().map(|h| {
            let mut m = serde_json::Map::new();
            m.insert("open".into(), json!(h.open));
            if let Some(t) = &h.target {
                m.insert("target".into(), Value::String(t.clone()));
            }
            if let Some(l) = &h.location {
                m.insert("location".into(), Value::String(l.clone()));
            }
            if let Some(tip) = &h.tooltip {
                m.insert("tooltip".into(), Value::String(tip.clone()));
            }
            Value::Object(m)
        });
        json!({
            "format": state.format.as_str(),
            "zoom": state.zoom_pct,
            "find": find,
            "tabs": tabs,
            "diagnostics": diag,
            "selection": state.selection_ref,
            "copyAvailable": !state.selection_ref.is_empty(),
            "pendingHyperlink": pending,
            "lastCopy": state.last_copy,
            // Trusted Rust-built chrome; loader assigns innerHTML only (#29 budget).
            "chromeHtml": combined_chrome_html(state, editor),
        })
    }
}
