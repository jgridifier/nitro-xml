// SPDX-License-Identifier: MIT
//! PowerPoint editor UI: ribbon, shortcuts, selection chrome (#88).

mod commands;
mod ribbon;
mod selection;
mod shortcuts;

pub use commands::{CommandMeta, PptxCommandSurface, PPTX_EDITOR_COMMANDS};
pub use ribbon::{ribbon_html, slide_nav_html};
pub use selection::SelectionState;
pub use shortcuts::{find_shortcut, matches_keys, KeyEvent, Shortcut, ShortcutMatch, SHORTCUTS};

use nitro_xml_opc::edit::{EditError, EditHistory, EditSession};
use serde_json::Value;

use crate::edit::host::PptxHost;
use crate::edit::shapes::register_shape_commands;

/// Editable PowerPoint shell: edit session + chrome command surface.
pub struct PptxEditor {
    pub session: EditSession<PptxHost, EditHistory>,
    pub commands: PptxCommandSurface,
    pub selection: SelectionState,
    active_tab: String,
    read_only: bool,
}

impl PptxEditor {
    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        let host = PptxHost::open(bytes)?;
        Ok(Self {
            session: EditSession::new(host, EditHistory::new()),
            commands: PptxCommandSurface::new(false),
            selection: SelectionState::new(0),
            active_tab: "home".to_string(),
            read_only: false,
        })
    }

    /// Build an editor around an existing host (tests / in-memory fixtures).
    pub fn from_host(host: PptxHost) -> Self {
        Self {
            session: EditSession::new(host, EditHistory::new()),
            commands: PptxCommandSurface::new(false),
            selection: SelectionState::new(0),
            active_tab: "home".to_string(),
            read_only: false,
        }
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        self.commands.set_read_only(read_only);
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }

    pub fn active_tab(&self) -> &str {
        &self.active_tab
    }

    pub fn set_active_tab(&mut self, tab: &str) {
        self.active_tab = tab.to_string();
    }

    pub fn slide_count(&self) -> usize {
        self.session.host.slides.len()
    }

    pub fn set_slide(&mut self, index: u32) -> Result<(), EditError> {
        if index as usize >= self.session.host.slides.len() {
            return Err(EditError::Other {
                message: format!("slide {index} missing"),
            });
        }
        self.selection.set_slide(index);
        Ok(())
    }

    pub fn select_elements(&mut self, ids: impl IntoIterator<Item = String>) {
        self.selection.select(ids);
    }

    pub fn ribbon_html(&self) -> String {
        format!(
            "{}{}",
            ribbon::ribbon_html(&self.commands, &self.selection, &self.active_tab),
            ribbon::slide_nav_html(self.slide_count(), &self.selection),
        )
    }

    pub fn dispatch_command(&mut self, id: &str, args: &Value) -> Result<(), EditError> {
        self.commands
            .dispatch(id, args, &mut self.selection, &mut self.session)
    }

    pub fn match_shortcut(&self, event: &KeyEvent, mac: bool) -> Option<ShortcutMatch> {
        if self.read_only && !shortcut_allowed_readonly(event, mac) {
            return None;
        }
        find_shortcut(event, mac)
    }

    pub fn handle_shortcut(&mut self, event: &KeyEvent, mac: bool) -> bool {
        let Some(m) = self.match_shortcut(event, mac) else {
            return false;
        };
        self.dispatch_command(m.command, &Value::Null).is_ok()
    }

    pub fn can_undo(&self) -> bool {
        self.session.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.session.can_redo()
    }

    /// Registry used for headless script replay from the same shape command set.
    pub fn shape_registry() -> nitro_xml_opc::edit::CommandRegistry {
        let mut registry = nitro_xml_opc::edit::CommandRegistry::new();
        register_shape_commands(&mut registry);
        registry
    }

    // TODO(#62): crash recovery snapshot / restore hooks for the editor shell.
    // TODO(#52): arm pptx edit-path perf gate once keystroke path is wired end-to-end.
}

fn shortcut_allowed_readonly(event: &KeyEvent, mac: bool) -> bool {
    let Some(m) = find_shortcut(event, mac) else {
        return false;
    };
    matches!(m.command, "home.select-all")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::host::{EditableElement, EditableSlide};
    use serde_json::json;

    fn stub_editor() -> PptxEditor {
        let host = PptxHost {
            slides: vec![EditableSlide {
                path: "ppt/slides/slide1.xml".into(),
                elements: vec![{
                    let mut el = EditableElement::blank("el-a", "shape");
                    el.shape_type = Some("rect".into());
                    el.set_box(10.0, 20.0, 100.0, 50.0, 0.0);
                    el
                }],
            }],
        };
        PptxEditor::from_host(host)
    }

    #[test]
    fn catches_mod_z_undo_after_delete() {
        // Catches: Mod+Z not undoing a ribbon/API delete in the pptx editor (#88).
        let mut editor = stub_editor();
        editor.select_elements(vec!["el-a".into()]);
        editor
            .dispatch_command("home.delete", &Value::Null)
            .expect("delete");
        assert!(editor.session.host.slides[0].elements.is_empty());
        let ev = KeyEvent {
            key: "z".into(),
            code: Some("KeyZ".into()),
            ctrl: true,
            meta: false,
            shift: false,
            alt: false,
        };
        assert!(editor.handle_shortcut(&ev, false));
        assert_eq!(editor.session.host.slides[0].elements.len(), 1);
        assert_eq!(editor.session.host.slides[0].elements[0].id, "el-a");
    }

    #[test]
    fn catches_insert_shape_ribbon_command() {
        // Catches: insert.shape not adding a shape through the editor API (#88).
        let mut editor = stub_editor();
        editor.set_active_tab("insert");
        editor
            .dispatch_command(
                "insert.shape",
                &json!({ "id": "new-1", "shapeType": "ellipse" }),
            )
            .expect("insert");
        assert_eq!(editor.session.host.slides[0].elements.len(), 2);
        assert!(editor
            .ribbon_html()
            .contains("data-command=\"insert.shape\""));
        assert_eq!(editor.selection.primary_id(), Some("new-1"));
    }
}
