// SPDX-License-Identifier: MIT
//! Wasm glue for the docx editor session: open, chrome commands, headless edits, save.

use nitro_xml_docx::{save_docx, DocxEditor};
use serde_json::Value;

use crate::docx::DocxState;
use crate::office_edit::{command_ribbon, is_text_key, OfficeEffect, RibbonItem};

/// What a key did at the caret: nothing, moved it (no relayout), or changed the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
    Ignored,
    Moved,
    Edited,
}

/// The last coalescible edit, so consecutive typing / deleting folds into one undo step.
pub struct Typing {
    kind: &'static str,
    block_id: String,
    offset: usize,
    done_len: usize,
}

fn paragraph_text(block: &Value) -> String {
    block
        .get("runs")
        .and_then(|r| r.as_array())
        .map(|rs| {
            rs.iter()
                .map(|r| r.get("text").and_then(|t| t.as_str()).unwrap_or(""))
                .collect()
        })
        .unwrap_or_default()
}

fn prev_boundary(text: &str, mut off: usize) -> usize {
    off = off.saturating_sub(1);
    while off > 0 && !text.is_char_boundary(off) {
        off -= 1;
    }
    off
}

fn next_boundary(text: &str, mut off: usize) -> usize {
    off += 1;
    while off < text.len() && !text.is_char_boundary(off) {
        off += 1;
    }
    off.min(text.len())
}

impl DocxState {
    pub fn ensure_edit(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.edit.is_some() {
            return Ok(());
        }
        let mut edit = DocxEditor::open(bytes)?;
        edit.set_read_only(false);
        self.edit = Some(edit);
        Ok(())
    }

    pub fn save_bytes(&self, original: &[u8]) -> Result<Vec<u8>, String> {
        match &self.edit {
            // Keep the open-package bytes as the writer baseline so undo restores them.
            Some(edit) => save_docx(&edit.session.host, original),
            None => Ok(original.to_vec()),
        }
    }

    pub fn dispatch_chrome(&mut self, id: &str) -> Result<OfficeEffect, String> {
        self.typing = None;
        let id = canonical_command(id);
        if id == "file.save" {
            return Ok(OfficeEffect::Save);
        }
        if matches!(id, "cut" | "copy" | "paste") {
            return self.clipboard_command(id);
        }
        let edit = self
            .edit
            .as_mut()
            .ok_or_else(|| "no edit session".to_string())?;
        edit.dispatch_command(id, &Value::Null)
            .map_err(|e| e.to_string())?;
        Ok(OfficeEffect::Applied)
    }

    /// Copy, cut, or paste the caret paragraph's plain text. Copy does not touch the package.
    /// Cut and paste are one history step each, so edit.undo restores the paragraph.
    fn clipboard_command(&mut self, id: &str) -> Result<OfficeEffect, String> {
        let (bi, off, text) = self
            .caret_paragraph()
            .ok_or_else(|| "no paragraph".to_string())?;
        match id {
            "copy" => {
                self.text_clip = text.clone();
                Ok(OfficeEffect::Clipboard {
                    text,
                    changed: false,
                })
            }
            "cut" => {
                self.text_clip = text.clone();
                if text.is_empty() {
                    return Ok(OfficeEffect::Clipboard {
                        text,
                        changed: false,
                    });
                }
                let spec = serde_json::json!({
                    "command": "deleteText",
                    "blockIndex": bi,
                    "start": 0,
                    "end": text.len(),
                });
                self.apply_edit_spec(&spec.to_string())?;
                self.set_caret(bi, 0);
                Ok(OfficeEffect::Clipboard {
                    text: self.text_clip.clone(),
                    changed: true,
                })
            }
            "paste" => {
                let clip = self.text_clip.clone();
                if clip.is_empty() {
                    return Ok(OfficeEffect::Clipboard {
                        text: String::new(),
                        changed: false,
                    });
                }
                let spec = serde_json::json!({
                    "command": "insertText",
                    "blockIndex": bi,
                    "offset": off,
                    "text": clip,
                });
                self.apply_edit_spec(&spec.to_string())?;
                self.set_caret(bi, off + self.text_clip.len());
                Ok(OfficeEffect::Applied)
            }
            _ => Err(format!("unknown clipboard command {id}")),
        }
    }

    pub fn apply_raw_edit(&mut self, spec: &str) -> Result<OfficeEffect, String> {
        self.typing = None;
        self.apply_edit_spec(spec)
    }

    fn apply_edit_spec(&mut self, spec: &str) -> Result<OfficeEffect, String> {
        let v: Value = serde_json::from_str(spec).map_err(|e| e.to_string())?;
        let command = v
            .get("command")
            .and_then(|c| c.as_str())
            .ok_or_else(|| "missing command".to_string())?
            .to_owned();
        let command = canonical_command(&command).to_owned();
        if command == "file.save" {
            return Ok(OfficeEffect::Save);
        }
        let mut args = v;
        if let Some(obj) = args.as_object_mut() {
            obj.remove("command");
        }
        let edit = self
            .edit
            .as_mut()
            .ok_or_else(|| "no edit session".to_string())?;
        edit.dispatch_command(&command, &args)
            .map_err(|e| e.to_string())?;
        Ok(OfficeEffect::Applied)
    }

    /// Keys at the caret: typing, Enter, Backspace/Delete (joining paragraphs at the edges) and
    /// Left/Right. Up/Down/Home/End need line geometry, so the loader resolves those from the DOM.
    pub fn type_key(
        &mut self,
        key: &str,
        ctrl: bool,
        meta: bool,
        alt: bool,
    ) -> Result<KeyOutcome, String> {
        let printable = is_text_key(key) && key != "Backspace" && key != "Delete";
        let known = printable
            || matches!(
                key,
                "Backspace" | "Delete" | "Enter" | "ArrowLeft" | "ArrowRight"
            );
        if ctrl || meta || alt || !known {
            return Ok(KeyOutcome::Ignored);
        }
        let Some((bi, off, text)) = self.caret_paragraph() else {
            return Ok(KeyOutcome::Ignored);
        };
        let prev_para = |s: &Self| s.paragraph_before(bi);
        let next_para = |s: &Self| s.paragraph_after(bi);
        let (kind, spec, caret): (&str, Option<serde_json::Value>, (usize, usize)) = match key {
            "ArrowLeft" => {
                let to = if off > 0 {
                    (bi, prev_boundary(&text, off))
                } else if let Some(p) = prev_para(self) {
                    (p, self.paragraph_len(p))
                } else {
                    (bi, 0)
                };
                ("", None, to)
            }
            "ArrowRight" => {
                let to = if off < text.len() {
                    (bi, next_boundary(&text, off))
                } else if let Some(n) = next_para(self) {
                    (n, 0)
                } else {
                    (bi, text.len())
                };
                ("", None, to)
            }
            "Enter" => (
                "",
                Some(serde_json::json!({
                    "command": "splitParagraph", "blockIndex": bi, "offset": off,
                })),
                (bi + 1, 0),
            ),
            "Backspace" if off > 0 => {
                let start = prev_boundary(&text, off);
                (
                    "del",
                    Some(serde_json::json!({
                        "command": "deleteText", "blockIndex": bi, "start": start, "end": off,
                    })),
                    (bi, start),
                )
            }
            "Backspace" => match prev_para(self) {
                Some(p) => (
                    "",
                    Some(serde_json::json!({ "command": "joinParagraphs", "blockIndex": p })),
                    (p, self.paragraph_len(p)),
                ),
                None => return Ok(KeyOutcome::Moved),
            },
            "Delete" if off < text.len() => (
                "del",
                Some(serde_json::json!({
                    "command": "deleteText", "blockIndex": bi,
                    "start": off, "end": next_boundary(&text, off),
                })),
                (bi, off),
            ),
            "Delete" => match next_para(self) {
                Some(_) => (
                    "",
                    Some(serde_json::json!({ "command": "joinParagraphs", "blockIndex": bi })),
                    (bi, off),
                ),
                None => return Ok(KeyOutcome::Moved),
            },
            _ => (
                "ins",
                Some(serde_json::json!({
                    "command": "insertText", "blockIndex": bi, "offset": off, "text": key,
                })),
                (bi, off + key.len()),
            ),
        };
        let Some(spec) = spec else {
            self.typing = None;
            self.set_caret(caret.0, caret.1);
            return Ok(KeyOutcome::Moved);
        };
        let block_id = self.block_id(bi);
        let continues = !kind.is_empty()
            && self.typing.as_ref().is_some_and(|t| {
                t.kind == kind
                    && t.block_id == block_id
                    && t.offset == off
                    && Some(t.done_len) == self.done_len()
            });
        self.apply_edit_spec(&spec.to_string())?;
        if continues {
            if let Some(edit) = self.edit.as_mut() {
                edit.session.history.merge_last_into_previous();
            }
        }
        self.set_caret(caret.0, caret.1);
        self.typing = (!kind.is_empty()).then(|| Typing {
            kind,
            block_id: self.block_id(caret.0),
            offset: caret.1,
            done_len: self.done_len().unwrap_or(0),
        });
        Ok(KeyOutcome::Edited)
    }

    fn done_len(&self) -> Option<usize> {
        self.edit.as_ref().map(|e| e.session.history.done().len())
    }

    fn block_id(&self, bi: usize) -> String {
        self.edit
            .as_ref()
            .and_then(|e| e.session.host.blocks.get(bi))
            .and_then(|b| b.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned()
    }

    fn is_paragraph(&self, bi: usize) -> bool {
        self.edit
            .as_ref()
            .and_then(|e| e.session.host.blocks.get(bi))
            .and_then(|b| b.get("type"))
            .and_then(|t| t.as_str())
            == Some("paragraph")
    }

    fn paragraph_before(&self, bi: usize) -> Option<usize> {
        (bi > 0 && self.is_paragraph(bi - 1)).then(|| bi - 1)
    }

    fn paragraph_after(&self, bi: usize) -> Option<usize> {
        self.is_paragraph(bi + 1).then_some(bi + 1)
    }

    fn paragraph_len(&self, bi: usize) -> usize {
        self.edit
            .as_ref()
            .and_then(|e| e.session.host.blocks.get(bi))
            .map(paragraph_text)
            .map(|t| t.len())
            .unwrap_or(0)
    }

    /// The caret's paragraph (block index, clamped byte offset, plain text). Re-finds the block by
    /// id after undo/redo or structure edits, and falls back to the first paragraph.
    fn caret_paragraph(&mut self) -> Option<(usize, usize, String)> {
        let edit = self.edit.as_mut()?;
        let blocks = &edit.session.host.blocks;
        let c = &edit.print.caret;
        let by_id = (!c.block_id.is_empty())
            .then(|| {
                blocks
                    .iter()
                    .position(|b| b.get("id").and_then(|v| v.as_str()) == Some(c.block_id.as_str()))
            })
            .flatten();
        let is_para = |i: usize| {
            blocks
                .get(i)
                .and_then(|b| b.get("type"))
                .and_then(|t| t.as_str())
                == Some("paragraph")
        };
        let bi = by_id
            .filter(|&i| is_para(i))
            .or_else(|| is_para(c.block_index).then_some(c.block_index))
            .or_else(|| (0..blocks.len()).find(|&i| is_para(i)))?;
        let text = paragraph_text(&blocks[bi]);
        let mut off = c.offset.min(text.len());
        while !text.is_char_boundary(off) {
            off -= 1;
        }
        Some((bi, off, text))
    }

    fn set_caret(&mut self, bi: usize, off: usize) {
        if let Some(edit) = self.edit.as_mut() {
            edit.set_caret_block(bi, off);
        }
    }

    /// `[block id, chars before the caret, chars in the paragraph]` for the loader's caret overlay.
    pub fn caret_json(&mut self) -> Option<serde_json::Value> {
        let (bi, off, text) = self.caret_paragraph()?;
        Some(serde_json::json!([
            self.block_id(bi),
            text[..off].chars().count(),
            text.chars().count()
        ]))
    }

    /// Place the caret from a pointer hit: `shown_before` rendered chars precede the click inside
    /// the block `block_id`, whose rendered text has `shown_total` chars. Rendered text can carry
    /// a list label ahead of the model text, which the length difference accounts for.
    pub fn set_caret_from_view(
        &mut self,
        block_id: &str,
        shown_before: usize,
        shown_total: usize,
    ) -> bool {
        let Some(edit) = self.edit.as_ref() else {
            return false;
        };
        let Some(bi) = edit.session.host.blocks.iter().position(|b| {
            b.get("id").and_then(|v| v.as_str()) == Some(block_id)
                && b.get("type").and_then(|t| t.as_str()) == Some("paragraph")
        }) else {
            return false;
        };
        let text = paragraph_text(&edit.session.host.blocks[bi]);
        let n = text.chars().count();
        let chars = shown_before
            .saturating_sub(shown_total.saturating_sub(n))
            .min(n);
        let off = text
            .char_indices()
            .nth(chars)
            .map(|(i, _)| i)
            .unwrap_or(text.len());
        self.typing = None;
        self.set_caret(bi, off);
        true
    }

    /// Re-layout the painted document from the bytes a save would produce.
    pub fn refresh_view(&mut self, original: &[u8]) -> Result<(), String> {
        let bytes = self.save_bytes(original)?;
        let active = self.active;
        let zoom = self.zoom;
        let edit = self.edit.take();
        let typing = self.typing.take();
        let clip = self.text_clip.clone();
        let opened = Self::open(&bytes);
        let mut next = match opened {
            Ok(state) => state,
            Err(e) => {
                self.edit = edit;
                self.typing = typing;
                return Err(e);
            }
        };
        next.edit = edit;
        next.typing = typing;
        next.text_clip = clip;
        next.zoom = zoom;
        let n = next.page_count();
        if n > 0 {
            next.active = active.min(n - 1);
        }
        *self = next;
        Ok(())
    }

    pub fn ribbon_html(&self) -> String {
        let Some(edit) = &self.edit else {
            return String::new();
        };
        command_ribbon(
            "Document",
            &[
                RibbonItem {
                    id: "file.save",
                    label: "Save",
                    enabled: true,
                },
                RibbonItem {
                    id: "edit.undo",
                    label: "Undo",
                    enabled: edit.session.can_undo(),
                },
                RibbonItem {
                    id: "edit.redo",
                    label: "Redo",
                    enabled: edit.session.can_redo(),
                },
                RibbonItem {
                    id: "home.cut",
                    label: "Cut",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.copy",
                    label: "Copy",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.paste",
                    label: "Paste",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.bold",
                    label: "Bold",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.italic",
                    label: "Italic",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.underline",
                    label: "Underline",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.double-underline",
                    label: "Double Underline",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.caps",
                    label: "All Caps",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.hidden",
                    label: "Hidden",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.expand-spacing",
                    label: "Expand",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.condense-spacing",
                    label: "Condense",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.scale-up",
                    label: "Scale Up",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.scale-down",
                    label: "Scale Down",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.strikethrough",
                    label: "Strikethrough",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.highlight",
                    label: "Highlight",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.subscript",
                    label: "Subscript",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.superscript",
                    label: "Superscript",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.font-name",
                    label: "Font",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.indent",
                    label: "Increase Indent",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.outdent",
                    label: "Decrease Indent",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.indent-right",
                    label: "Increase Right Indent",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.outdent-right",
                    label: "Decrease Right Indent",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.line-spacing",
                    label: "Line Spacing",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.shading",
                    label: "Shade",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.border-bottom",
                    label: "Bottom Border",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.page-break-before",
                    label: "Page Break Before",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.space-before",
                    label: "Space Before",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.space-after",
                    label: "Space After",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.hanging-indent",
                    label: "Hanging",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.first-line",
                    label: "First Line",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.bullets",
                    label: "Bullets",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.align-left",
                    label: "Align Left",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.align-center",
                    label: "Center",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.align-right",
                    label: "Align Right",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.align-justify",
                    label: "Justify",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.numbering",
                    label: "Numbering",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.style-normal",
                    label: "Normal",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.style-title",
                    label: "Title",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.style-heading-1",
                    label: "Heading 1",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.style-heading-2",
                    label: "Heading 2",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.style-heading-3",
                    label: "Heading 3",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.font-size",
                    label: "Font Size",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.grow-font",
                    label: "Grow Font",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.shrink-font",
                    label: "Shrink Font",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.font-color",
                    label: "Font Color",
                    enabled: true,
                },
                RibbonItem {
                    id: "table.insert-row",
                    label: "Insert Row",
                    enabled: true,
                },
                RibbonItem {
                    id: "table.delete-row",
                    label: "Delete Row",
                    enabled: true,
                },
                RibbonItem {
                    id: "table.insert-column",
                    label: "Insert Column",
                    enabled: true,
                },
                RibbonItem {
                    id: "table.delete-column",
                    label: "Delete Column",
                    enabled: true,
                },
                RibbonItem {
                    id: "table.cell-text",
                    label: "Cell Text",
                    enabled: true,
                },
                RibbonItem {
                    id: "insert.table",
                    label: "Table",
                    enabled: true,
                },
                RibbonItem {
                    id: "insert.picture",
                    label: "Picture",
                    enabled: true,
                },
                RibbonItem {
                    id: "insert.page-break",
                    label: "Page Break",
                    enabled: true,
                },
                RibbonItem {
                    id: "table.delete",
                    label: "Delete Table",
                    enabled: true,
                },
            ],
        )
    }
}

/// Chrome and shortcut ids (`home.bold`) share the xlsx table. The docx editor
/// registers the short form (`bold`).
fn canonical_command(id: &str) -> &str {
    match id {
        "home.cut" => "cut",
        "home.copy" => "copy",
        "home.paste" => "paste",
        "home.bold" => "bold",
        "home.italic" => "italic",
        "home.underline" => "underline",
        "home.double-underline" => "double-underline",
        "home.caps" => "caps",
        "home.hidden" => "hidden",
        "home.expand-spacing" => "expand-spacing",
        "home.condense-spacing" => "condense-spacing",
        "home.scale-up" => "scale-up",
        "home.scale-down" => "scale-down",
        "home.strikethrough" => "strike",
        "home.highlight" => "highlight",
        "home.indent" => "indent",
        "home.outdent" => "outdent",
        "home.indent-right" => "indent-right",
        "home.outdent-right" => "outdent-right",
        "home.line-spacing" => "line-spacing",
        "home.shading" => "shading",
        "home.border-bottom" => "border-bottom",
        "home.page-break-before" => "page-break-before",
        "home.space-before" => "space-before",
        "home.space-after" => "space-after",
        "home.hanging-indent" => "hanging-indent",
        "home.first-line" => "first-line",
        "home.subscript" => "subscript",
        "home.superscript" => "superscript",
        "home.font-name" => "font-name",
        "home.bullets" => "bullets",
        "home.align-left" => "align-left",
        "home.align-center" => "align-center",
        "home.align-right" => "align-right",
        "home.align-justify" => "align-justify",
        "home.numbering" => "numbering",
        "home.style-normal" => "style-normal",
        "home.style-title" => "style-title",
        "home.style-heading-1" => "style-heading-1",
        "home.style-heading-2" => "style-heading-2",
        "home.style-heading-3" => "style-heading-3",
        "insert.page-break" => "insert.page-break",
        "home.font-size" => "font-size",
        "home.grow-font" => "grow-font",
        "home.shrink-font" => "shrink-font",
        "home.font-color" => "font-color",
        other => other,
    }
}
