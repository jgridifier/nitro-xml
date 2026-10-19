// SPDX-License-Identifier: MIT
//! Word ribbon / shortcut command ids → edit session (#83).

use std::collections::BTreeMap;

use nitro_xml_opc::edit::{
    ChangeKind, Command, CommandRegistry, EditError, EditHistory, EditHost, EditScope,
};
use serde_json::{json, Value};

use crate::edit::format::{set_paragraph_align, set_run_formatting};
use crate::edit::host::DocxHost;
use crate::edit::register_docx_command_registry;
use crate::edit::table::insert_table;
use crate::editor::print_edit::schedule_relayout;
use crate::editor::search::SearchController;

#[derive(Debug, Clone)]
pub struct CommandMeta {
    pub id: &'static str,
    pub label: &'static str,
    pub shortcut: Option<&'static str>,
    pub editing: bool,
}

macro_rules! word_cmd {
    ($id:expr, $label:expr, $shortcut:expr, $editing:expr) => {
        CommandMeta {
            id: $id,
            label: $label,
            shortcut: $shortcut,
            editing: $editing,
        }
    };
}

/// Stable ids aligned with upstream `RIBBON_ACTION_IDS` plus file/edit chrome.
pub const DOCX_EDITOR_COMMANDS: &[CommandMeta] = &[
    word_cmd!("file.save", "Save", Some("Ctrl+S"), true),
    word_cmd!("edit.undo", "Undo", Some("Ctrl+Z"), true),
    word_cmd!("edit.redo", "Redo", Some("Ctrl+Y"), true),
    word_cmd!("find-and-replace", "Find", Some("Ctrl+F"), false),
    word_cmd!("replace", "Replace", Some("Ctrl+H"), true),
    word_cmd!("bold", "Bold", Some("Ctrl+B"), true),
    word_cmd!("italic", "Italic", Some("Ctrl+I"), true),
    word_cmd!("underline", "Underline", Some("Ctrl+U"), true),
    word_cmd!("align-left", "Align Left", None, true),
    word_cmd!("align-center", "Center", None, true),
    word_cmd!("align-right", "Align Right", None, true),
    word_cmd!("insert.table", "Table", None, true),
    word_cmd!("print", "Print", Some("Ctrl+P"), false),
    word_cmd!("layout-view", "Print Layout", None, false),
    word_cmd!("paste", "Paste", Some("Ctrl+V"), true),
    word_cmd!("cut", "Cut", Some("Ctrl+X"), true),
    word_cmd!("copy", "Copy", Some("Ctrl+C"), false),
];

/// Chrome + script registry for one docx editor session.
pub struct DocxCommandSurface {
    pub registry: CommandRegistry,
    meta: BTreeMap<String, CommandMeta>,
    pub search: SearchController,
    pub search_panel_open: bool,
    pub search_focus: SearchPanelFocus,
    pub last_command: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchPanelFocus {
    #[default]
    Find,
    Replace,
}

impl DocxCommandSurface {
    pub fn new(read_only: bool) -> Self {
        let mut registry = CommandRegistry::new();
        register_docx_command_registry(&mut registry);
        register_editor_commands(&mut registry);
        let mut meta = BTreeMap::new();
        for m in DOCX_EDITOR_COMMANDS {
            meta.insert(m.id.to_string(), m.clone());
        }
        Self {
            registry,
            meta,
            search: SearchController::new(read_only),
            search_panel_open: false,
            search_focus: SearchPanelFocus::Find,
            last_command: None,
        }
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.registry.set_read_only(read_only);
        self.search.set_read_only(read_only);
        if read_only {
            self.search_panel_open = false;
        }
    }

    pub fn label(&self, id: &str) -> Option<&str> {
        self.meta.get(id).map(|m| m.label)
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.registry.is_enabled(id)
    }

    pub fn dispatch(
        &mut self,
        id: &str,
        args: &Value,
        print_edit: &mut crate::editor::print_edit::PrintEditState,
        session: &mut nitro_xml_opc::edit::EditSession<DocxHost, EditHistory>,
        caret_block: usize,
        caret_run: usize,
    ) -> Result<(), EditError> {
        if !self.registry.is_enabled(id) {
            return Err(EditError::unsupported(id));
        }
        self.last_command = Some(id.to_string());

        match id {
            // TODO(#80): `save_docx` when the writer lands on main.
            "file.save" => Err(EditError::unsupported("file.save")),
            "edit.undo" => {
                session.undo();
                schedule_relayout(print_edit);
                Ok(())
            }
            "edit.redo" => {
                session.redo();
                schedule_relayout(print_edit);
                Ok(())
            }
            "find-and-replace" => {
                self.search_panel_open = true;
                self.search_focus = SearchPanelFocus::Find;
                if let Some(q) = args.get("query").and_then(|v| v.as_str()) {
                    let case = args
                        .get("caseSensitive")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    self.search.search(&session.host, q, case);
                }
                Ok(())
            }
            "replace" => {
                self.search_panel_open = true;
                self.search_focus = SearchPanelFocus::Replace;
                Ok(())
            }
            "search.findNext" => {
                let cursor = args
                    .get("cursor")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                self.search.find_next(&session.host, cursor);
                Ok(())
            }
            "search.findPrevious" => {
                let cursor = args
                    .get("cursor")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                self.search.find_previous(&session.host, cursor);
                Ok(())
            }
            "search.replaceOne" => {
                let replacement = args
                    .get("replacement")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let cursor = args
                    .get("cursor")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                if self
                    .search
                    .replace_one(&mut session.host, replacement, cursor)
                {
                    schedule_relayout(print_edit);
                }
                Ok(())
            }
            "search.replaceAll" => {
                let replacement = args
                    .get("replacement")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let n = self.search.replace_all(&mut session.host, replacement);
                if n > 0 {
                    schedule_relayout(print_edit);
                }
                Ok(())
            }
            "bold" | "italic" | "underline" => {
                let toggle_key = match id {
                    "bold" => "bold",
                    "italic" => "italic",
                    _ => "underline",
                };
                let scopes = vec![EditScope::Workbook];
                session.run(
                    format!("toggle {toggle_key}"),
                    ChangeKind::Format,
                    scopes,
                    |h| {
                        let block = h
                            .paragraph(caret_block)
                            .map_err(|e| EditError::Other { message: e })?;
                        let runs =
                            block
                                .get("runs")
                                .and_then(|r| r.as_array())
                                .ok_or_else(|| EditError::Other {
                                    message: "runs missing".into(),
                                })?;
                        let run = runs.get(caret_run).ok_or_else(|| EditError::Other {
                            message: "run missing".into(),
                        })?;
                        let current = run
                            .get(toggle_key)
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let mut patch = serde_json::Map::new();
                        patch.insert(toggle_key.into(), json!(!current));
                        set_run_formatting(h, caret_block, caret_run, &patch)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "align-left" | "align-center" | "align-right" => {
                let align = match id {
                    "align-left" => "left",
                    "align-center" => "center",
                    _ => "right",
                };
                session.run(
                    format!("align {align}"),
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    |h| {
                        set_paragraph_align(h, caret_block, align)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "insert.table" => {
                let at = caret_block.saturating_add(1);
                session.run(
                    "insert.table",
                    ChangeKind::Structure,
                    vec![EditScope::Workbook],
                    |h| -> Result<(), EditError> {
                        insert_table(h, at, 2, 2)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            _ => {
                // Script ops (setRunFormatting, setParagraphText, …) must land on the
                // history stack so edit.undo restores them. The match arms above already do.
                let registry = &self.registry;
                session.run(
                    id.to_string(),
                    ChangeKind::Other,
                    vec![EditScope::Workbook],
                    |host| registry.run(id, host.as_any(), args).map(|_| ()),
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
        }
    }
}

pub fn register_editor_commands(registry: &mut CommandRegistry) {
    for m in DOCX_EDITOR_COMMANDS {
        let mut cmd = Command::stub(m.id, m.label);
        cmd.editing = m.editing;
        registry.register(cmd);
    }
    for id in [
        "search.findNext",
        "search.findPrevious",
        "search.replaceOne",
        "search.replaceAll",
    ] {
        registry.register(Command::stub(id, id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::print_edit::PrintEditState;
    use nitro_xml_opc::edit::EditSession;
    use serde_json::json;

    fn session_host() -> (EditSession<DocxHost, EditHistory>, DocxHost) {
        let host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": "Hi", "bold": false }],
        })]);
        let session = EditSession::new(host.clone(), EditHistory::new());
        (session, host)
    }

    #[test]
    fn catches_readonly_blocks_editing_commands() {
        // Catches: bold running while read-only (#83 negative).
        let mut surface = DocxCommandSurface::new(true);
        surface.set_read_only(true);
        assert!(!surface.is_enabled("bold"));
        let (mut session, _) = session_host();
        let mut print = PrintEditState::default();
        assert!(surface
            .dispatch("bold", &Value::Null, &mut print, &mut session, 0, 0)
            .is_err());
    }

    #[test]
    fn catches_script_format_op_undo_via_editor_dispatch() {
        // Catches: setRunFormatting mutating the host without a history step, so edit.undo is a no-op.
        let mut surface = DocxCommandSurface::new(false);
        let (mut session, _) = session_host();
        let mut print = PrintEditState::default();
        surface
            .dispatch(
                "setRunFormatting",
                &json!({ "blockIndex": 0, "runIndex": 0, "bold": true }),
                &mut print,
                &mut session,
                0,
                0,
            )
            .expect("format");
        assert_eq!(session.host.blocks[0]["runs"][0]["bold"], true);
        surface
            .dispatch("edit.undo", &Value::Null, &mut print, &mut session, 0, 0)
            .expect("undo");
        assert_eq!(session.host.blocks[0]["runs"][0]["bold"], false);
    }

    #[test]
    fn catches_insert_table_left_in_place_after_undo() {
        // Catches: insert.table never adding a table block, or undo leaving that table behind.
        let mut surface = DocxCommandSurface::new(false);
        let (mut session, _) = session_host();
        let mut print = PrintEditState::default();
        surface
            .dispatch("insert.table", &Value::Null, &mut print, &mut session, 0, 0)
            .expect("insert");
        assert!(
            session
                .host
                .blocks
                .iter()
                .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("table")),
            "table was not inserted"
        );
        surface
            .dispatch("edit.undo", &Value::Null, &mut print, &mut session, 0, 0)
            .expect("undo");
        assert!(
            session
                .host
                .blocks
                .iter()
                .all(|b| b.get("type").and_then(|t| t.as_str()) != Some("table")),
            "undo left the table in the document"
        );
    }
}
