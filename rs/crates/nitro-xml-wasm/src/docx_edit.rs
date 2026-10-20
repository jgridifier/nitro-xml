// SPDX-License-Identifier: MIT
//! Wasm glue for the docx editor session: open, chrome commands, headless edits, save.

use nitro_xml_docx::{save_docx, DocxEditor};
use serde_json::Value;

use crate::docx::DocxState;
use crate::office_edit::{command_ribbon, edit_plain, is_text_key, OfficeEffect, RibbonItem};

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
        let id = canonical_command(id);
        if id == "file.save" {
            return Ok(OfficeEffect::Save);
        }
        let edit = self
            .edit
            .as_mut()
            .ok_or_else(|| "no edit session".to_string())?;
        edit.dispatch_command(id, &Value::Null)
            .map_err(|e| e.to_string())?;
        Ok(OfficeEffect::Applied)
    }

    pub fn apply_raw_edit(&mut self, spec: &str) -> Result<OfficeEffect, String> {
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

    /// Printable keys and Backspace/Delete edit the caret paragraph.
    pub fn type_key(
        &mut self,
        key: &str,
        ctrl: bool,
        meta: bool,
        alt: bool,
    ) -> Result<bool, String> {
        if ctrl || meta || alt || !is_text_key(key) {
            return Ok(false);
        }
        let (block, text, offset, attrs) = {
            let edit = self
                .edit
                .as_ref()
                .ok_or_else(|| "no edit session".to_string())?;
            let caret = edit.print.caret.block_index;
            let blocks = &edit.session.host.blocks;
            let block = if blocks
                .get(caret)
                .and_then(|b| b.get("type"))
                .and_then(|t| t.as_str())
                == Some("paragraph")
            {
                caret
            } else {
                blocks
                    .iter()
                    .position(|b| b.get("type").and_then(|t| t.as_str()) == Some("paragraph"))
                    .ok_or_else(|| "no paragraph".to_string())?
            };
            let runs = blocks[block].get("runs").and_then(|r| r.as_array());
            let text: String = runs
                .map(|rs| {
                    rs.iter()
                        .map(|r| r.get("text").and_then(|t| t.as_str()).unwrap_or(""))
                        .collect()
                })
                .unwrap_or_default();
            let offset = if block == caret {
                edit.print.caret.offset
            } else {
                0
            };
            let mut attrs = runs
                .and_then(|rs| rs.first())
                .and_then(|r| r.as_object())
                .cloned()
                .unwrap_or_default();
            attrs.remove("text");
            (block, text, offset, attrs)
        };
        let Some(next) = edit_plain(&text, offset, key) else {
            return Ok(false);
        };
        let mut spec = serde_json::json!({
            "command": "setParagraphText",
            "blockIndex": block,
            "text": next,
        });
        if let Some(obj) = spec.as_object_mut() {
            for (k, v) in attrs {
                obj.entry(k).or_insert(v);
            }
        }
        self.apply_raw_edit(&spec.to_string())?;
        if let Some(edit) = self.edit.as_mut() {
            edit.print.caret.block_index = block;
            edit.print.caret.offset = match key {
                "Backspace" => {
                    let mut end = offset.min(text.len());
                    while end > 0 && !text.is_char_boundary(end) {
                        end -= 1;
                    }
                    let mut prev = end;
                    if prev > 0 {
                        prev -= 1;
                        while prev > 0 && !text.is_char_boundary(prev) {
                            prev -= 1;
                        }
                    }
                    prev
                }
                "Delete" => offset.min(next.len()),
                _ => offset.min(text.len()) + key.len(),
            };
        }
        Ok(true)
    }

    /// Re-layout the painted document from the bytes a save would produce.
    pub fn refresh_view(&mut self, original: &[u8]) -> Result<(), String> {
        let bytes = self.save_bytes(original)?;
        let active = self.active;
        let zoom = self.zoom;
        let edit = self.edit.take();
        let opened = Self::open(&bytes);
        let mut next = match opened {
            Ok(state) => state,
            Err(e) => {
                self.edit = edit;
                return Err(e);
            }
        };
        next.edit = edit;
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
                    id: "home.font-size",
                    label: "Font Size",
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
                    id: "insert.table",
                    label: "Table",
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
        "home.bold" => "bold",
        "home.italic" => "italic",
        "home.underline" => "underline",
        "home.align-left" => "align-left",
        "home.align-center" => "align-center",
        "home.align-right" => "align-right",
        "home.font-size" => "font-size",
        "home.font-color" => "font-color",
        other => other,
    }
}
