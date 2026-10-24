// SPDX-License-Identifier: MIT
//! Wasm glue for the pptx editor session: open, chrome commands, headless edits, save.

use nitro_xml_pptx::edit::host::PptxHost;
use nitro_xml_pptx::editor::PptxEditor;
use nitro_xml_pptx::save_pptx;
use serde_json::Value;

use crate::office_edit::{command_ribbon, edit_plain, is_text_key, OfficeEffect, RibbonItem};
use crate::pptx::PptxState;

const SHAPE_COMMANDS: &[&str] = &[
    "home.bold",
    "home.italic",
    "home.underline",
    "home.strikethrough",
    "home.double-strike",
    "home.small-caps",
    "home.caps",
    "home.align-left",
    "home.align-center",
    "home.align-right",
    "home.align-justify",
    "home.bullets",
    "home.numbering",
    "home.font-size",
    "home.grow-font",
    "home.shrink-font",
    "home.font-color",
    "home.indent",
    "home.outdent",
    "home.line-spacing",
    "home.subscript",
    "home.superscript",
    "home.font-name",
    "home.shape-fill",
    "home.shape-outline",
    "home.shape-dash",
    "home.duplicate",
    "home.delete",
    "home.bring-forward",
    "home.send-backward",
    "home.bring-to-front",
    "home.send-to-back",
    "home.group",
    "home.ungroup",
];

impl PptxState {
    pub fn ensure_edit(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.edit.is_some() {
            self.sync_editor_selection();
            return Ok(());
        }
        // Reuse the viewer `Presentation` (no second `Presentation::open`). When slides are
        // not all parsed yet, `from_presentation` finishes lazy loads on this deck only.
        let host = PptxHost::from_presentation(&mut self.pres)?;
        debug_assert_eq!(bytes, self.package_arc().as_ref());
        let mut edit =
            PptxEditor::from_opened_host(host, std::sync::Arc::clone(self.package_arc()));
        edit.set_read_only(false);
        let slide = self.active as u32;
        let _ = edit.set_slide(slide);
        self.edit = Some(edit);
        self.sync_editor_selection();
        Ok(())
    }

    /// Point the editor at the viewer's active slide and selection.
    /// A shape command with nothing selected targets the first shape on that slide
    /// so Bold/Italic from the ribbon changes saved bytes without a prior click.
    pub fn sync_editor_selection(&mut self) {
        let slide = self.active as u32;
        let selected = self.selected_ids.clone();
        let Some(edit) = self.edit.as_mut() else {
            return;
        };
        let _ = edit.set_slide(slide);
        if !selected.is_empty() {
            edit.select_elements(selected);
        }
    }

    fn ensure_shape_target(&mut self) {
        self.sync_editor_selection();
        let Some(edit) = self.edit.as_mut() else {
            return;
        };
        if edit.selection.primary_id().is_some() {
            return;
        }
        let slide = edit.selection.slide_index as usize;
        let id = edit
            .session
            .host
            .slides
            .get(slide)
            .and_then(|s| s.elements.first())
            .map(|el| el.id.clone());
        if let Some(id) = id {
            edit.select_elements(std::iter::once(id));
        }
    }

    fn pull_editor_selection(&mut self) {
        let Some(edit) = self.edit.as_ref() else {
            return;
        };
        self.selected_ids = edit.selection.element_ids.clone();
        let slide = edit.selection.slide_index as usize;
        if slide < self.pres.slide_count() {
            self.active = slide;
        }
    }

    pub fn save_bytes(&self, original: &[u8]) -> Result<Vec<u8>, String> {
        match &self.edit {
            // Rewrite from the package opened on the handle. The editor's own
            // `save()` swaps that baseline and would make a later undo save the
            // edited bytes again.
            Some(edit) => save_pptx(&edit.session.host, original),
            None => Ok(original.to_vec()),
        }
    }

    pub fn dispatch_chrome(&mut self, id: &str) -> Result<OfficeEffect, String> {
        self.ensure_shape_if_needed(id);
        if id == "file.save" {
            return Ok(OfficeEffect::Save);
        }
        if matches!(id, "home.cut" | "home.copy" | "home.paste") {
            return self.clipboard_command(id);
        }
        let edit = self
            .edit
            .as_mut()
            .ok_or_else(|| "no edit session".to_string())?;
        edit.dispatch_command(id, &Value::Null)
            .map_err(|e| e.to_string())?;
        self.pull_editor_selection();
        Ok(OfficeEffect::Applied)
    }

    /// Copy, cut, or paste the selected shape's plain text. Copy does not touch the package.
    fn clipboard_command(&mut self, id: &str) -> Result<OfficeEffect, String> {
        self.ensure_shape_target();
        let (slide, element_id, text) = self.shape_plain_target()?;
        match id {
            "home.copy" => {
                self.remember_shape();
                self.text_clip = text.clone();
                Ok(OfficeEffect::Clipboard {
                    text,
                    changed: false,
                })
            }
            "home.cut" => {
                self.text_clip = text.clone();
                if text.is_empty() {
                    return Ok(OfficeEffect::Clipboard {
                        text,
                        changed: false,
                    });
                }
                self.set_shape_text(slide, &element_id, "")?;
                Ok(OfficeEffect::Clipboard {
                    text: self.text_clip.clone(),
                    changed: true,
                })
            }
            "home.paste" => {
                if let Some(snapshot) = self.shape_clip.clone() {
                    let edit = self
                        .edit
                        .as_mut()
                        .ok_or_else(|| "no edit session".to_string())?;
                    edit.paste_clipboard_shape(&snapshot)
                        .map_err(|e| e.to_string())?;
                    return Ok(OfficeEffect::Applied);
                }
                let clip = self.text_clip.clone();
                if clip.is_empty() {
                    return Ok(OfficeEffect::Clipboard {
                        text: String::new(),
                        changed: false,
                    });
                }
                let mut next = text;
                next.push_str(&clip);
                self.set_shape_text(slide, &element_id, &next)?;
                Ok(OfficeEffect::Applied)
            }
            _ => Err(format!("unknown clipboard command {id}")),
        }
    }

    fn shape_plain_target(&self) -> Result<(u32, String, String), String> {
        let edit = self
            .edit
            .as_ref()
            .ok_or_else(|| "no edit session".to_string())?;
        let slide = edit.selection.slide_index;
        let id = edit
            .selection
            .primary_id()
            .ok_or_else(|| "no shape".to_string())?
            .to_owned();
        let text = edit
            .session
            .host
            .slide(slide)
            .ok()
            .and_then(|s| s.elements.iter().find(|e| e.id == id))
            .map(shape_plain)
            .unwrap_or_default();
        Ok((slide, id, text))
    }

    fn remember_shape(&mut self) {
        let snap = self.edit.as_ref().and_then(|edit| {
            let id = edit.selection.primary_id()?.to_owned();
            let slide = edit.selection.slide_index;
            edit.session
                .host
                .slide(slide)
                .ok()?
                .elements
                .iter()
                .find(|el| el.id == id)
                .cloned()
        });
        if snap.is_some() {
            self.shape_clip = snap;
        }
    }

    fn set_shape_text(&mut self, slide: u32, id: &str, text: &str) -> Result<(), String> {
        let spec = serde_json::json!({
            "command": "setShapeText",
            "slideIndex": slide,
            "elementId": id,
            "text": text,
        });
        self.apply_raw_edit(&spec.to_string())?;
        Ok(())
    }

    pub fn apply_raw_edit(&mut self, spec: &str) -> Result<OfficeEffect, String> {
        let v: Value = serde_json::from_str(spec).map_err(|e| e.to_string())?;
        let command = v
            .get("command")
            .and_then(|c| c.as_str())
            .ok_or_else(|| "missing command".to_string())?
            .to_owned();
        if command == "file.save" {
            return Ok(OfficeEffect::Save);
        }
        self.ensure_shape_if_needed(&command);
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
        self.pull_editor_selection();
        Ok(OfficeEffect::Applied)
    }

    fn ensure_shape_if_needed(&mut self, id: &str) {
        if SHAPE_COMMANDS.contains(&id) {
            self.ensure_shape_target();
        } else {
            self.sync_editor_selection();
        }
    }

    /// Re-parse the painted deck from the bytes a save would produce, keeping the edit session.
    pub fn refresh_view(&mut self, original: &[u8]) -> Result<(), String> {
        let bytes = self.save_bytes(original)?;
        let active = self.active;
        let selected = self.selected_ids.clone();
        let vw = self.viewport_w;
        let vh = self.viewport_h;
        let scale = self.scale;
        let edit = self.edit.take();
        let clip = self.text_clip.clone();
        let shape_clip = self.shape_clip.clone();
        let opened = Self::open(std::sync::Arc::<[u8]>::from(bytes));
        let mut next = match opened {
            Ok(state) => state,
            Err(e) => {
                self.edit = edit;
                return Err(e);
            }
        };
        next.edit = edit;
        next.text_clip = clip;
        next.shape_clip = shape_clip;
        next.viewport_w = vw;
        next.viewport_h = vh;
        next.scale = scale;
        let n = next.pres.slide_count();
        if n > 0 {
            next.active = active.min(n - 1);
        }
        next.selected_ids = selected;
        *self = next;
        Ok(())
    }

    /// Printable keys and Backspace/Delete edit the selected shape's text.
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
        self.ensure_shape_target();
        let (slide, id, text) = {
            let edit = self
                .edit
                .as_ref()
                .ok_or_else(|| "no edit session".to_string())?;
            let slide = edit.selection.slide_index;
            let id = edit
                .selection
                .primary_id()
                .ok_or_else(|| "no shape to type into".to_string())?
                .to_owned();
            let text = edit
                .session
                .host
                .slide(slide)
                .ok()
                .and_then(|s| s.elements.iter().find(|e| e.id == id))
                .map(shape_plain)
                .unwrap_or_default();
            (slide, id, text)
        };
        let Some(next) = edit_plain(&text, text.len(), key) else {
            return Ok(false);
        };
        let spec = serde_json::json!({
            "command": "setShapeText",
            "slideIndex": slide,
            "elementId": id,
            "text": next,
        });
        self.apply_raw_edit(&spec.to_string())?;
        Ok(true)
    }

    pub fn ribbon_html(&self) -> String {
        let Some(edit) = &self.edit else {
            return String::new();
        };
        command_ribbon(
            "Presentation",
            &[
                RibbonItem {
                    id: "file.save",
                    label: "Save",
                    enabled: true,
                },
                RibbonItem {
                    id: "edit.undo",
                    label: "Undo",
                    enabled: edit.can_undo(),
                },
                RibbonItem {
                    id: "edit.redo",
                    label: "Redo",
                    enabled: edit.can_redo(),
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
                    id: "home.strikethrough",
                    label: "Strikethrough",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.double-strike",
                    label: "Double Strikethrough",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.small-caps",
                    label: "Small Caps",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.caps",
                    label: "All Caps",
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
                    id: "home.bullets",
                    label: "Bullets",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.numbering",
                    label: "Numbering",
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
                    id: "home.font-name",
                    label: "Font",
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
                    id: "home.line-spacing",
                    label: "Line Spacing",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.shape-fill",
                    label: "Fill",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.shape-outline",
                    label: "Outline",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.shape-dash",
                    label: "Dashed Outline",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.layout",
                    label: "Layout",
                    enabled: true,
                },
                RibbonItem {
                    id: "insert.shape",
                    label: "Shape",
                    enabled: true,
                },
                RibbonItem {
                    id: "insert.text-box",
                    label: "Text Box",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.duplicate",
                    label: "Duplicate",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.delete",
                    label: "Delete",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.bring-forward",
                    label: "Bring Forward",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.send-backward",
                    label: "Send Backward",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.bring-to-front",
                    label: "Bring to Front",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.send-to-back",
                    label: "Send to Back",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.group",
                    label: "Group",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.ungroup",
                    label: "Ungroup",
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
                    id: "table.delete",
                    label: "Delete Table",
                    enabled: true,
                },
                RibbonItem {
                    id: "insert.slide",
                    label: "New Slide",
                    enabled: true,
                },
                RibbonItem {
                    id: "insert.duplicate-slide",
                    label: "Duplicate Slide",
                    enabled: true,
                },
                RibbonItem {
                    id: "insert.move-slide-up",
                    label: "Move Slide Up",
                    enabled: true,
                },
                RibbonItem {
                    id: "insert.move-slide-down",
                    label: "Move Slide Down",
                    enabled: true,
                },
                RibbonItem {
                    id: "home.delete-slide",
                    label: "Delete Slide",
                    enabled: true,
                },
            ],
        )
    }
}

fn shape_plain(el: &nitro_xml_pptx::edit::host::EditableElement) -> String {
    if let Some(text) = el.text.as_deref() {
        return text.to_owned();
    }
    el.text_segments.iter().map(|s| s.text.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Arc;

    #[test]
    fn catches_ensure_edit_reuses_viewer_presentation_not_second_open() {
        // Catches: ensure_edit calling PptxHost::open / second Presentation::open while the
        // viewer already holds the package Arc (duplicate zip parse + peak heap).
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/generated/pptx/slides_10.pptx");
        let bytes: Arc<[u8]> = Arc::from(std::fs::read(&path).expect("fixture"));
        let mut state = PptxState::open(Arc::clone(&bytes)).expect("viewer open");
        state
            .pres
            .ensure_all_slides()
            .expect("preload slides for soft-share gate");
        assert!(state.pres.all_slides_loaded());
        let refs_after_open = Arc::strong_count(&bytes);
        state.ensure_edit(bytes.as_ref()).expect("ensure_edit");
        let edit = state.edit.as_ref().expect("edit session");
        assert_eq!(
            edit.slide_count(),
            state.pres.slide_count(),
            "host slide count must match viewer deck"
        );
        assert_eq!(
            Arc::strong_count(&bytes),
            refs_after_open + 1,
            "ensure_edit must retain one more Arc ref on the package, not a duplicate Vec"
        );
    }
}
