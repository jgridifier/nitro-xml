// SPDX-License-Identifier: MIT
//! Browser/OS spellcheck bridge for the Rust text surface (#58).
//!
//! Nitro does not ship a dictionary or rank suggestions. The host syncs plain
//! text and UTF-16 selection into a hidden `<textarea>` or `contenteditable`
//! with `spellcheck` toggled like upstream (`formula-field.ts`, inline editor).
//! Misspelled UTF-16 ranges come back from the host checker; this module turns
//! them into underline geometry via the same `selection_rects` path as highlights.

use serde_json::json;
use wasm_bindgen::prelude::*;

use super::caret::{selection_rects, WidthFn};
use super::selection::utf16_to_byte;

/// User-facing spellcheck toggle and BCP-47 language tag for the OS checker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpellcheckState {
    pub enabled: bool,
    pub lang: String,
}

impl Default for SpellcheckState {
    fn default() -> Self {
        Self {
            enabled: true,
            lang: String::new(),
        }
    }
}

impl SpellcheckState {
    pub fn new(enabled: bool, lang: impl Into<String>) -> Self {
        Self {
            enabled,
            lang: lang.into(),
        }
    }

    /// DOM `spellcheck` attribute / property (upstream mirrors this boolean).
    pub fn dom_spellcheck_attr(&self) -> bool {
        self.enabled
    }

    /// Snapshot the host bridge reads each frame to sync the hidden input.
    pub fn bridge_snapshot(
        &self,
        text: &str,
        anchor_u16: usize,
        focus_u16: usize,
    ) -> SpellBridgeSnapshot {
        SpellBridgeSnapshot {
            text: text.to_string(),
            anchor_u16,
            focus_u16,
            enabled: self.enabled,
            lang: self.lang.clone(),
        }
    }
}

/// Plain-text + selection state mirrored into the browser spellchecker surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpellBridgeSnapshot {
    pub text: String,
    pub anchor_u16: usize,
    pub focus_u16: usize,
    pub enabled: bool,
    pub lang: String,
}

/// Red squiggle rects for misspelled UTF-16 ranges (empty when spellcheck is off).
pub fn underline_rects(
    text: &str,
    misspells_utf16: &[(usize, usize)],
    y: f32,
    line_height: f32,
    width_of: &WidthFn<'_>,
    enabled: bool,
) -> Vec<super::caret::CaretRect> {
    if !enabled {
        return Vec::new();
    }
    let mut out = Vec::new();
    for &(start_u16, end_u16) in misspells_utf16 {
        if start_u16 >= end_u16 {
            continue;
        }
        let start = utf16_to_byte(text, start_u16);
        let end = utf16_to_byte(text, end_u16);
        out.extend(selection_rects(text, start, end, y, line_height, width_of));
    }
    out
}

/// Headless eval for spellcheck toggle + underline geometry vs textarea/CE oracle (#58).
#[wasm_bindgen(js_name = spellcheckEval)]
pub fn spellcheck_eval(script_json: &str) -> Result<String, JsValue> {
    let v: serde_json::Value = serde_json::from_str(script_json)
        .map_err(|e| JsValue::from_str(&format!("bad script json: {e}")))?;
    let mut text = v
        .get("text")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let mut anchor_u16 = v.get("anchor").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    let mut focus_u16 = v.get("focus").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    let mut state = SpellcheckState::new(
        v.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true),
        v.get("lang")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
    );
    let char_w = v.get("charWidth").and_then(|x| x.as_f64()).unwrap_or(8.0) as f32;
    let line_h = v.get("lineHeight").and_then(|x| x.as_f64()).unwrap_or(16.0) as f32;
    let mut misspells: Vec<(usize, usize)> = v
        .get("misspells")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let s = m.get("start").and_then(|x| x.as_u64())? as usize;
                    let e = m.get("end").and_then(|x| x.as_u64())? as usize;
                    Some((s, e))
                })
                .collect()
        })
        .unwrap_or_default();

    let ops = v
        .get("ops")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    for op in ops {
        let kind = op.get("op").and_then(|x| x.as_str()).unwrap_or("");
        match kind {
            "setEnabled" => {
                state.enabled = op.get("enabled").and_then(|x| x.as_bool()).unwrap_or(false);
            }
            "setLang" => {
                state.lang = op
                    .get("lang")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            "setText" => {
                text = op
                    .get("text")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                if let Some(c) = op.get("caret").and_then(|x| x.as_u64()) {
                    anchor_u16 = c as usize;
                    focus_u16 = c as usize;
                }
            }
            "setSelection" => {
                anchor_u16 = op.get("anchor").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                focus_u16 = op.get("focus").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
            }
            "setMisspells" => {
                misspells = op
                    .get("misspells")
                    .and_then(|x| x.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| {
                                let s = m.get("start").and_then(|x| x.as_u64())? as usize;
                                let e = m.get("end").and_then(|x| x.as_u64())? as usize;
                                Some((s, e))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            }
            _ => {}
        }
    }

    let width_fn = |s: &str| s.chars().count() as f32 * char_w;
    let rects = underline_rects(&text, &misspells, 0.0, line_h, &width_fn, state.enabled);
    let bridge = state.bridge_snapshot(&text, anchor_u16, focus_u16);

    let out = json!({
        "text": text,
        "anchor": anchor_u16,
        "focus": focus_u16,
        "enabled": state.enabled,
        "lang": state.lang,
        "spellcheckAttr": state.dom_spellcheck_attr(),
        "bridge": {
            "text": bridge.text,
            "anchor": bridge.anchor_u16,
            "focus": bridge.focus_u16,
            "enabled": bridge.enabled,
            "lang": bridge.lang,
        },
        "underlineRects": rects.iter().map(|r| json!({
            "x": r.x, "y": r.y, "w": r.w, "h": r.h
        })).collect::<Vec<_>>(),
    });
    Ok(out.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mono(s: &str) -> f32 {
        s.chars().count() as f32 * 8.0
    }

    #[test]
    fn catches_disabled_spellcheck_emits_no_underline_rects() {
        // Catches: spellcheck off must not paint squiggles even when misspell ranges are supplied.
        let text = "asdf qwerty";
        let miss = [(0usize, 4usize)];
        let rects = underline_rects(text, &miss, 0.0, 16.0, &mono, false);
        assert!(rects.is_empty());
        assert!(!SpellcheckState::new(false, "en").dom_spellcheck_attr());
    }

    #[test]
    fn catches_misspell_underline_matches_selection_geometry() {
        // Catches: misspell underline must use the same width math as selection highlights.
        let text = "hello zzorld";
        let miss = [(6usize, 12usize)];
        let u = underline_rects(text, &miss, 0.0, 16.0, &mono, true);
        let s = selection_rects(
            text,
            utf16_to_byte(text, 6),
            utf16_to_byte(text, 12),
            0.0,
            16.0,
            &mono,
        );
        assert_eq!(u.len(), s.len());
        assert_eq!(u[0].x, s[0].x);
        assert_eq!(u[0].w, s[0].w);
    }

    #[test]
    fn catches_bridge_snapshot_round_trips_utf16_offsets() {
        // Catches: bridge must expose UTF-16 selection indices for the hidden textarea sync.
        use super::super::selection::{byte_to_utf16, TextField};
        let text = "a😀b";
        let state = SpellcheckState::new(true, "en-US");
        let snap = state.bridge_snapshot(text, 1, 3);
        assert_eq!(snap.text, text);
        assert_eq!(snap.anchor_u16, 1);
        assert_eq!(snap.focus_u16, 3);
        let field = TextField::from_utf16_selection(text, 1, 3);
        assert_eq!(byte_to_utf16(text, field.anchor()), 1);
    }
}
