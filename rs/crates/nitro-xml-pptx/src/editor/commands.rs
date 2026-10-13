// SPDX-License-Identifier: MIT
//! PowerPoint ribbon / shortcut command ids → edit session (#88).

use std::collections::BTreeMap;

use nitro_xml_opc::edit::{
    ChangeKind, Command, CommandRegistry, EditError, EditHistory, EditHost, EditScope, EditSession,
};
use serde_json::{json, Value};

use crate::edit::host::PptxHost;
use crate::edit::shapes::register_shape_commands;
use crate::editor::selection::SelectionState;

#[derive(Debug, Clone)]
pub struct CommandMeta {
    pub id: &'static str,
    pub label: &'static str,
    pub shortcut: Option<&'static str>,
    pub editing: bool,
}

macro_rules! pptx_cmd {
    ($id:expr, $label:expr, $shortcut:expr, $editing:expr) => {
        CommandMeta {
            id: $id,
            label: $label,
            shortcut: $shortcut,
            editing: $editing,
        }
    };
}

/// Stable chrome ids for the pptx editor shell (ribbon + shortcuts + API).
pub const PPTX_EDITOR_COMMANDS: &[CommandMeta] = &[
    pptx_cmd!("file.save", "Save", Some("Ctrl+S"), true),
    pptx_cmd!("edit.undo", "Undo", Some("Ctrl+Z"), true),
    pptx_cmd!("edit.redo", "Redo", Some("Ctrl+Y"), true),
    pptx_cmd!("home.bold", "Bold", Some("Ctrl+B"), true),
    pptx_cmd!("home.italic", "Italic", Some("Ctrl+I"), true),
    pptx_cmd!("home.duplicate", "Duplicate", Some("Ctrl+D"), true),
    pptx_cmd!("home.delete", "Delete", Some("Delete"), true),
    pptx_cmd!("home.select-all", "Select All", Some("Ctrl+A"), false),
    pptx_cmd!("home.bring-forward", "Bring Forward", None, true),
    pptx_cmd!("home.send-backward", "Send Backward", None, true),
    pptx_cmd!("home.bring-to-front", "Bring to Front", None, true),
    pptx_cmd!("home.send-to-back", "Send to Back", None, true),
    pptx_cmd!("home.group", "Group", Some("Ctrl+G"), true),
    pptx_cmd!("home.ungroup", "Ungroup", Some("Ctrl+Shift+G"), true),
    pptx_cmd!("insert.shape", "Shape", None, true),
    pptx_cmd!("insert.text-box", "Text Box", None, true),
    pptx_cmd!("insert.table", "Table", None, true),
    pptx_cmd!("insert.slide", "New Slide", None, true),
    pptx_cmd!("home.delete-slide", "Delete Slide", None, true),
];

/// Chrome + shape registry for one pptx editor session.
pub struct PptxCommandSurface {
    pub registry: CommandRegistry,
    meta: BTreeMap<String, CommandMeta>,
    pub last_command: Option<String>,
}

impl PptxCommandSurface {
    pub fn new(read_only: bool) -> Self {
        let mut registry = CommandRegistry::new();
        register_shape_commands(&mut registry);
        register_editor_commands(&mut registry);
        // TODO(#85): register_text_commands / register_table_commands when merged.
        // TODO(#86): slide/notes insert-delete commands when merged.
        let mut meta = BTreeMap::new();
        for m in PPTX_EDITOR_COMMANDS {
            meta.insert(m.id.to_string(), m.clone());
        }
        let mut surface = Self {
            registry,
            meta,
            last_command: None,
        };
        if read_only {
            surface.set_read_only(true);
        }
        surface
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.registry.set_read_only(read_only);
    }

    pub fn label(&self, id: &str) -> Option<&str> {
        self.meta.get(id).map(|m| m.label)
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        // Selection-gated drawing commands stay enabled in the registry; the
        // editor shell decides based on selection when dispatching.
        self.registry.is_enabled(id)
    }

    pub fn dispatch(
        &mut self,
        id: &str,
        args: &Value,
        selection: &mut SelectionState,
        session: &mut EditSession<PptxHost, EditHistory>,
    ) -> Result<(), EditError> {
        if !self.registry.is_enabled(id) {
            return Err(EditError::unsupported(id));
        }
        self.last_command = Some(id.to_string());

        match id {
            // Handled by [`crate::editor::PptxEditor::dispatch_command`] via `save_pptx`.
            "file.save" => Err(EditError::Other {
                message: "file.save must go through PptxEditor::save".into(),
            }),
            "edit.undo" => {
                session.undo();
                Ok(())
            }
            "edit.redo" => {
                session.redo();
                Ok(())
            }
            "home.select-all" => {
                let slide = selection.slide_index;
                let ids: Vec<String> = session
                    .host
                    .slide(slide)
                    .map(|s| s.elements.iter().map(|e| e.id.clone()).collect())
                    .unwrap_or_default();
                selection.select(ids);
                Ok(())
            }
            "home.bold" | "home.italic" => {
                let key = if id == "home.bold" { "bold" } else { "italic" };
                let element_id = selection.primary_id().ok_or_else(|| EditError::Other {
                    message: format!("{id} needs a selected shape"),
                })?;
                let slide = selection.slide_index;
                let current = session
                    .host
                    .slide(slide)
                    .ok()
                    .and_then(|s| s.elements.iter().find(|e| e.id == element_id))
                    .and_then(|e| e.text_style.as_ref())
                    .map(|ts| if key == "bold" { ts.bold } else { ts.italic })
                    .unwrap_or(false);
                let mut patch = json!({
                    "slideIndex": slide,
                    "elementId": element_id,
                });
                patch[key] = json!(!current);
                self.run_shape_op("setShapeTextStyle", &patch, session)
            }
            "home.duplicate" => {
                let element_id = selection.primary_id().ok_or_else(|| EditError::Other {
                    message: "home.duplicate needs a selected shape".into(),
                })?;
                let args = json!({
                    "slideIndex": selection.slide_index,
                    "elementId": element_id,
                });
                self.run_shape_op("duplicateShape", &args, session)
            }
            "home.delete" => {
                let element_id = selection.primary_id().ok_or_else(|| EditError::Other {
                    message: "home.delete needs a selected shape".into(),
                })?;
                let args = json!({
                    "slideIndex": selection.slide_index,
                    "elementId": element_id,
                });
                let result = self.run_shape_op("deleteShape", &args, session);
                if result.is_ok() {
                    selection.clear();
                }
                result
            }
            "home.bring-forward" => self.reorder("bringForward", selection, session),
            "home.send-backward" => self.reorder("sendBackward", selection, session),
            "home.bring-to-front" => self.reorder("bringToFront", selection, session),
            "home.send-to-back" => self.reorder("sendToBack", selection, session),
            "home.group" => {
                if selection.element_ids.len() < 2 {
                    return Err(EditError::Other {
                        message: "home.group needs at least two selected shapes".into(),
                    });
                }
                let group_id = args
                    .get("groupId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("group-editor");
                let op_args = json!({
                    "slideIndex": selection.slide_index,
                    "elementIds": selection.element_ids,
                    "groupId": group_id,
                });
                self.run_shape_op("groupShapes", &op_args, session)
            }
            "home.ungroup" => {
                let group_id = selection.primary_id().ok_or_else(|| EditError::Other {
                    message: "home.ungroup needs a selected group".into(),
                })?;
                let op_args = json!({
                    "slideIndex": selection.slide_index,
                    "groupId": group_id,
                });
                self.run_shape_op("ungroupShapes", &op_args, session)
            }
            "insert.shape" => {
                let shape_type = args
                    .get("shapeType")
                    .and_then(|v| v.as_str())
                    .unwrap_or("rect");
                let x = args.get("x").and_then(|v| v.as_f64()).unwrap_or(100.0);
                let y = args.get("y").and_then(|v| v.as_f64()).unwrap_or(100.0);
                let width = args.get("width").and_then(|v| v.as_f64()).unwrap_or(200.0);
                let height = args.get("height").and_then(|v| v.as_f64()).unwrap_or(120.0);
                let id = args
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("shape-editor");
                let op_args = json!({
                    "slideIndex": selection.slide_index,
                    "shapeType": shape_type,
                    "x": x,
                    "y": y,
                    "width": width,
                    "height": height,
                    "id": id,
                });
                let result = self.run_shape_op("addShape", &op_args, session);
                if result.is_ok() {
                    selection.select(vec![id.to_string()]);
                }
                result
            }
            // TODO(#85): text box / table insert via text+table edit modules.
            "insert.text-box" | "insert.table" => Err(EditError::unsupported(id)),
            // TODO(#86): slide insert/delete when slides/notes edits land.
            "insert.slide" | "home.delete-slide" => Err(EditError::unsupported(id)),
            // Direct API / script op ids (addShape, deleteShape, …).
            _ => {
                let merged = merge_selection_args(id, args, selection);
                self.run_shape_op(id, &merged, session)
            }
        }
    }

    fn reorder(
        &self,
        op: &str,
        selection: &SelectionState,
        session: &mut EditSession<PptxHost, EditHistory>,
    ) -> Result<(), EditError> {
        let element_id = selection.primary_id().ok_or_else(|| EditError::Other {
            message: format!("{op} needs a selected shape"),
        })?;
        let args = json!({
            "slideIndex": selection.slide_index,
            "elementId": element_id,
        });
        self.run_shape_op(op, &args, session)
    }

    fn run_shape_op(
        &self,
        id: &str,
        args: &Value,
        session: &mut EditSession<PptxHost, EditHistory>,
    ) -> Result<(), EditError> {
        if !self.registry.is_enabled(id) {
            return Err(EditError::unsupported(id));
        }
        let kind = kind_for_op(id);
        let scopes = vec![EditScope::Workbook];
        let registry = &self.registry;
        session
            .run(id, kind, scopes, |host| {
                registry.run(id, host.as_any(), args).map(|_| ())
            })
            .map(|_| ())
    }
}

fn kind_for_op(id: &str) -> ChangeKind {
    match id {
        "setShapeFill" | "setShapeLine" | "setShapeTextStyle" => ChangeKind::Format,
        "addShape" | "deleteShape" | "duplicateShape" | "bringForward" | "sendBackward"
        | "bringToFront" | "sendToBack" | "groupShapes" | "ungroupShapes"
        | "patchShapeGeometry" => ChangeKind::Structure,
        _ => ChangeKind::Other,
    }
}

/// Fill slideIndex / elementId from selection when the API caller omitted them.
fn merge_selection_args(id: &str, args: &Value, selection: &SelectionState) -> Value {
    let mut map = match args {
        Value::Object(m) => m.clone(),
        _ => serde_json::Map::new(),
    };
    if !map.contains_key("slideIndex") {
        map.insert("slideIndex".into(), json!(selection.slide_index));
    }
    if !map.contains_key("elementId") {
        if let Some(eid) = selection.primary_id() {
            // Ops that create shapes don't need a selection id.
            if !matches!(id, "addShape") {
                map.insert("elementId".into(), json!(eid));
            }
        }
    }
    Value::Object(map)
}

pub fn register_editor_commands(registry: &mut CommandRegistry) {
    for m in PPTX_EDITOR_COMMANDS {
        let mut cmd = Command::stub(m.id, m.label);
        cmd.editing = m.editing;
        registry.register(cmd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::host::{EditableElement, EditableSlide};

    fn stub_session() -> EditSession<PptxHost, EditHistory> {
        let host = PptxHost {
            slides: vec![EditableSlide {
                path: "ppt/slides/slide1.xml".into(),
                elements: vec![
                    {
                        let mut el = EditableElement::blank("el-a", "shape");
                        el.shape_type = Some("rect".into());
                        el.set_box(10.0, 20.0, 100.0, 50.0, 0.0);
                        el
                    },
                    {
                        let mut el = EditableElement::blank("el-b", "shape");
                        el.shape_type = Some("rect".into());
                        el.set_box(40.0, 60.0, 80.0, 40.0, 0.0);
                        el
                    },
                ],
            }],
            dirty_parts: Default::default(),
        };
        EditSession::new(host, EditHistory::new())
    }

    #[test]
    fn catches_delete_shortcut_command_removes_selected_shape() {
        // Catches: home.delete not removing the selected shape via the editor surface (#88).
        let mut surface = PptxCommandSurface::new(false);
        let mut session = stub_session();
        let mut selection = SelectionState::new(0);
        selection.select(vec!["el-a".into()]);
        surface
            .dispatch("home.delete", &Value::Null, &mut selection, &mut session)
            .expect("delete");
        assert_eq!(session.host.slides[0].elements.len(), 1);
        assert_eq!(session.host.slides[0].elements[0].id, "el-b");
        assert!(!selection.has_selection());
        assert!(session.can_undo());
    }

    #[test]
    fn catches_readonly_blocks_insert_shape() {
        // Catches: insert.shape running while the pptx editor is read-only (#88).
        let mut surface = PptxCommandSurface::new(true);
        assert!(!surface.is_enabled("insert.shape"));
        let mut session = stub_session();
        let mut selection = SelectionState::new(0);
        assert!(surface
            .dispatch("insert.shape", &Value::Null, &mut selection, &mut session)
            .is_err());
    }
}
