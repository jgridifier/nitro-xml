// SPDX-License-Identifier: MIT
//! Rich multi-paragraph text surface for Word and slide text (#61).
//!
//! Builds on the single-line surface (#55). Host chrome / model write-back
//! land with E3 (#78) and E4 (#85).
//!
//! TODO(#56): IME · TODO(#57): bidi · TODO(#59): a11y · TODO(#58): spellcheck.

pub mod caret;
pub mod insert;
pub mod keys;
pub mod model;
pub mod position;
pub mod selection;

pub use caret::caret_rect;
pub use insert::{insert_text, split_paragraph, style_at};
pub use keys::{apply_rich_key, insert_printable, RichKeyAction};
pub use model::{RichBlock, RichCell, RichDocument, RichParagraph, RichRun, RichTable, RunStyle};
pub use position::{ParaPath, RichPos};
pub use selection::{is_cross_paragraph, selected_plain, RichSelection};

use wasm_bindgen::prelude::*;

use crate::text::keys::KeyEvent;

/// Headless eval for rich caret/selection parity (#61 / #43 tolerance via synthetic rects).
#[wasm_bindgen(js_name = richTextSurfaceEval)]
pub fn rich_text_surface_eval(script_json: &str) -> Result<String, JsValue> {
    let v: serde_json::Value = serde_json::from_str(script_json)
        .map_err(|e| JsValue::from_str(&format!("bad script: {e}")))?;

    let mut doc = parse_doc(&v)?;
    let mut sel = parse_sel(&v, &doc);
    let char_w = v.get("charWidth").and_then(|x| x.as_f64()).unwrap_or(8.0) as f32;
    let line_h = v.get("lineHeight").and_then(|x| x.as_f64()).unwrap_or(16.0) as f32;
    let mut clipboard = String::new();
    let readonly = v.get("readOnly").and_then(|x| x.as_bool()).unwrap_or(false);

    let ops = v
        .get("ops")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();

    for op in ops {
        if readonly {
            // Negative control: readonly must not mutate.
            continue;
        }
        let kind = op.get("op").and_then(|x| x.as_str()).unwrap_or("");
        match kind {
            "key" => {
                let ev = KeyEvent {
                    key: op
                        .get("key")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    shift: op.get("shift").and_then(|x| x.as_bool()).unwrap_or(false),
                    ctrl: op.get("ctrl").and_then(|x| x.as_bool()).unwrap_or(false),
                    meta: op.get("meta").and_then(|x| x.as_bool()).unwrap_or(false),
                    alt: op.get("alt").and_then(|x| x.as_bool()).unwrap_or(false),
                    is_mac: op.get("mac").and_then(|x| x.as_bool()).unwrap_or(false),
                };
                match apply_rich_key(&mut doc, &mut sel, &ev) {
                    RichKeyAction::Copy => clipboard = selected_plain(&doc, &sel),
                    RichKeyAction::Cut => {
                        clipboard = selected_plain(&doc, &sel);
                        insert::delete_selection(&mut doc, &mut sel);
                    }
                    RichKeyAction::Paste => insert_text(&mut doc, &mut sel, &clipboard),
                    RichKeyAction::Handled | RichKeyAction::Unhandled => {}
                }
            }
            "insert" => {
                let t = op.get("text").and_then(|x| x.as_str()).unwrap_or("");
                insert_printable(&mut doc, &mut sel, t);
            }
            "split" => split_paragraph(&mut doc, &mut sel),
            _ => {}
        }
    }

    let caret = caret_rect(&doc, &sel.focus, char_w, line_h);
    let style = style_at(&doc, &sel.focus);
    Ok(serde_json::json!({
        "plain": doc.plain_text(),
        "anchor": pos_json(&sel.anchor),
        "focus": pos_json(&sel.focus),
        "selected": selected_plain(&doc, &sel),
        "crossParagraph": is_cross_paragraph(&sel),
        "styleAtCaret": {
            "bold": style.bold,
            "italic": style.italic,
        },
        "caret": { "x": caret.x, "y": caret.y, "w": caret.w, "h": caret.h },
        "clipboard": clipboard,
        "readOnly": readonly,
        "blockCount": doc.blocks.len(),
    })
    .to_string())
}

fn pos_json(p: &RichPos) -> serde_json::Value {
    match &p.path {
        ParaPath::Block { block } => serde_json::json!({
            "kind": "block", "block": block, "offset": p.offset
        }),
        ParaPath::Cell {
            block,
            row,
            col,
            para,
        } => serde_json::json!({
            "kind": "cell", "block": block, "row": row, "col": col, "para": para, "offset": p.offset
        }),
    }
}

fn parse_doc(v: &serde_json::Value) -> Result<RichDocument, JsValue> {
    if let Some(paras) = v.get("paragraphs").and_then(|x| x.as_array()) {
        let mut blocks = Vec::new();
        for p in paras {
            if let Some(runs) = p.get("runs").and_then(|x| x.as_array()) {
                let mut rs = Vec::new();
                for r in runs {
                    let text = r.get("text").and_then(|x| x.as_str()).unwrap_or("");
                    let bold = r.get("bold").and_then(|x| x.as_bool()).unwrap_or(false);
                    let italic = r.get("italic").and_then(|x| x.as_bool()).unwrap_or(false);
                    rs.push(RichRun::styled(
                        text,
                        RunStyle {
                            bold,
                            italic,
                            ..RunStyle::default()
                        },
                    ));
                }
                if rs.is_empty() {
                    rs.push(RichRun::plain(""));
                }
                blocks.push(RichBlock::Paragraph(RichParagraph { runs: rs }));
            } else {
                let t = p.as_str().unwrap_or("");
                blocks.push(RichBlock::Paragraph(RichParagraph::from_plain(t)));
            }
        }
        if blocks.is_empty() {
            return Ok(RichDocument::empty());
        }
        return Ok(RichDocument { blocks });
    }
    if let Some(table) = v.get("table").and_then(|x| x.as_array()) {
        let mut rows = Vec::new();
        for row in table {
            let mut cells = Vec::new();
            for cell in row.as_array().cloned().unwrap_or_default() {
                let t = cell.as_str().unwrap_or("");
                cells.push(RichCell::from_plain(t));
            }
            rows.push(cells);
        }
        return Ok(RichDocument {
            blocks: vec![RichBlock::Table(RichTable { rows })],
        });
    }
    Ok(RichDocument::from_paragraphs(&[""]))
}

fn parse_sel(v: &serde_json::Value, doc: &RichDocument) -> RichSelection {
    let focus = parse_pos(v.get("focus")).unwrap_or_else(|| RichPos::block_start(0));
    let anchor = parse_pos(v.get("anchor")).unwrap_or_else(|| focus.clone());
    selection::clamp_selection(doc, &RichSelection { anchor, focus })
}

fn parse_pos(v: Option<&serde_json::Value>) -> Option<RichPos> {
    let v = v?;
    let kind = v.get("kind").and_then(|x| x.as_str()).unwrap_or("block");
    let offset = v.get("offset").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    match kind {
        "cell" => Some(RichPos {
            path: ParaPath::Cell {
                block: v.get("block").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
                row: v.get("row").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
                col: v.get("col").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
                para: v.get("para").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
            },
            offset,
        }),
        _ => Some(RichPos {
            path: ParaPath::Block {
                block: v.get("block").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
            },
            offset,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_readonly_ops_do_not_mutate_document() {
        // Catches: readonly rich surface must ignore insert/key ops (negative control #61).
        let script = r#"{
            "readOnly": true,
            "paragraphs": ["hello"],
            "anchor": {"kind":"block","block":0,"offset":5},
            "focus": {"kind":"block","block":0,"offset":5},
            "ops": [{"op":"insert","text":"X"}, {"op":"key","key":"Backspace"}]
        }"#;
        let out: serde_json::Value =
            serde_json::from_str(&rich_text_surface_eval(script).unwrap()).unwrap();
        assert_eq!(out["plain"], "hello");
        assert_eq!(out["focus"]["offset"], 5);
    }

    #[test]
    fn catches_shift_selection_reports_cross_paragraph() {
        // Catches: shift-select across ¶ must set crossParagraph and selected text with \\n.
        let script = r#"{
            "paragraphs": ["ab", "cd"],
            "anchor": {"kind":"block","block":0,"offset":1},
            "focus": {"kind":"block","block":0,"offset":1},
            "ops": [
                {"op":"key","key":"ArrowRight","shift":true},
                {"op":"key","key":"ArrowRight","shift":true},
                {"op":"key","key":"ArrowRight","shift":true}
            ]
        }"#;
        let out: serde_json::Value =
            serde_json::from_str(&rich_text_surface_eval(script).unwrap()).unwrap();
        assert_eq!(out["crossParagraph"], true);
        assert_eq!(out["selected"], "b\nc");
    }
}
