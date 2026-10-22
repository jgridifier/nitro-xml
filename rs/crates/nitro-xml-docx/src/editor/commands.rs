// SPDX-License-Identifier: MIT
//! Word ribbon / shortcut command ids → edit session (#83).

use std::collections::BTreeMap;

use nitro_xml_opc::edit::{
    ChangeKind, Command, CommandRegistry, EditError, EditHistory, EditHost, EditScope,
};
use serde_json::{json, Value};

use crate::edit::drawing::append_inline_picture;
use crate::edit::format::{
    adjust_left_indent, adjust_right_indent, cycle_line_spacing, effective_character_spacing_twips,
    effective_run_font_pt, effective_text_scale_percent, set_character_spacing,
    set_paragraph_align, set_paragraph_style, set_run_formatting, set_text_scale,
    step_office_font_pt, toggle_caps, toggle_double_underline, toggle_first_line_indent,
    toggle_hanging_indent, toggle_paragraph_bottom_border, toggle_paragraph_shading,
    toggle_paragraph_space, toggle_vanish, CHAR_SPACING_LIMIT_TWIPS, CHAR_SPACING_STEP_TWIPS,
    INDENT_STEP_TWIPS, TEXT_SCALE_MAX_PERCENT, TEXT_SCALE_MIN_PERCENT, TEXT_SCALE_STEP_PERCENT,
};
use crate::edit::host::DocxHost;
use crate::edit::numbering::{apply_numbering, ensure_list_definition, toggle_bullet_list};
use crate::edit::register_docx_command_registry;
use crate::edit::table::{
    delete_table, delete_table_column, delete_table_row, insert_table, insert_table_column,
    insert_table_row, set_table_cell_text,
};

/// Plain text written into the first cell by `table.cell-text`.
const CELL_TEXT: &str = "Nitro cell";
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
    word_cmd!("double-underline", "Double Underline", None, true),
    word_cmd!("caps", "All Caps", None, true),
    word_cmd!("hidden", "Hidden", None, true),
    word_cmd!("expand-spacing", "Expand", None, true),
    word_cmd!("condense-spacing", "Condense", None, true),
    word_cmd!("scale-up", "Scale Up", None, true),
    word_cmd!("scale-down", "Scale Down", None, true),
    word_cmd!("strike", "Strikethrough", Some("Ctrl+5"), true),
    word_cmd!("highlight", "Highlight", None, true),
    word_cmd!("indent", "Increase Indent", None, true),
    word_cmd!("outdent", "Decrease Indent", None, true),
    word_cmd!("indent-right", "Increase Right Indent", None, true),
    word_cmd!("outdent-right", "Decrease Right Indent", None, true),
    word_cmd!("line-spacing", "Line Spacing", None, true),
    word_cmd!("border-bottom", "Bottom Border", None, true),
    word_cmd!("space-before", "Space Before", None, true),
    word_cmd!("shading", "Shade", None, true),
    word_cmd!("space-after", "Space After", None, true),
    word_cmd!("hanging-indent", "Hanging", None, true),
    word_cmd!("first-line", "First Line", None, true),
    word_cmd!("subscript", "Subscript", None, true),
    word_cmd!("superscript", "Superscript", None, true),
    word_cmd!("font-name", "Font", None, true),
    word_cmd!("bullets", "Bullets", None, true),
    word_cmd!("align-left", "Align Left", None, true),
    word_cmd!("align-center", "Center", None, true),
    word_cmd!("align-right", "Align Right", None, true),
    word_cmd!("align-justify", "Justify", None, true),
    word_cmd!("numbering", "Numbering", None, true),
    word_cmd!("style-normal", "Normal", None, true),
    word_cmd!("style-title", "Title", None, true),
    word_cmd!("style-heading-1", "Heading 1", None, true),
    word_cmd!("style-heading-2", "Heading 2", None, true),
    word_cmd!("style-heading-3", "Heading 3", None, true),
    word_cmd!("insert.page-break", "Page Break", None, true),
    word_cmd!("insert.table", "Table", None, true),
    word_cmd!("insert.picture", "Picture", None, true),
    word_cmd!("font-size", "Font Size", None, true),
    word_cmd!("grow-font", "Grow Font", None, true),
    word_cmd!("shrink-font", "Shrink Font", None, true),
    word_cmd!("font-color", "Font Color", None, true),
    word_cmd!("table.insert-row", "Insert Row", None, true),
    word_cmd!("table.delete-row", "Delete Row", None, true),
    word_cmd!("table.insert-column", "Insert Column", None, true),
    word_cmd!("table.delete-column", "Delete Column", None, true),
    word_cmd!("table.cell-text", "Cell Text", None, true),
    word_cmd!("table.delete", "Delete Table", None, true),
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
            "bold" | "italic" | "underline" | "strike" => {
                let toggle_key = match id {
                    "bold" => "bold",
                    "italic" => "italic",
                    "underline" => "underline",
                    _ => "strike",
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
                        if toggle_key == "underline" {
                            patch.insert("underlineStyle".into(), json!(""));
                        }
                        set_run_formatting(h, caret_block, caret_run, &patch)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "highlight" => {
                session.run(
                    String::from("highlight"),
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
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
                        let on = run
                            .get("highlight")
                            .and_then(|v| v.as_str())
                            .is_some_and(|s| !s.is_empty());
                        let mut patch = serde_json::Map::new();
                        patch.insert(
                            "highlight".into(),
                            if on { json!("") } else { json!("yellow") },
                        );
                        set_run_formatting(h, caret_block, caret_run, &patch)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "align-left" | "align-center" | "align-right" | "align-justify" => {
                let align = match id {
                    "align-left" => "left",
                    "align-center" => "center",
                    "align-right" => "right",
                    _ => "justify",
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
            "numbering" => {
                session.run(
                    String::from("numbering"),
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    |h| {
                        let id = ensure_list_definition(h, "decimal")?;
                        apply_numbering(h, caret_block, id, 0)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "bullets" => {
                session.run(
                    String::from("bullets"),
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    |h| {
                        toggle_bullet_list(h, caret_block)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "indent" | "outdent" => {
                let delta = if id == "indent" {
                    INDENT_STEP_TWIPS
                } else {
                    -INDENT_STEP_TWIPS
                };
                let current = session
                    .host
                    .paragraph(caret_block)
                    .map_err(|e| EditError::Other { message: e })?
                    .get("indentLeftTwips")
                    .and_then(|n| n.as_i64())
                    .unwrap_or(0);
                let next = if delta < 0 {
                    current.saturating_sub(delta.unsigned_abs() as i64).max(0)
                } else {
                    current.saturating_add(delta)
                };
                if next == current {
                    return Ok(());
                }
                session.run(id, ChangeKind::Format, vec![EditScope::Workbook], |h| {
                    adjust_left_indent(h, caret_block, delta)?;
                    Ok(())
                })?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "indent-right" | "outdent-right" => {
                let delta = if id == "indent-right" {
                    INDENT_STEP_TWIPS
                } else {
                    -INDENT_STEP_TWIPS
                };
                let current = session
                    .host
                    .paragraph(caret_block)
                    .map_err(|e| EditError::Other { message: e })?
                    .get("indentRightTwips")
                    .and_then(|n| n.as_i64())
                    .unwrap_or(0);
                let next = if delta < 0 {
                    current.saturating_sub(delta.unsigned_abs() as i64).max(0)
                } else {
                    current.saturating_add(delta)
                };
                if next == current {
                    return Ok(());
                }
                session.run(id, ChangeKind::Format, vec![EditScope::Workbook], |h| {
                    adjust_right_indent(h, caret_block, delta)?;
                    Ok(())
                })?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "line-spacing" => {
                session.run(
                    String::from("line-spacing"),
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    |h| {
                        cycle_line_spacing(h, caret_block)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "shading" => {
                session.run(
                    String::from("shading"),
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    |h| {
                        toggle_paragraph_shading(h, caret_block)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "border-bottom" => {
                session.run(
                    String::from("border-bottom"),
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    |h| {
                        toggle_paragraph_bottom_border(h, caret_block)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "space-before" | "space-after" => {
                let key = if id == "space-before" {
                    "spacingBeforeTwips"
                } else {
                    "spacingAfterTwips"
                };
                session.run(id, ChangeKind::Format, vec![EditScope::Workbook], |h| {
                    toggle_paragraph_space(h, caret_block, key)?;
                    Ok(())
                })?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "hanging-indent" => {
                session.run(
                    String::from("hanging-indent"),
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    |h| {
                        toggle_hanging_indent(h, caret_block)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "double-underline" => {
                session.run(
                    String::from("double-underline"),
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    |h| {
                        toggle_double_underline(h, caret_block, caret_run)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "caps" => {
                session.run(
                    String::from("caps"),
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    |h| {
                        toggle_caps(h, caret_block, caret_run)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "hidden" => {
                session.run(
                    String::from("hidden"),
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    |h| {
                        toggle_vanish(h, caret_block, caret_run)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "first-line" => {
                session.run(
                    String::from("first-line"),
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    |h| {
                        toggle_first_line_indent(h, caret_block)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "subscript" | "superscript" => {
                let want = if id == "subscript" {
                    "subscript"
                } else {
                    "superscript"
                };
                session.run(id, ChangeKind::Format, vec![EditScope::Workbook], |h| {
                    let block = h
                        .paragraph(caret_block)
                        .map_err(|e| EditError::Other { message: e })?;
                    let runs = block
                        .get("runs")
                        .and_then(|r| r.as_array())
                        .ok_or_else(|| EditError::Other {
                            message: "runs missing".into(),
                        })?;
                    let run = runs.get(caret_run).ok_or_else(|| EditError::Other {
                        message: "run missing".into(),
                    })?;
                    let current = run
                        .get("verticalAlign")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let mut patch = serde_json::Map::new();
                    patch.insert(
                        "verticalAlign".into(),
                        if current == want {
                            json!("")
                        } else {
                            json!(want)
                        },
                    );
                    set_run_formatting(h, caret_block, caret_run, &patch)?;
                    Ok(())
                })?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "font-name" => {
                session.run(
                    String::from("font-name"),
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
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
                        let current = run.get("fontFamily").and_then(|v| v.as_str()).unwrap_or("");
                        let next = if current == "Georgia" {
                            "Calibri"
                        } else {
                            "Georgia"
                        };
                        let mut patch = serde_json::Map::new();
                        patch.insert("fontFamily".into(), json!(next));
                        set_run_formatting(h, caret_block, caret_run, &patch)?;
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
            "style-normal" | "style-title" | "style-heading-1" | "style-heading-2"
            | "style-heading-3" => {
                let style_id = match id {
                    "style-normal" => "Normal",
                    "style-title" => "Title",
                    "style-heading-1" => "Heading1",
                    "style-heading-2" => "Heading2",
                    _ => "Heading3",
                };
                let current = session
                    .host
                    .paragraph(caret_block)
                    .ok()
                    .and_then(|block| block.get("style").and_then(|v| v.as_str()))
                    .map(str::to_owned);
                if current.as_deref() == Some(style_id) {
                    return Ok(());
                }
                session.run(
                    format!("style {style_id}"),
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    |h| set_paragraph_style(h, caret_block, style_id),
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "insert.page-break" => {
                let at = caret_block.saturating_add(1);
                session.run(
                    "insert.page-break",
                    ChangeKind::Structure,
                    vec![EditScope::Workbook],
                    |h| insert_page_break(h, at),
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "insert.picture" => {
                session.run(
                    String::from("insert.picture"),
                    ChangeKind::Structure,
                    vec![EditScope::Workbook],
                    |h| {
                        append_inline_picture(h, caret_block)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "expand-spacing" | "condense-spacing" => {
                let delta = if id == "expand-spacing" {
                    CHAR_SPACING_STEP_TWIPS
                } else {
                    -CHAR_SPACING_STEP_TWIPS
                };
                let current =
                    effective_character_spacing_twips(&session.host, caret_block, caret_run);
                let next =
                    (current + delta).clamp(-CHAR_SPACING_LIMIT_TWIPS, CHAR_SPACING_LIMIT_TWIPS);
                if next == current {
                    return Ok(());
                }
                session.run(
                    id,
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    move |h| {
                        set_character_spacing(h, caret_block, caret_run, next)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "scale-up" | "scale-down" => {
                let delta = if id == "scale-up" {
                    TEXT_SCALE_STEP_PERCENT
                } else {
                    -TEXT_SCALE_STEP_PERCENT
                };
                let current = effective_text_scale_percent(&session.host, caret_block, caret_run);
                let next = (current + delta).clamp(TEXT_SCALE_MIN_PERCENT, TEXT_SCALE_MAX_PERCENT);
                if next == current {
                    return Ok(());
                }
                session.run(
                    id,
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    move |h| {
                        set_text_scale(h, caret_block, caret_run, next)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "grow-font" | "shrink-font" => {
                let grow = id == "grow-font";
                let current = effective_run_font_pt(&session.host, caret_block, caret_run);
                let next = step_office_font_pt(current, grow);
                if (next - current).abs() < f64::EPSILON {
                    return Ok(());
                }
                session.run(
                    id,
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    move |h| {
                        let mut patch = serde_json::Map::new();
                        patch.insert("fontSize".into(), json!(next));
                        set_run_formatting(h, caret_block, caret_run, &patch)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "font-size" | "font-color" => {
                let mut patch = serde_json::Map::new();
                let label = if id == "font-size" {
                    patch.insert("fontSize".into(), json!(18.0));
                    "font-size"
                } else {
                    patch.insert("color".into(), json!("C00000"));
                    "font-color"
                };
                session.run(
                    label,
                    ChangeKind::Format,
                    vec![EditScope::Workbook],
                    move |h| -> Result<(), EditError> {
                        set_run_formatting(h, caret_block, caret_run, &patch)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "table.delete" => {
                let Ok(at) = first_table(&session.host) else {
                    return Ok(());
                };
                session.run(
                    "table.delete",
                    ChangeKind::Structure,
                    vec![EditScope::Workbook],
                    move |h| -> Result<(), EditError> {
                        delete_table(h, at)?;
                        Ok(())
                    },
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "table.cell-text" => {
                let Ok(bi) = first_table(&session.host) else {
                    return Ok(());
                };
                if table_cell_plain(&session.host, bi).as_deref() == Some(CELL_TEXT) {
                    return Ok(());
                }
                session.run(
                    "table.cell-text",
                    ChangeKind::Other,
                    vec![EditScope::Workbook],
                    move |h| set_table_cell_text(h, bi, 0, 0, CELL_TEXT),
                )?;
                schedule_relayout(print_edit);
                Ok(())
            }
            "table.insert-row"
            | "table.delete-row"
            | "table.insert-column"
            | "table.delete-column" => {
                let label = match id {
                    "table.insert-row" => "table.insert-row",
                    "table.delete-row" => "table.delete-row",
                    "table.insert-column" => "table.insert-column",
                    _ => "table.delete-column",
                };
                session.run(
                    label,
                    ChangeKind::Structure,
                    vec![EditScope::Workbook],
                    |h| -> Result<(), EditError> {
                        let bi = first_table(h)?;
                        match label {
                            "table.insert-row" => {
                                let n = table_extent(h, bi, true);
                                insert_table_row(h, bi, n)
                            }
                            "table.delete-row" => {
                                let n = table_extent(h, bi, true);
                                delete_table_row(h, bi, n.saturating_sub(1))
                            }
                            "table.insert-column" => {
                                let n = table_extent(h, bi, false);
                                insert_table_column(h, bi, n)
                            }
                            _ => {
                                let n = table_extent(h, bi, false);
                                delete_table_column(h, bi, n.saturating_sub(1))
                            }
                        }
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

fn insert_page_break(host: &mut DocxHost, at: usize) -> Result<(), EditError> {
    let at = at.min(host.blocks.len());
    let id = format!("page-break-{at}-{}", host.blocks.len());
    host.blocks.insert(
        at,
        json!({
            "type": "paragraph",
            "id": id,
            "runs": [{ "text": "", "break": "page" }],
        }),
    );
    let mut shifted = std::collections::BTreeSet::new();
    for index in host.touched.iter().copied() {
        shifted.insert(if index >= at { index + 1 } else { index });
    }
    shifted.insert(at);
    host.touched = shifted;
    Ok(())
}

fn table_cell_plain(host: &DocxHost, block: usize) -> Option<String> {
    let cell = host
        .blocks
        .get(block)?
        .get("rows")?
        .as_array()?
        .first()?
        .as_array()?
        .first()?;
    let runs = cell
        .get("paragraphs")?
        .as_array()?
        .first()?
        .get("runs")?
        .as_array()?;
    let mut text = String::new();
    for run in runs {
        if let Some(part) = run.get("text").and_then(|v| v.as_str()) {
            text.push_str(part);
        }
    }
    Some(text)
}

fn first_table(host: &DocxHost) -> Result<usize, EditError> {
    host.blocks
        .iter()
        .position(|b| b.get("type").and_then(|t| t.as_str()) == Some("table"))
        .ok_or_else(|| EditError::Other {
            message: "no table in the document".into(),
        })
}

fn table_extent(host: &DocxHost, block: usize, rows: bool) -> usize {
    let table = &host.blocks[block];
    if rows {
        table
            .get("rows")
            .and_then(|r| r.as_array())
            .map(|r| r.len())
            .unwrap_or(0)
    } else {
        table
            .get("rows")
            .and_then(|r| r.as_array())
            .and_then(|r| r.first())
            .and_then(|r| r.as_array())
            .map(|c| c.len())
            .unwrap_or(0)
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
