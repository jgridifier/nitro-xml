// SPDX-License-Identifier: MIT
//! Live xlsx edit session: `XlsxHost` + history + chrome command dispatch (#69).
//!
//! TODO(#62): crash recovery replays a loader op journal into a fresh worker.
//! This session does not write that journal.

use nitro_xml_opc::edit::{
    ChangeKind, CommandRegistry, EditError, EditHistory, EditHost, EditScope, EditSession,
};
use serde_json::Value;

use crate::address::{format_address, format_range};
use crate::edit::{
    fill_area, register_cell_commands, register_fill_commands, register_shift_commands,
    register_structure_commands, XlsxHost,
};
use crate::model::CellAddress;
use crate::write::save_xlsx;

use super::cell_editor::CellEditor;
use super::commands::{map_chrome_command, MappedCommand, SelectionTarget};
use super::formula_bar::FormulaBarView;
use super::input_text::cell_input_text;

/// Result of a ribbon, shortcut, or headless chrome command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeEffect {
    Applied { structure: bool },
    Save,
    Unmapped,
}

/// Open workbook plus the formula-bar buffer.
pub struct EditorSession {
    session: EditSession<XlsxHost, EditHistory>,
    registry: CommandRegistry,
    cell: CellEditor,
}

impl EditorSession {
    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        let host = XlsxHost::open(bytes)?;
        Ok(Self::from_host(host))
    }

    pub fn from_host(host: XlsxHost) -> Self {
        let mut registry = CommandRegistry::new();
        register_cell_commands(&mut registry);
        register_fill_commands(&mut registry);
        register_structure_commands(&mut registry);
        register_shift_commands(&mut registry);
        Self {
            session: EditSession::new(host, EditHistory::new()),
            registry,
            cell: CellEditor::default(),
        }
    }

    pub fn host(&self) -> &XlsxHost {
        &self.session.host
    }

    pub fn host_mut(&mut self) -> &mut XlsxHost {
        &mut self.session.host
    }

    pub fn cell(&self) -> &CellEditor {
        &self.cell
    }

    pub fn cell_mut(&mut self) -> &mut CellEditor {
        &mut self.cell
    }

    pub fn stale_cells(&self, sheet: u32) -> Vec<(u32, u32)> {
        self.session.host.stale_cells(sheet)
    }

    pub fn input_text(&self, sheet: u32, row: u32, col: u32) -> String {
        let Ok(ws) = self.session.host.sheet(sheet) else {
            return String::new();
        };
        cell_input_text(&self.session.host.workbook, ws, row, col)
    }

    pub fn formula_bar(&self, target: &SelectionTarget, read_only: bool) -> FormulaBarView {
        let (text, caret, editing) = if self.cell.is_editing() {
            (self.cell.text().to_owned(), self.cell.focus(), true)
        } else {
            (
                self.input_text(target.sheet, target.row, target.col),
                0,
                false,
            )
        };
        FormulaBarView {
            read_only,
            name: if target.range.is_empty() {
                format_address(&CellAddress {
                    row: target.row,
                    col: target.col,
                })
            } else {
                target.range.clone()
            },
            text,
            caret,
            editing,
            expanded: self.cell.expanded(),
        }
    }

    pub fn save(&self, original: &[u8]) -> Result<Vec<u8>, String> {
        save_xlsx(&self.session.host, original)
    }

    /// Ribbon, shortcut, and headless chrome ids.
    pub fn dispatch_chrome(
        &mut self,
        id: &str,
        target: &SelectionTarget,
    ) -> Result<ChromeEffect, EditError> {
        if self.cell.is_editing() && !matches!(id, "edit.undo" | "edit.redo" | "file.save") {
            self.commit_edit()?;
        }
        match map_chrome_command(id, target, &self.session.host) {
            MappedCommand::Save => Ok(ChromeEffect::Save),
            MappedCommand::Undo => {
                self.session.undo();
                self.cell.cancel();
                Ok(ChromeEffect::Applied { structure: true })
            }
            MappedCommand::Redo => {
                self.session.redo();
                self.cell.cancel();
                Ok(ChromeEffect::Applied { structure: true })
            }
            MappedCommand::Unmapped => Ok(ChromeEffect::Unmapped),
            MappedCommand::Op {
                id: op,
                args,
                structure,
                label,
            } => {
                self.apply_op(op, args, label, structure)?;
                Ok(ChromeEffect::Applied { structure })
            }
        }
    }

    /// Direct #66/#67 op (`setCellInput`, `applyStyle`, `addSheet`, …).
    pub fn apply_raw(&mut self, id: &str, args: Value) -> Result<(), EditError> {
        let structure = matches!(
            id,
            "addSheet"
                | "renameSheet"
                | "deleteSheet"
                | "insertRows"
                | "deleteRows"
                | "insertColumns"
                | "deleteColumns"
        );
        self.apply_op(id, args, id, structure)
    }

    pub fn begin_edit(
        &mut self,
        target: &SelectionTarget,
        seed: Option<String>,
    ) -> Result<(), EditError> {
        if self.cell.is_editing()
            && (self.cell.sheet() != target.sheet
                || self.cell.row() != target.row
                || self.cell.col() != target.col)
        {
            self.commit_edit()?;
        }
        if self.cell.is_editing() && seed.is_none() {
            return Ok(());
        }
        let text = match seed {
            Some(s) => s,
            None => self.input_text(target.sheet, target.row, target.col),
        };
        self.cell.begin(target.sheet, target.row, target.col, text);
        Ok(())
    }

    pub fn cancel_edit(&mut self) {
        self.cell.cancel();
    }

    pub fn commit_edit(&mut self) -> Result<bool, EditError> {
        let Some((sheet, row, col, text)) = self.cell.take_commit() else {
            return Ok(false);
        };
        self.apply_op(
            "setCellInput",
            serde_json::json!({
                "sheet": sheet,
                "row": row,
                "col": col,
                "text": text,
            }),
            "setCellInput",
            false,
        )?;
        Ok(true)
    }

    /// `fx`: start a formula, or prefix `=` onto the buffer already being edited.
    pub fn insert_function(&mut self, target: &SelectionTarget) -> Result<(), EditError> {
        if !self.cell.is_editing() {
            return self.begin_edit(target, Some("=".into()));
        }
        if !self.cell.text().starts_with('=') {
            let text = format!("={}", self.cell.text());
            let end = text.len();
            self.cell.set_buffer(text, end, end);
        }
        Ok(())
    }

    fn apply_op(
        &mut self,
        id: &str,
        args: Value,
        label: &str,
        structure: bool,
    ) -> Result<(), EditError> {
        let (kind, scopes) = op_meta(id, &args, structure);
        let registry = &self.registry;
        self.session
            .run(label, kind, scopes, |host| {
                registry.run(id, host.as_any(), &args).map(|_| ())
            })
            .map(|_| ())
    }
}

fn op_meta(id: &str, args: &Value, structure: bool) -> (ChangeKind, Vec<EditScope>) {
    let sheet = args.get("sheet").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if structure || matches!(id, "addSheet" | "renameSheet" | "deleteSheet") {
        return (ChangeKind::Sheets, vec![EditScope::Workbook]);
    }
    if id == "fillRange" {
        let ranges = fill_area(args)
            .map(|r| vec![format_range(&r)])
            .unwrap_or_default();
        return (ChangeKind::Cells, vec![EditScope::Cells { sheet, ranges }]);
    }
    let kind = if id == "applyStyle" {
        ChangeKind::Format
    } else {
        ChangeKind::Cells
    };
    // The history snapshot only covers the ranges named here; an empty scope captured nothing,
    // so Undo of a typed or pasted cell did nothing.
    let num = |k: &str| args.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
    let ranges = match id {
        "setCellInput" | "setCellValue" => vec![format!("R{}C{}", num("row"), num("col"))],
        "applyStyle" | "applyCellStyle" => args
            .get("range")
            .and_then(|v| v.as_str())
            .map(|r| vec![r.to_owned()])
            .unwrap_or_default(),
        "setCellBlock" => {
            let rows = args.get("rows").and_then(|v| v.as_array());
            let h = rows.map_or(0, |r| r.len() as u64);
            let w = rows
                .and_then(|r| {
                    r.iter()
                        .map(|l| l.as_array().map_or(0, |a| a.len() as u64))
                        .max()
                })
                .unwrap_or(0);
            if h == 0 || w == 0 {
                Vec::new()
            } else if h * w > 4_096 {
                // Too many cells for per-cell absences: fall back to the whole-workbook snapshot.
                return (kind, vec![EditScope::Workbook]);
            } else {
                let (r0, c0) = (num("row") as u32, num("col") as u32);
                let a = CellAddress { row: r0, col: c0 };
                let b = CellAddress {
                    row: r0 + h as u32 - 1,
                    col: c0 + w as u32 - 1,
                };
                vec![format!("{}:{}", format_address(&a), format_address(&b))]
            }
        }
        _ => Vec::new(),
    };
    (kind, vec![EditScope::Cells { sheet, ranges }])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{paint_grid_frame_with_hints, EditorPaintHints, GridPaintViewport};
    use crate::styles::style_at;
    use serde_json::json;
    use std::path::Path;

    fn cells_1k() -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/generated/xlsx/cells_1k.xlsx");
        std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    }

    #[test]
    fn commit_marks_formula_dependents_and_paint_class() {
        // Catches: committing the cell editor must stale formula dependents and paint xg-stale.
        let bytes = cells_1k();
        let mut session = EditorSession::open(&bytes).expect("open");
        session
            .apply_raw(
                "setCellInput",
                serde_json::json!({"sheet": 0, "row": 0, "col": 2, "text": "=A1+1"}),
            )
            .expect("formula");
        let target = SelectionTarget::cell(0, 0, 0);
        session
            .begin_edit(&target, Some("99".into()))
            .expect("begin");
        assert!(session.commit_edit().expect("commit"));
        let stale = session.stale_cells(0);
        assert!(
            stale.contains(&(0, 2)),
            "C1 should be stale after editing A1, got {stale:?}"
        );
        let host = session.host();
        let sheet = host.sheet(0).expect("sheet");
        let sheets: Vec<Option<_>> = host.sheets.iter().cloned().map(Some).collect();
        let viewport = GridPaintViewport {
            scroll_x: 0,
            scroll_y: 0,
            width: 800,
            height: 600,
            frozen_rows: 0,
            frozen_cols: 0,
            zoom_pct: None,
        };
        let hints = EditorPaintHints {
            stale: stale.clone(),
            edit: None,
            ..EditorPaintHints::default()
        };
        let mut strings = Vec::new();
        let mut intern = |s: &str| {
            strings.push(s.to_owned());
            (strings.len() - 1) as u32
        };
        let _ = paint_grid_frame_with_hints(
            &mut intern,
            &host.workbook,
            sheet,
            0,
            &sheets,
            &viewport,
            &hints,
        );
        assert!(
            strings.iter().any(|s| s.contains("xg-stale")),
            "paint must mark stale formula cells, classes: {strings:?}"
        );
    }

    #[test]
    fn home_bold_applies_style_and_undoes() {
        // Catches: ribbon home.bold must run applyStyle on the selection, not stay a chrome stub.
        let bytes = cells_1k();
        let mut session = EditorSession::open(&bytes).expect("open");
        let target = SelectionTarget::cell(0, 0, 0);
        let before = {
            let host = session.host();
            let id = host.sheet(0).ok().and_then(|sh| {
                sh.cells
                    .iter()
                    .find(|c| c.row == 0 && c.col == 0)
                    .and_then(|c| c.style_id)
            });
            style_at(&host.workbook.styles, id).font.bold
        };
        let effect = session.dispatch_chrome("home.bold", &target).expect("bold");
        assert_eq!(effect, ChromeEffect::Applied { structure: false });
        let after = {
            let host = session.host();
            let id = host.sheet(0).ok().and_then(|sh| {
                sh.cells
                    .iter()
                    .find(|c| c.row == 0 && c.col == 0)
                    .and_then(|c| c.style_id)
            });
            style_at(&host.workbook.styles, id).font.bold
        };
        assert_ne!(before == Some(true), after == Some(true));
        session.dispatch_chrome("edit.undo", &target).expect("undo");
        let undone = {
            let host = session.host();
            let id = host.sheet(0).ok().and_then(|sh| {
                sh.cells
                    .iter()
                    .find(|c| c.row == 0 && c.col == 0)
                    .and_then(|c| c.style_id)
            });
            style_at(&host.workbook.styles, id).font.bold
        };
        assert_eq!(undone == Some(true), before == Some(true));
    }

    #[test]
    fn ribbon_insert_delete_rows_and_columns_span_the_selection_and_undo() {
        // Catches: ribbon Insert/Delete Sheet Rows/Columns left unmapped (dead button) or acting on one line when several are selected.
        let bytes = cells_1k();
        let mut session = EditorSession::open(&bytes).expect("open");
        let at = |s: &EditorSession, r: u32, c: u32| s.input_text(0, r, c);
        let (a1, a3, c1) = (at(&session, 0, 0), at(&session, 2, 0), at(&session, 0, 2));
        let rows = SelectionTarget {
            sheet: 0,
            row: 0,
            col: 0,
            range: "A1:B2".into(),
        };
        let eff = session
            .dispatch_chrome("cells.insert-rows", &rows)
            .expect("insert");
        assert_eq!(eff, ChromeEffect::Applied { structure: true });
        assert_eq!(at(&session, 2, 0), a1, "two rows inserted above A1");
        assert_eq!(at(&session, 0, 0), "");
        session
            .dispatch_chrome("cells.delete-rows", &rows)
            .expect("delete");
        assert_eq!(at(&session, 0, 0), a1);
        assert_eq!(at(&session, 2, 0), a3);
        let cols = SelectionTarget {
            sheet: 0,
            row: 0,
            col: 0,
            range: "A1:B2".into(),
        };
        session
            .dispatch_chrome("cells.insert-columns", &cols)
            .expect("insert col");
        assert_eq!(at(&session, 0, 4), c1, "two columns inserted left of A");
        session
            .dispatch_chrome("cells.delete-columns", &cols)
            .expect("delete col");
        assert_eq!(at(&session, 0, 2), c1);
        session.dispatch_chrome("edit.undo", &cols).expect("undo");
        // Undo re-serialises numbers ("3.0" -> "3"), so compare values, not text.
        let num = |t: String| t.parse::<f64>().ok();
        assert_eq!(
            num(at(&session, 0, 4)),
            num(c1),
            "undo restores the inserted state"
        );
    }

    #[test]
    fn fill_range_is_one_undo_step_and_extends_the_series() {
        // Catches: a fill-handle drag leaving partial cells on undo, or taking several undo steps.
        let bytes = cells_1k();
        let mut session = EditorSession::open(&bytes).expect("open");
        let at = |s: &EditorSession, r: u32| {
            let t = s.input_text(0, r, 20);
            t.strip_suffix(".0").map_or(t.clone(), str::to_owned)
        };
        session
            .apply_raw(
                "setCellInput",
                json!({"sheet":0,"row":0,"col":20,"text":"1"}),
            )
            .unwrap();
        session
            .apply_raw(
                "setCellInput",
                json!({"sheet":0,"row":1,"col":20,"text":"3"}),
            )
            .unwrap();
        session
            .apply_raw(
                "fillRange",
                json!({"sheet":0,"range":"U1:U2","direction":"down","count":2}),
            )
            .unwrap();
        assert_eq!((at(&session, 2), at(&session, 3)), ("5".into(), "7".into()));
        let target = SelectionTarget {
            sheet: 0,
            row: 0,
            col: 20,
            range: "U1".into(),
        };
        session.dispatch_chrome("edit.undo", &target).unwrap();
        assert_eq!((at(&session, 2), at(&session, 3)), ("".into(), "".into()));
        assert_eq!(at(&session, 1), "3");
    }
}
