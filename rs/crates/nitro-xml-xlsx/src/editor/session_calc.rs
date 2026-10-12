// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/edit/session-calc.ts;
// changes listed in PROVENANCE.md.
//! When an edit session recalculates: incrementally after cell edits, and fully after
//! structural edits. Rust [`CalcEngine`] has no `shift_cells` / `rename_sheet`, so a
//! shift or rename hint reloads the host grid, invalidates, and recalculates everything.

use serde_json::{json, Value};

use crate::edit::host::XlsxHost;
use crate::formula::{
    CalcBookSeed, CalcCellSeed, CalcEngine, CalcSheetSeed, CalculatedCell, CellPos,
};
use crate::model::{DumpCell, DumpCellValue};

/// Row or column shift the upstream engine would apply in place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShiftAxis {
    Row,
    Col,
}

/// What the engine must follow for a step. `None` when it cannot (a full rebuild).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalcHint {
    Shift {
        sheet: u32,
        axis: ShiftAxis,
        at: u32,
        count: i32,
    },
    Rename {
        from: String,
        to: String,
    },
}

/// Apply, undo, or redo of one history step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepMode {
    Apply,
    Undo,
    Redo,
}

/// The cells and structure flag the session recorded for one step.
#[derive(Debug, Clone)]
pub struct EditorStep {
    pub structural: bool,
    pub sheet: Option<u32>,
    pub ranges: Vec<String>,
    pub hint: Option<CalcHint>,
}

/// Recalculates the workbook after edit steps and copies formula values back.
pub struct SessionCalculator {
    engine: CalcEngine,
    /// False when the session was created with `recalc: false`.
    enabled: bool,
    /// Structural edits the engine could not follow leave the dependency graph stale.
    stale_graph: bool,
    /// Cells changed in manual mode since the last calculation.
    dirty: Vec<CellPos>,
    /// Settings at the last calculation; `None` before the first.
    calculated: Option<String>,
}

impl SessionCalculator {
    /// Load a calc engine from the same package bytes as the edit host.
    pub fn open(bytes: &[u8], enabled: bool) -> Result<Self, String> {
        Ok(Self {
            engine: CalcEngine::load(bytes)?,
            enabled,
            stale_graph: false,
            dirty: Vec::new(),
            calculated: None,
        })
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Recalculates after an applied, undone, or redone step.
    pub fn after_step(&mut self, host: &mut XlsxHost, step: &EditorStep, mode: StepMode) {
        if !self.enabled {
            self.engine.discard_preparation();
            return;
        }
        let structural = step.structural;
        let hint = if structural {
            hint_for(step, mode)
        } else {
            None
        };
        if is_manual(host) {
            if hint.is_some() && !self.stale_graph && self.dirty.is_empty() {
                self.follow(host);
                return;
            }
            self.engine.discard_preparation();
            if structural {
                self.stale_graph = true;
            } else if let Some(cells) = touched_cells(step) {
                self.dirty.extend(cells);
            } else {
                self.stale_graph = true;
            }
            return;
        }
        if hint.is_some() && !self.stale_graph {
            self.follow(host);
            let cells = touched_cells(step).unwrap_or_default();
            self.engine.recalculate_from(&cells);
            self.finish(host);
            return;
        }
        if self.stale_graph || structural || touched_cells(step).is_none() {
            self.full(host);
            return;
        }
        let cells = touched_cells(step).unwrap_or_default();
        self.incremental(host, &cells);
    }

    /// Recalculates after a change applied outside the history.
    pub fn after_external(&mut self, host: &mut XlsxHost, structural: bool, cells: &[CellPos]) {
        if !self.enabled || is_manual(host) {
            self.engine.discard_preparation();
            if !self.enabled {
                return;
            }
            if structural {
                self.stale_graph = true;
            } else {
                self.dirty.extend(cells.iter().copied());
            }
            return;
        }
        if self.stale_graph || structural {
            self.full(host);
        } else {
            self.incremental(host, cells);
        }
    }

    /// Builds the dependency graph ahead of the first edit.
    pub fn prepare(&mut self, host: &XlsxHost) -> bool {
        if !self.enabled || is_manual(host) || self.stale_graph {
            return true;
        }
        self.engine.prepare(None)
    }

    /// Before a structural edit: finish a graph being prepared while it still matches.
    pub fn before_structural(&mut self, host: &XlsxHost) {
        if !self.enabled || is_manual(host) || self.stale_graph {
            return;
        }
        let _ = self.prepare(host);
    }

    /// Rebuilds the dependency graph and recalculates every formula.
    pub fn full(&mut self, host: &mut XlsxHost) {
        self.stale_graph = false;
        self.dirty.clear();
        self.reload(host);
        self.engine.invalidate();
        self.engine.recalculate_all();
        self.finish(host);
    }

    /// Calculate Now: dirty cells since the last calculation, or everything when `full`.
    pub fn now(&mut self, host: &mut XlsxHost, full: bool) {
        if !self.enabled {
            return;
        }
        if full
            || self.stale_graph
            || self.calculated.as_deref() != Some(settings_of(host).as_str())
        {
            self.full(host);
            return;
        }
        let changed = std::mem::take(&mut self.dirty);
        self.incremental(host, &changed);
    }

    /// Recalculates one sheet's formulas and their dependents.
    pub fn sheet(&mut self, host: &mut XlsxHost, sheet: u32) -> Result<(), String> {
        if host.sheets.get(sheet as usize).is_none() {
            return Err(format!("No sheet at index {sheet}"));
        }
        if self.stale_graph {
            self.full(host);
            return Ok(());
        }
        let mut cells = Vec::new();
        if let Some(grid) = host.sheets.get(sheet as usize) {
            for cell in &grid.cells {
                if cell.formula.is_some() {
                    cells.push(CellPos {
                        sheet,
                        row: cell.row,
                        col: cell.col,
                    });
                }
            }
        }
        if !cells.is_empty() {
            self.incremental(host, &cells);
        }
        Ok(())
    }

    /// Shift/rename cannot move the Rust graph. Reload and invalidate instead.
    fn follow(&mut self, host: &mut XlsxHost) {
        self.reload(host);
        self.engine.invalidate();
    }

    fn incremental(&mut self, host: &mut XlsxHost, cells: &[CellPos]) {
        for cell in cells {
            sync_host_cell(&mut self.engine, host, *cell);
        }
        if cells.is_empty() {
            self.calculated = Some(settings_of(host));
            return;
        }
        self.engine.recalculate_from(cells);
        self.finish(host);
    }

    fn reload(&mut self, host: &XlsxHost) {
        self.engine.replace_book(book_seed(host));
    }

    fn finish(&mut self, host: &mut XlsxHost) {
        let calculated = self.engine.export_calculated();
        write_results(host, &calculated);
        for warning in self.engine.warnings() {
            if !host.workbook.warnings.iter().any(|have| have == warning) {
                host.workbook.warnings.push(warning.clone());
            }
        }
        self.stale_graph = false;
        self.dirty.clear();
        self.calculated = Some(settings_of(host));
    }
}

/// What the engine must follow; `None` when undoing a shift (the graph no longer has those refs).
fn hint_for(step: &EditorStep, mode: StepMode) -> Option<CalcHint> {
    let hint = step.hint.as_ref()?;
    match hint {
        CalcHint::Rename { from, to } => {
            if mode == StepMode::Undo {
                Some(CalcHint::Rename {
                    from: to.clone(),
                    to: from.clone(),
                })
            } else {
                Some(hint.clone())
            }
        }
        CalcHint::Shift { .. } => {
            if mode == StepMode::Undo {
                None
            } else {
                Some(hint.clone())
            }
        }
    }
}

fn is_manual(host: &XlsxHost) -> bool {
    host.workbook.calc_mode.as_deref() == Some("manual")
}

/// Iteration settings and the 1904 date system. A change recalculates everything.
fn settings_of(host: &XlsxHost) -> String {
    let iter = match host.workbook.iterate {
        Some((count, delta)) => format!("{count}:{delta}"),
        None => String::new(),
    };
    format!("{iter}|{}", host.workbook.date1904)
}

/// Cells a non-structural step covers. `None` when the ranges are too large to list.
fn touched_cells(step: &EditorStep) -> Option<Vec<CellPos>> {
    let sheet = step.sheet.unwrap_or(0);
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for range in &step.ranges {
        if let Some((row, col)) = parse_rc(range) {
            if seen.insert((sheet, row, col)) {
                out.push(CellPos { sheet, row, col });
            }
            continue;
        }
        let parsed = crate::address::parse_range(range)?;
        let rows = u64::from(parsed.end.row - parsed.start.row) + 1;
        let cols = u64::from(parsed.end.col - parsed.start.col) + 1;
        if rows.saturating_mul(cols) > 4096 {
            return None;
        }
        for row in parsed.start.row..=parsed.end.row {
            for col in parsed.start.col..=parsed.end.col {
                if seen.insert((sheet, row, col)) {
                    out.push(CellPos { sheet, row, col });
                }
            }
        }
    }
    Some(out)
}

/// `R{row}C{col}` as recorded by the xlsx cell-edit scope (zero-based).
fn parse_rc(text: &str) -> Option<(u32, u32)> {
    let rest = text.strip_prefix('R')?;
    let (row, col) = rest.split_once('C')?;
    if row.is_empty() || col.is_empty() || !row.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !col.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((row.parse().ok()?, col.parse().ok()?))
}

fn book_seed(host: &XlsxHost) -> CalcBookSeed {
    CalcBookSeed {
        sheet_names: host.sheets.iter().map(|sheet| sheet.name.clone()).collect(),
        date1904: host.workbook.date1904,
        iterate: host.workbook.iterate,
        defined: host
            .workbook
            .defined_names
            .iter()
            .map(|name| (name.name.clone(), name.formula.clone(), name.local_sheet))
            .collect(),
        sheets: host
            .sheets
            .iter()
            .map(|sheet| CalcSheetSeed {
                merges: sheet.merges.clone(),
                cells: sheet
                    .cells
                    .iter()
                    .map(|cell| CalcCellSeed {
                        row: cell.row,
                        col: cell.col,
                        value: cell.value.clone(),
                        formula: cell.formula.clone(),
                        spill_anchor: cell.spill_anchor,
                        legacy: cell.legacy_formula == Some(true) && cell.array_range.is_none(),
                        array_range: cell.array_range.clone(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn sync_host_cell(engine: &mut CalcEngine, host: &XlsxHost, pos: CellPos) {
    let Some(sheet) = host.sheets.get(pos.sheet as usize) else {
        return;
    };
    let found = sheet
        .cells
        .iter()
        .find(|cell| cell.row == pos.row && cell.col == pos.col);
    let input = match found {
        Some(cell) => CalcCellSeed {
            row: pos.row,
            col: pos.col,
            value: cell.value.clone(),
            formula: cell.formula.clone(),
            spill_anchor: cell.spill_anchor,
            legacy: cell.legacy_formula == Some(true) && cell.array_range.is_none(),
            array_range: cell.array_range.clone(),
        },
        None => CalcCellSeed {
            row: pos.row,
            col: pos.col,
            value: DumpCellValue::Null,
            formula: None,
            spill_anchor: None,
            legacy: false,
            array_range: None,
        },
    };
    engine.sync_input(pos.sheet, input);
}

fn write_results(host: &mut XlsxHost, cells: &[CalculatedCell]) {
    for cell in cells {
        let Some(sheet) = host.sheets.get_mut(cell.sheet as usize) else {
            continue;
        };
        if cell.has_formula {
            if let Some(existing) = sheet
                .cells
                .iter_mut()
                .find(|item| item.row == cell.row && item.col == cell.col)
            {
                existing.value = cell.value.clone();
            }
            continue;
        }
        if cell.spill_anchor.is_none() {
            continue;
        }
        if let Some(existing) = sheet
            .cells
            .iter_mut()
            .find(|item| item.row == cell.row && item.col == cell.col)
        {
            existing.value = cell.value.clone();
            existing.formula = None;
            existing.spill_anchor = cell.spill_anchor;
            continue;
        }
        sheet.cells.push(DumpCell {
            row: cell.row,
            col: cell.col,
            value: cell.value.clone(),
            type_tag: None,
            formula: None,
            style_id: None,
            rich_text: None,
            array_range: None,
            legacy_formula: None,
            dynamic_array: None,
            spill_anchor: cell.spill_anchor,
            cell_metadata: None,
            value_metadata: None,
        });
    }
}

/// Formula cells in sheet/row/col order, for the editor-recalc differential.
pub fn formula_cells_json(host: &XlsxHost) -> Value {
    let mut cells = Vec::new();
    for (sheet_index, sheet) in host.sheets.iter().enumerate() {
        for cell in &sheet.cells {
            let Some(formula) = &cell.formula else {
                continue;
            };
            cells.push(json!({
                "sheet": sheet_index,
                "row": cell.row,
                "col": cell.col,
                "f": formula,
                "v": cell.value.to_json(),
            }));
        }
    }
    cells.sort_by(|a, b| {
        let key = |value: &Value| {
            (
                value.get("sheet").and_then(|v| v.as_u64()).unwrap_or(0),
                value.get("row").and_then(|v| v.as_u64()).unwrap_or(0),
                value.get("col").and_then(|v| v.as_u64()).unwrap_or(0),
            )
        };
        key(a).cmp(&key(b))
    });
    Value::Array(cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::cells::write_input;
    use std::fs;
    use std::path::PathBuf;

    fn fixture() -> Vec<u8> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/generated/xlsx/cells_1k.xlsx");
        fs::read(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
    }

    fn cell_value(host: &XlsxHost, row: u32, col: u32) -> DumpCellValue {
        host.sheets[0]
            .cells
            .iter()
            .find(|cell| cell.row == row && cell.col == col)
            .map(|cell| cell.value.clone())
            .unwrap_or(DumpCellValue::Null)
    }

    #[test]
    fn catches_manual_mode_leaving_a_typed_formula_null_until_calculate_now() {
        // Catches: manual calculation publishing a formula result on the edit instead of waiting for Calculate Now.
        let bytes = fixture();
        let mut host = XlsxHost::open(&bytes).expect("open");
        host.workbook.calc_mode = Some("manual".to_owned());
        let mut calc = SessionCalculator::open(&bytes, true).expect("calc");
        write_input(&mut host, 0, 0, 2, "=1+2").expect("formula");
        calc.after_step(
            &mut host,
            &EditorStep {
                structural: false,
                sheet: Some(0),
                ranges: vec!["R0C2".to_owned()],
                hint: None,
            },
            StepMode::Apply,
        );
        assert!(
            matches!(cell_value(&host, 0, 2), DumpCellValue::Null),
            "manual mode must not fill the formula value"
        );
        calc.now(&mut host, false);
        assert!(
            matches!(cell_value(&host, 0, 2), DumpCellValue::Number(n) if n == 3.0),
            "Calculate Now must publish =1+2 as 3, got {:?}",
            cell_value(&host, 0, 2)
        );
    }
}
