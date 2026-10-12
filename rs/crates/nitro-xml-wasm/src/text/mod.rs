// SPDX-License-Identifier: MIT
//! Single-line text-editing surface for the cell editor and formula bar (#55).
//!
//! Upstream xlsx uses a native `<textarea>` for caret, selection, IME and
//! clipboard while editing (`formula-field.ts`). Nitro owns that surface in
//! Rust: grapheme/word/line caret motion, shift-selection, pointer hit-test,
//! and plain/TSV clipboard helpers. Host chrome (formula bar / cell editor)
//! lands in #69; IME composition is #56.

pub mod caret;
pub mod clipboard;
pub mod keys;
pub mod selection;

pub use caret::{caret_rect, offset_at_x, selection_rects};
pub use clipboard::{
    copy_plain, cut_plain, escape_html, parse_tsv, paste_plain, plain_html_table, to_tsv,
    ClipboardPayload,
};
pub use keys::{apply_key, KeyAction, KeyEvent};
pub use selection::TextField;

use wasm_bindgen::prelude::*;

/// Headless eval for differential caret/selection parity vs a `<textarea>` (#55 / #43).
///
/// Offsets are **UTF-16 code units** (JS string indices) so the harness can
/// compare directly with `textarea.selectionStart/End`.
#[wasm_bindgen(js_name = textSurfaceEval)]
pub fn text_surface_eval(script_json: &str) -> Result<String, JsValue> {
    let v: serde_json::Value = serde_json::from_str(script_json)
        .map_err(|e| JsValue::from_str(&format!("bad script json: {e}")))?;
    let text = v
        .get("text")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let anchor_u16 = v.get("anchor").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    let focus_u16 = v.get("focus").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    let char_w = v.get("charWidth").and_then(|x| x.as_f64()).unwrap_or(8.0) as f32;
    let line_h = v.get("lineHeight").and_then(|x| x.as_f64()).unwrap_or(16.0) as f32;

    let mut field = TextField::from_utf16_selection(&text, anchor_u16, focus_u16);
    let mut clipboard = String::new();
    let mut last_payload: Option<ClipboardPayload> = None;
    let ops = v
        .get("ops")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();

    let width_fn = |s: &str| s.chars().count() as f32 * char_w;

    for op in ops {
        let kind = op.get("op").and_then(|x| x.as_str()).unwrap_or("");
        match kind {
            "key" => {
                let key = op.get("key").and_then(|x| x.as_str()).unwrap_or("");
                let ev = KeyEvent {
                    key: key.to_string(),
                    shift: op.get("shift").and_then(|x| x.as_bool()).unwrap_or(false),
                    ctrl: op.get("ctrl").and_then(|x| x.as_bool()).unwrap_or(false),
                    meta: op.get("meta").and_then(|x| x.as_bool()).unwrap_or(false),
                    alt: op.get("alt").and_then(|x| x.as_bool()).unwrap_or(false),
                    is_mac: op.get("mac").and_then(|x| x.as_bool()).unwrap_or(false),
                };
                match apply_key(&mut field, &ev) {
                    KeyAction::Copy => clipboard = copy_plain(&field),
                    KeyAction::Cut => clipboard = cut_plain(&mut field),
                    KeyAction::Paste => {
                        let _ = paste_plain(&mut field, &clipboard);
                    }
                    KeyAction::Handled | KeyAction::Unhandled => {}
                    KeyAction::Commit | KeyAction::Cancel => {}
                }
            }
            "insert" => {
                let t = op.get("text").and_then(|x| x.as_str()).unwrap_or("");
                field.insert(t);
            }
            "pointer" => {
                let x = op.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let extend = op.get("extend").and_then(|v| v.as_bool()).unwrap_or(false);
                let off = offset_at_x(field.text(), x, &width_fn);
                if extend {
                    field.extend_to(off);
                } else {
                    field.set_caret(off);
                }
            }
            "copy" => clipboard = copy_plain(&field),
            "cut" => clipboard = cut_plain(&mut field),
            "paste" => {
                let t = op
                    .get("text")
                    .and_then(|x| x.as_str())
                    .unwrap_or(clipboard.as_str());
                let _ = paste_plain(&mut field, t);
            }
            "copyGrid" => {
                let cell = field.selected_text().to_string();
                let rows = vec![vec![cell.clone()]];
                let tsv = to_tsv(&rows);
                let html = plain_html_table(&rows);
                clipboard = cell;
                last_payload = Some(ClipboardPayload {
                    tsv,
                    html,
                    cut: false,
                });
            }
            "setText" => {
                let t = op.get("text").and_then(|x| x.as_str()).unwrap_or("");
                let caret = op
                    .get("caret")
                    .and_then(|x| x.as_u64())
                    .map(|n| selection::utf16_to_byte(t, n as usize));
                field.set_text(t, caret);
            }
            _ => {}
        }
    }

    let (anchor16, focus16) = field.utf16_selection();
    let caret = caret_rect(field.text(), field.focus(), 0.0, line_h, &width_fn);
    let sels = selection_rects(
        field.text(),
        field.anchor(),
        field.focus(),
        0.0,
        line_h,
        &width_fn,
    );
    let mut out = serde_json::json!({
        "text": field.text(),
        "anchor": anchor16,
        "focus": focus16,
        "selected": field.selected_text(),
        "clipboard": clipboard,
        "escapedSelected": escape_html(field.selected_text()),
        "parsedClipboardRows": parse_tsv(&clipboard),
        "caret": { "x": caret.x, "y": caret.y, "w": caret.w, "h": caret.h },
        "selectionRects": sels.iter().map(|r| serde_json::json!({
            "x": r.x, "y": r.y, "w": r.w, "h": r.h
        })).collect::<Vec<_>>(),
    });
    if let Some(p) = last_payload {
        out["payload"] = serde_json::json!({
            "tsv": p.tsv,
            "html": p.html,
            "cut": p.cut,
        });
    }
    Ok(out.to_string())
}
