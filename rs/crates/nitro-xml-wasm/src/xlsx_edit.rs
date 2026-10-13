// SPDX-License-Identifier: MIT
//! Wasm glue for the xlsx editor UI (#69): text surface keys, chrome commands, save.

use nitro_xml_xlsx::address::{format_address, MAX_COL, MAX_ROW};
use nitro_xml_xlsx::editor::{ChromeEffect, EditorSession, SelectionTarget};
use nitro_xml_xlsx::model::{CellAddress, SheetSelection};
use nitro_xml_xlsx::render::{select_cell, selection_ref};
use serde_json::Value;

use crate::text::{apply_key, cut_plain, KeyAction, KeyEvent, TextField};
use crate::xlsx::XlsxState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorKeyEffect {
    Ignored,
    Handled,
    /// Commit or a navigation key; row/col deltas move the active cell afterwards.
    Move {
        row: i32,
        col: i32,
    },
}

impl XlsxState {
    pub fn ensure_edit(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.edit.is_some() {
            return Ok(());
        }
        self.edit = Some(EditorSession::open(bytes)?);
        self.sync_model_from_edit();
        Ok(())
    }

    pub fn sync_model_from_edit(&mut self) {
        let Some(edit) = self.edit.as_ref() else {
            return;
        };
        let workbook = edit.host().workbook.clone();
        let host_sheets = edit.host().sheets.clone();
        let active_sheet = workbook.active_sheet as usize;
        let sels: Vec<Option<SheetSelection>> = self
            .sheets
            .iter()
            .map(|s| s.as_ref().and_then(|sh| sh.view.selection.clone()))
            .collect();
        let mut sheets: Vec<Option<_>> = host_sheets.into_iter().map(Some).collect();
        for (i, sel) in sels.into_iter().enumerate() {
            if let (Some(sel), Some(Some(sh))) = (sel, sheets.get_mut(i)) {
                sh.view.selection = Some(sel);
            }
        }
        self.workbook = workbook;
        self.sheets = sheets;
        if self.active >= self.sheets.len() {
            self.active = active_sheet.min(self.sheets.len().saturating_sub(1));
        }
    }

    pub fn selection_target(&self) -> SelectionTarget {
        let sheet = self.active as u32;
        let sel = self
            .sheets
            .get(self.active)
            .and_then(|s| s.as_ref())
            .and_then(|s| s.view.selection.as_ref());
        match sel {
            Some(sel) => {
                let range = sel
                    .ranges
                    .first()
                    .map(nitro_xml_xlsx::format_range)
                    .unwrap_or_else(|| format_address(&sel.active));
                SelectionTarget {
                    sheet,
                    row: sel.active.row,
                    col: sel.active.col,
                    range,
                }
            }
            None => SelectionTarget::cell(sheet, 0, 0),
        }
    }

    pub fn formula_bar_html(&self, read_only: bool) -> String {
        let Some(edit) = &self.edit else {
            return String::new();
        };
        let view = edit.formula_bar(&self.selection_target(), read_only);
        nitro_xml_xlsx::editor::formula_bar_html(&view)
    }

    pub fn save_bytes(&self, original: &[u8]) -> Result<Vec<u8>, String> {
        match &self.edit {
            Some(edit) => edit.save(original),
            None => Ok(original.to_vec()),
        }
    }

    pub fn dispatch_chrome(&mut self, id: &str) -> Result<ChromeEffect, String> {
        let target = self.selection_target();
        let edit = self
            .edit
            .as_mut()
            .ok_or_else(|| "no edit session".to_string())?;
        let effect = edit
            .dispatch_chrome(id, &target)
            .map_err(|e| e.to_string())?;
        if matches!(effect, ChromeEffect::Applied { .. }) {
            let structure = matches!(effect, ChromeEffect::Applied { structure: true });
            self.sync_model_from_edit();
            if structure {
                self.adopt_host_active_sheet();
            }
        }
        Ok(effect)
    }

    pub fn apply_raw_edit(&mut self, spec: &str) -> Result<ChromeEffect, String> {
        let v: Value = serde_json::from_str(spec).map_err(|e| e.to_string())?;
        let command = v
            .get("command")
            .and_then(|c| c.as_str())
            .ok_or_else(|| "missing command".to_string())?
            .to_owned();
        if is_chrome_id(&command) {
            return self.dispatch_chrome(&command);
        }
        let mut args = v;
        if let Some(obj) = args.as_object_mut() {
            obj.remove("command");
        }
        let edit = self
            .edit
            .as_mut()
            .ok_or_else(|| "no edit session".to_string())?;
        edit.apply_raw(&command, args).map_err(|e| e.to_string())?;
        let structure = matches!(command.as_str(), "addSheet" | "renameSheet" | "deleteSheet");
        self.sync_model_from_edit();
        if structure {
            self.adopt_host_active_sheet();
        }
        Ok(ChromeEffect::Applied { structure })
    }

    pub fn editor_key(
        &mut self,
        key: &str,
        shift: bool,
        ctrl: bool,
        meta: bool,
        alt: bool,
        mac: bool,
    ) -> Result<EditorKeyEffect, String> {
        let target = self.selection_target();
        let editing = self
            .edit
            .as_ref()
            .map(|edit| edit.cell().is_editing())
            .unwrap_or(false);
        if !editing {
            return self.editor_key_idle(key, shift, ctrl, meta, alt, &target);
        }
        let effect = {
            let edit = self
                .edit
                .as_mut()
                .ok_or_else(|| "no edit session".to_string())?;
            apply_surface_key(edit, key, shift, ctrl, meta, alt, mac)
        };
        match effect {
            Surface::Commit => {
                self.formula_commit()?;
                let (row, col) = nudge_for(key, shift);
                if row != 0 || col != 0 {
                    let _ = self.nudge_active(row, col);
                }
                Ok(EditorKeyEffect::Move { row, col })
            }
            Surface::Cancel => {
                self.formula_cancel();
                Ok(EditorKeyEffect::Handled)
            }
            Surface::Edited => Ok(EditorKeyEffect::Handled),
            Surface::Ignored => Ok(EditorKeyEffect::Ignored),
        }
    }

    pub fn formula_cancel(&mut self) {
        if let Some(edit) = self.edit.as_mut() {
            edit.cancel_edit();
        }
    }

    pub fn formula_commit(&mut self) -> Result<bool, String> {
        let Some(edit) = self.edit.as_mut() else {
            return Ok(false);
        };
        let committed = edit.commit_edit().map_err(|e| e.to_string())?;
        if committed {
            self.sync_model_from_edit();
        }
        Ok(committed)
    }

    pub fn formula_fx(&mut self) -> Result<(), String> {
        let target = self.selection_target();
        let edit = self
            .edit
            .as_mut()
            .ok_or_else(|| "no edit session".to_string())?;
        edit.insert_function(&target).map_err(|e| e.to_string())
    }

    pub fn formula_expand(&mut self) {
        if let Some(edit) = self.edit.as_mut() {
            edit.cell_mut().toggle_expanded();
        }
    }

    pub fn formula_focus(&mut self) -> Result<(), String> {
        let target = self.selection_target();
        let edit = self
            .edit
            .as_mut()
            .ok_or_else(|| "no edit session".to_string())?;
        edit.begin_edit(&target, None).map_err(|e| e.to_string())
    }

    pub fn commit_live_edit(&mut self) -> Result<bool, String> {
        self.formula_commit()
    }

    fn editor_key_idle(
        &mut self,
        key: &str,
        shift: bool,
        ctrl: bool,
        meta: bool,
        alt: bool,
        target: &SelectionTarget,
    ) -> Result<EditorKeyEffect, String> {
        if alt || ctrl || meta {
            return Ok(EditorKeyEffect::Ignored);
        }
        if key == "F2" {
            let edit = self.edit.as_mut().expect("edit");
            edit.begin_edit(target, None).map_err(|e| e.to_string())?;
            return Ok(EditorKeyEffect::Handled);
        }
        if key == "Delete" || key == "Backspace" {
            let edit = self.edit.as_mut().expect("edit");
            edit.apply_raw(
                "setCellInput",
                serde_json::json!({
                    "sheet": target.sheet,
                    "row": target.row,
                    "col": target.col,
                    "text": "",
                }),
            )
            .map_err(|e| e.to_string())?;
            self.sync_model_from_edit();
            return Ok(EditorKeyEffect::Handled);
        }
        if key == "Enter" {
            let row = if shift { -1 } else { 1 };
            let _ = self.nudge_active(row, 0);
            return Ok(EditorKeyEffect::Move { row, col: 0 });
        }
        if is_printable(key) {
            let edit = self.edit.as_mut().expect("edit");
            edit.begin_edit(target, Some(key.to_owned()))
                .map_err(|e| e.to_string())?;
            return Ok(EditorKeyEffect::Handled);
        }
        Ok(EditorKeyEffect::Ignored)
    }

    fn adopt_host_active_sheet(&mut self) {
        let Some(edit) = self.edit.as_ref() else {
            return;
        };
        let idx = edit.host().workbook.active_sheet as usize;
        if idx < self.sheets.len() {
            self.active = idx;
            self.workbook.active_sheet = idx as u32;
        }
    }

    fn nudge_active(&mut self, d_row: i32, d_col: i32) -> Option<String> {
        let active = self.active;
        let sheet = self.sheets.get_mut(active)?.as_mut()?;
        let current = sheet
            .view
            .selection
            .clone()
            .unwrap_or_else(|| select_cell(sheet, CellAddress { row: 0, col: 0 }));
        let row = (current.active.row as i32 + d_row).clamp(0, MAX_ROW as i32) as u32;
        let col = (current.active.col as i32 + d_col).clamp(0, MAX_COL as i32) as u32;
        let next = select_cell(sheet, CellAddress { row, col });
        let text = selection_ref(&next);
        sheet.view.selection = Some(next);
        Some(text)
    }
}

enum Surface {
    Commit,
    Cancel,
    Edited,
    Ignored,
}

fn apply_surface_key(
    edit: &mut EditorSession,
    key: &str,
    shift: bool,
    ctrl: bool,
    meta: bool,
    alt: bool,
    mac: bool,
) -> Surface {
    let cell = edit.cell();
    let mut field = TextField::new(cell.text());
    field.set_caret(cell.anchor());
    field.extend_to(cell.focus());
    let action = apply_key(
        &mut field,
        &KeyEvent {
            key: key.to_owned(),
            shift,
            ctrl,
            meta,
            alt,
            is_mac: mac,
        },
    );
    match action {
        KeyAction::Commit => Surface::Commit,
        KeyAction::Cancel => Surface::Cancel,
        KeyAction::Unhandled => {
            if is_printable(key) && !ctrl && !meta && !alt {
                field.insert(key);
                edit.cell_mut()
                    .set_buffer(field.text().to_owned(), field.anchor(), field.focus());
                Surface::Edited
            } else {
                Surface::Ignored
            }
        }
        KeyAction::Cut => {
            let _ = cut_plain(&mut field);
            edit.cell_mut()
                .set_buffer(field.text().to_owned(), field.anchor(), field.focus());
            Surface::Edited
        }
        KeyAction::Copy | KeyAction::Paste | KeyAction::Handled => {
            edit.cell_mut()
                .set_buffer(field.text().to_owned(), field.anchor(), field.focus());
            Surface::Edited
        }
    }
}

fn nudge_for(key: &str, shift: bool) -> (i32, i32) {
    match key {
        "Enter" if shift => (-1, 0),
        "Enter" => (1, 0),
        "Tab" if shift => (0, -1),
        "Tab" => (0, 1),
        _ => (0, 0),
    }
}

fn is_printable(key: &str) -> bool {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => !c.is_control(),
        _ => false,
    }
}

fn is_chrome_id(id: &str) -> bool {
    id.starts_with("home.")
        || id.starts_with("sheet.")
        || id.starts_with("edit.")
        || id.starts_with("file.")
        || id.starts_with("cells.")
        || id.starts_with("format.")
}

pub fn editor_key_json(effect: EditorKeyEffect, selection: Option<String>) -> String {
    let (handled, row, col) = match effect {
        EditorKeyEffect::Ignored => (false, 0, 0),
        EditorKeyEffect::Handled => (true, 0, 0),
        EditorKeyEffect::Move { row, col } => (true, row, col),
    };
    serde_json::json!({
        "handled": handled,
        "moveRow": row,
        "moveCol": col,
        "selection": selection,
    })
    .to_string()
}
