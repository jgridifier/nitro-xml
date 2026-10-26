// SPDX-License-Identifier: MIT
//! Wasm glue for the xlsx editor UI (#69): text surface keys, chrome commands, save.

use nitro_xml_xlsx::address::{format_address, MAX_COL, MAX_ROW};
use nitro_xml_xlsx::edit::XlsxHost;
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
        if self.lazy_first_view_mut().is_some() {
            // Soft-share reloads every sheet via parse_worksheet (full part + merges/CF).
            // Skip ensure_sheet_complete SeekTail — it re-streams sheetData only to be discarded.
            self.clear_pending_tail_complete();
            let (workbook, sheets) = self
                .lazy_first_view_mut()
                .expect("lazy first view")
                .materialize_worksheets_for_edit()?;
            let host = XlsxHost::from_parts(workbook, sheets)?;
            self.edit = Some(EditorSession::from_host(host));
            self.workbook = self.edit.as_ref().expect("edit").host().workbook.clone();
            self.sheets = vec![None; self.workbook.sheets.len()];
            self.grid_in_host = true;
            let active_sheet = self.workbook.active_sheet as usize;
            if self.active >= self.sheets.len() {
                self.active = active_sheet.min(self.sheets.len().saturating_sub(1));
            }
        } else {
            self.edit = Some(EditorSession::open(bytes)?);
            self.sync_model_from_edit();
        }
        Ok(())
    }

    /// Full host→viewer copy. Required after structure edits and on first `ensure_edit` from bytes (non-lazy).
    /// Cell typing must not use this: cloning every DumpCell on cells_100k/1m is ~150 ms/edit.
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
        // Host open rebuilds cell_pos; cloning keeps it. Re-ensure so paint never falls
        // back to CellIndex::build over 1M cells after a partial clone path.
        for sh in sheets.iter_mut().flatten() {
            sh.ensure_cell_pos();
        }
        self.workbook = workbook;
        self.sheets = sheets;
        self.grid_in_host = false;
        if self.active >= self.sheets.len() {
            self.active = active_sheet.min(self.sheets.len().saturating_sub(1));
        }
    }

    /// Patch styles + the addressed cells only (keeps viewer sheet `cells` vec identity).
    fn sync_cells_from_edit(&mut self, sheet: u32, coords: &[(u32, u32)]) {
        if self.grid_in_host {
            return;
        }
        let Some(edit) = self.edit.as_ref() else {
            return;
        };
        let host = edit.host();
        // Styles stay small; SST / sheet grids must not be cloned per keystroke.
        self.workbook.styles = host.workbook.styles.clone();
        self.workbook.active_sheet = host.workbook.active_sheet;
        let Some(host_sh) = host.sheets.get(sheet as usize) else {
            self.sync_model_from_edit();
            return;
        };
        let Some(Some(sh)) = self.sheets.get_mut(sheet as usize) else {
            self.sync_model_from_edit();
            return;
        };
        sh.ensure_cell_pos();
        for &(row, col) in coords {
            let host_cell = host_sh
                .find_cell_pos(row, col)
                .map(|i| host_sh.cells[i].clone());
            match host_cell {
                Some(cell) => sh.upsert_dump_cell(cell),
                None => sh.remove_cell_at_coord(row, col),
            }
        }
    }

    pub fn selection_target(&self) -> SelectionTarget {
        let sheet = self.active as u32;
        let sel = self
            .sheet_ref(self.active)
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
        // Insert/Delete Rows/Columns keep the selection on the same cells (the engine shifts it with the content).
        let keep_selection = id
            .starts_with("cells.")
            .then(|| {
                self.sheet_ref(self.active)
                    .and_then(|s| s.view.selection.clone())
            })
            .flatten();
        let edit = self
            .edit
            .as_mut()
            .ok_or_else(|| "no edit session".to_string())?;
        let effect = edit
            .dispatch_chrome(id, &target)
            .map_err(|e| e.to_string())?;
        if matches!(effect, ChromeEffect::Applied { .. }) {
            let structure = matches!(effect, ChromeEffect::Applied { structure: true });
            if structure {
                self.sync_model_from_edit();
                self.adopt_host_active_sheet();
                if let Some(sel) = keep_selection {
                    let active = self.active;
                    if let Some(sheet) = self.sheet_mut(active) {
                        sheet.view.selection = Some(sel);
                    }
                }
            } else {
                // Bold / number-format etc.: sync the active cell (selection), not the grid.
                self.sync_cells_from_edit(target.sheet, &[(target.row, target.col)]);
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
        edit.apply_raw(&command, args.clone())
            .map_err(|e| e.to_string())?;
        let structure = matches!(command.as_str(), "addSheet" | "renameSheet" | "deleteSheet");
        if structure
            || matches!(
                command.as_str(),
                "insertRows" | "deleteRows" | "insertColumns" | "deleteColumns"
            )
        {
            self.sync_model_from_edit();
            if structure {
                self.adopt_host_active_sheet();
            }
        } else if matches!(command.as_str(), "setCellInput" | "setCellValue") {
            let sheet = args.get("sheet").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let row = args.get("row").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let col = args.get("col").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            self.sync_cells_from_edit(sheet, &[(row, col)]);
        } else if matches!(command.as_str(), "applyStyle" | "applyCellStyle") {
            let sheet = args.get("sheet").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let range = args.get("range").and_then(|x| x.as_str()).unwrap_or("");
            if let Some(coords) = coords_for_style_range(range) {
                self.sync_cells_from_edit(sheet, &coords);
            } else {
                self.sync_model_from_edit();
            }
        } else {
            self.sync_model_from_edit();
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
        let target = self.selection_target();
        let Some(edit) = self.edit.as_mut() else {
            return Ok(false);
        };
        let committed = edit.commit_edit().map_err(|e| e.to_string())?;
        if committed {
            self.sync_cells_from_edit(target.sheet, &[(target.row, target.col)]);
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
            self.sync_cells_from_edit(target.sheet, &[(target.row, target.col)]);
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
        let sheet = self.sheet_mut(active)?;
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

/// Expand an A1-style style range to cell coords. Large ranges return `None` → full sync.
fn coords_for_style_range(range: &str) -> Option<Vec<(u32, u32)>> {
    let r = nitro_xml_xlsx::parse_range(range)?;
    let rows = r.end.row.saturating_sub(r.start.row).saturating_add(1);
    let cols = r.end.col.saturating_sub(r.start.col).saturating_add(1);
    if rows.saturating_mul(cols) > 4_096 {
        return None;
    }
    let mut out = Vec::with_capacity((rows * cols) as usize);
    for row in r.start.row..=r.end.row {
        for col in r.start.col..=r.end.col {
            out.push((row, col));
        }
    }
    Some(out)
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

#[cfg(test)]
mod tests {
    use crate::handle::Handle;
    use crate::xlsx_edit::EditorKeyEffect;
    use nitro_xml_xlsx::model::DumpCellValue;
    use std::path::PathBuf;

    #[test]
    fn catches_ensure_edit_soft_share_materializes_full_sheet_not_viewport() {
        // Catches: ensure_edit reusing viewport-only lazy cells so edits off-window or
        // formula range scans see a truncated grid.
        use nitro_xml_xlsx::edit::XlsxHost;
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/generated/xlsx/cells_100k.xlsx");
        let bytes = std::fs::read(&path).expect("cells_100k");
        let open_count = XlsxHost::open(&bytes).expect("open").sheets[0].cells.len();
        let mut h = Handle::new(bytes);
        h.open_document("cells_100k.xlsx").expect("open");
        let viewport_cells = h.xlsx.as_ref().unwrap().sheets[0]
            .as_ref()
            .map(|s| s.cells.len())
            .unwrap_or(0);
        assert!(
            viewport_cells * 4 < open_count,
            "precondition: sparse viewport ({viewport_cells} vs {open_count})"
        );
        h.load_editor_chrome();
        h.set_read_only(false);
        h.apply_edit(r#"{"command":"setCellInput","sheet":0,"row":0,"col":0,"text":"full-grid"}"#)
            .expect("edit");
        let viewer = h.xlsx.as_ref().unwrap().sheet_ref(0).expect("sheet0");
        let host = h.xlsx.as_ref().unwrap().edit.as_ref().unwrap().host();
        assert_eq!(viewer.cells.len(), open_count);
        assert!(viewer.cells.len() > viewport_cells);
        assert_eq!(viewer.cells.len(), host.sheets[0].cells.len());
    }

    #[test]
    fn catches_ensure_edit_before_process_frame_materializes() {
        // Catches: ensure_edit requiring a prior host tick before lazy materialize runs.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/generated/xlsx/cells_1k.xlsx");
        let bytes = std::fs::read(&path).expect("cells_1k");
        let mut h = Handle::new(bytes);
        h.open_document("cells_1k.xlsx").expect("open");
        h.load_editor_chrome();
        h.set_read_only(false);
        h.apply_edit(r#"{"command":"setCellInput","sheet":0,"row":0,"col":0,"text":"no-tick"}"#)
            .expect("edit");
        assert!(
            h.xlsx.as_ref().unwrap().edit.is_some(),
            "edit session must open without process_frame"
        );
        let n = h
            .xlsx
            .as_ref()
            .unwrap()
            .sheet_ref(0)
            .map(|s| s.cells.len())
            .unwrap_or(0);
        assert!(n > 900, "expected full 1k sheet after ensure_edit, got {n}");
    }

    #[test]
    fn catches_ensure_edit_falls_back_when_lazy_none() {
        // Catches: ensure_edit failing or no-op when the viewer upgraded off lazy first-view.
        use nitro_xml_xlsx::edit::XlsxHost;
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/generated/xlsx/cells_1k.xlsx");
        let bytes = std::fs::read(&path).expect("cells_1k");
        let open_count = XlsxHost::open(&bytes).expect("open").sheets[0].cells.len();
        let mut h = Handle::new(bytes);
        h.open_document("cells_1k.xlsx").expect("open");
        h.xlsx.as_mut().unwrap().drop_lazy_view();
        assert!(!h.xlsx.as_ref().unwrap().lazy_is_some_for_test());
        h.load_editor_chrome();
        h.set_read_only(false);
        h.apply_edit(r#"{"command":"setCellInput","sheet":0,"row":0,"col":0,"text":"fallback"}"#)
            .expect("edit");
        let host_n = h
            .xlsx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .host()
            .sheets[0]
            .cells
            .len();
        assert_eq!(host_n, open_count);
    }

    #[test]
    fn catches_soft_share_host_cell_count_matches_xlsx_host_open() {
        // Catches: lazy soft-share materialize diverging from the editor open oracle path.
        use nitro_xml_xlsx::edit::XlsxHost;
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/generated/xlsx/cells_100k.xlsx");
        let bytes = std::fs::read(&path).expect("cells_100k");
        let open_count = XlsxHost::open(&bytes).expect("open").sheets[0].cells.len();
        let mut h = Handle::new(bytes);
        h.open_document("cells_100k.xlsx").expect("open");
        h.load_editor_chrome();
        h.set_read_only(false);
        h.apply_edit(r#"{"command":"setCellInput","sheet":0,"row":0,"col":0,"text":"parity"}"#)
            .expect("edit");
        let host_n = h
            .xlsx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .host()
            .sheets[0]
            .cells
            .len();
        assert_eq!(host_n, open_count);
    }

    #[test]
    fn catches_ensure_edit_keeps_lazy_first_view_alive() {
        // Catches: ensure_edit dropping lazy FirstView — that removed +0.6 MiB edit heap
        // regress but paint must still use cell_pos on the edit-synced grid while lazy
        // stays inert (no sync_lazy_sheet re-stream during edit ticks).
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/generated/xlsx/cells_100k.xlsx");
        let bytes = std::fs::read(&path).expect("cells_100k");
        let mut h = Handle::new(bytes);
        h.open_document("cells_100k.xlsx").expect("open");
        assert!(
            h.xlsx.as_ref().unwrap().lazy_is_some_for_test(),
            "precondition: open uses lazy first view"
        );
        h.load_editor_chrome();
        h.set_read_only(false);
        h.apply_edit(r#"{"command":"setCellInput","sheet":0,"row":0,"col":0,"text":"lazy-keep"}"#)
            .expect("edit");
        let x = h.xlsx.as_ref().unwrap();
        assert!(x.edit.is_some(), "edit session open");
        assert!(
            x.lazy_is_some_for_test(),
            "lazy first view must stay alive on ensure_edit (heap-neutral path)"
        );
        let sh = x.sheet_ref(0).expect("sheet0");
        assert_eq!(
            sh.cell_pos.synced_len(),
            sh.cells.len(),
            "viewer cell_pos must be synced for paint after ensure_edit"
        );
    }

    #[test]
    fn catches_cell_pos_index_staying_synced_after_set_cell_input() {
        // Catches: sync_cells_from_edit / ensure_cell scanning all DumpCells per
        // keystroke on large sheets (cells_1m edit trap). Index len must stay in
        // lockstep with cells after targeted sync.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/generated/xlsx/cells_100k.xlsx");
        let bytes = std::fs::read(&path).expect("cells_100k");
        let mut h = Handle::new(bytes);
        h.open_document("cells_100k.xlsx").expect("open");
        h.load_editor_chrome();
        h.set_read_only(false);

        for (row, col, text) in [
            (0u32, 0u32, "idx-a1"),
            (50, 3, "idx-b"),
            (999, 0, "idx-tail"),
        ] {
            let spec = format!(
                r#"{{"command":"setCellInput","sheet":0,"row":{row},"col":{col},"text":"{text}"}}"#
            );
            h.apply_edit(&spec).expect("edit");
        }

        let viewer = h.xlsx.as_ref().unwrap().sheet_ref(0).expect("sheet0");
        assert_eq!(viewer.cell_pos.synced_len(), viewer.cells.len());
        let host = h.xlsx.as_ref().unwrap().edit.as_ref().unwrap().host();
        assert_eq!(
            host.sheets[0].cell_pos.synced_len(),
            host.sheets[0].cells.len()
        );
        for (row, col, text) in [
            (0u32, 0u32, "idx-a1"),
            (50, 3, "idx-b"),
            (999, 0, "idx-tail"),
        ] {
            let cell = viewer
                .find_cell_pos(row, col)
                .map(|i| &viewer.cells[i])
                .expect("viewer cell");
            match &cell.value {
                DumpCellValue::String(s) => assert_eq!(s, text),
                other => panic!("expected string at {row}:{col}, got {other:?}"),
            }
        }
    }

    #[test]
    fn catches_set_cell_input_sync_replacing_viewer_cells_vec() {
        // Catches: sync_model_from_edit cloning the whole sheet grid after every
        // setCellInput (~150ms apply on cells_100k / cells_1m). Targeted sync must
        // keep the viewer `cells` vec identity and still surface the new value.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/generated/xlsx/cells_100k.xlsx");
        let bytes = std::fs::read(&path).expect("cells_100k");
        let mut h = Handle::new(bytes);
        h.open_document("cells_100k.xlsx").expect("open");
        h.load_editor_chrome();
        h.set_read_only(false);

        let (ptr_before, len_before) = {
            let sh = h.xlsx.as_ref().unwrap().sheet_ref(0).expect("sheet0");
            (sh.cells.as_ptr(), sh.cells.len())
        };

        h.apply_edit(r#"{"command":"setCellInput","sheet":0,"row":0,"col":0,"text":"sync-probe"}"#)
            .expect("edit");

        let sh = h.xlsx.as_ref().unwrap().sheet_ref(0).expect("sheet0");
        assert_eq!(
            sh.cells.as_ptr(),
            ptr_before,
            "viewer cells vec was replaced — full-grid sync regress"
        );
        assert_eq!(sh.cells.len(), len_before);
        let cell = sh
            .cells
            .iter()
            .find(|c| c.row == 0 && c.col == 0)
            .expect("A1");
        match &cell.value {
            DumpCellValue::String(s) => assert_eq!(s, "sync-probe"),
            other => panic!("expected string at A1, got {other:?}"),
        }
    }

    #[test]
    fn catches_grid_in_host_selection_ref_now_stays_on_host_after_enter_nudge() {
        // Catches: grid_in_host selection_ref_now stays on host after Enter nudge (keypath.click-then-type)
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/generated/xlsx/cells_1k.xlsx");
        let bytes = std::fs::read(&path).expect("cells_1k");
        let mut h = Handle::new(bytes);
        h.open_document("cells_1k.xlsx").expect("open");
        h.load_editor_chrome();
        h.set_read_only(false);
        h.apply_edit(r#"{"command":"setCellInput","sheet":0,"row":0,"col":0,"text":"warm"}"#)
            .expect("ensure_edit");
        let xlsx = h.xlsx.as_mut().expect("xlsx");
        assert!(
            xlsx.grid_in_host,
            "precondition: lazy soft-share leaves selection on the edit host"
        );
        xlsx.apply_selection_ref("G7").expect("G7");
        assert_eq!(xlsx.selection_ref_now().as_deref(), Some("G7"));
        let effect = xlsx
            .editor_key("k", false, false, false, false, false)
            .expect("type");
        assert!(matches!(effect, EditorKeyEffect::Handled));
        let effect = xlsx
            .editor_key("Enter", false, false, false, false, false)
            .expect("enter");
        assert!(matches!(effect, EditorKeyEffect::Move { row: 1, col: 0 }));
        assert_eq!(
            xlsx.selection_ref_now().as_deref(),
            Some("G8"),
            "shell selection_ref must follow host nudge after commit"
        );
    }

    #[test]
    fn catches_xlsx_apply_edit_painting_before_host_tick() {
        // Catches: applyEdit calling process_frame so every cell edit paints
        // twice (once in apply, again on the host tick's prepareFrame) — the
        // dominant edit→screen cost after sync/history were fixed (#344/#345).
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/generated/xlsx/cells_1k.xlsx");
        let bytes = std::fs::read(&path).expect("cells_1k");
        let mut h = Handle::new(bytes);
        h.open_document("cells_1k.xlsx").expect("open");
        h.load_editor_chrome();
        h.set_read_only(false);
        let _ = h.take_frame_commands();

        h.apply_edit(
            r#"{"command":"setCellInput","sheet":0,"row":0,"col":0,"text":"defer-paint"}"#,
        )
        .expect("edit");
        let mid = h.take_frame_commands();
        assert!(
            mid.is_empty(),
            "applyEdit must defer paint to prepareFrame; got {} cmd bytes",
            mid.len()
        );

        h.prepare_frame();
        let painted = h.take_frame_commands();
        assert!(
            !painted.is_empty(),
            "prepareFrame after applyEdit should emit paint commands"
        );
    }
}
