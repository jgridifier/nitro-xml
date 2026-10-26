// SPDX-License-Identifier: MIT
//! Xlsx cell value, number-format, style, and structure edits on the shared command registry (#66, #67).
//! Undo/redo uses the shared [`EditHistory`] (#64).

pub mod cells;
pub mod fill;
pub mod host;
pub mod shift;
pub mod structure;
pub mod style_patch;

pub use cells::register_cell_commands;
pub use fill::register_fill_commands;
pub use host::XlsxHost;
pub use shift::register_shift_commands;
pub use structure::register_structure_commands;

use nitro_xml_opc::edit::{
    parse_edit_script, replay_script, CommandRegistry, EditError, EditHistory, EditScript,
    EditSession, HistoryStep, Op, ReplayReport,
};
use serde_json::{json, Value};

use crate::editor::{
    formula_cells_json, CalcHint, EditorStep, SessionCalculator, ShiftAxis, StepMode,
};

/// Replay an xlsx edit script against loaded package bytes; returns the report and model snapshot.
///
/// Recalculates so formula values match `parity/edit/xlsx_model_dump.mjs`. Edit-parity save
/// uses [`replay_xlsx_session`], which does not.
pub fn replay_cell_script(
    bytes: &[u8],
    script_json: &str,
) -> Result<(ReplayReport, Value), EditError> {
    let (report, session) = replay_xlsx_session_with(bytes, script_json, true)?;
    let snapshot = session.host.model_snapshot();
    Ok((report, snapshot))
}

/// Open a session, register xlsx commands, and replay the script (history retained for undo/redo).
///
/// Calculation stays off, matching TS `createEditSession(workbook, { recalc: false })` in
/// `parity/edit/apply.mjs`. [`replay_formula_checkpoints`] takes its own flag (`xlsx_editor_recalc`
/// passes `true` unless `--no-recalc`). [`undo_parity_checkpoints`] stays on so model
/// checkpoints match the oracle when `XLSX_RECALC=1`.
pub fn replay_xlsx_session(
    bytes: &[u8],
    script_json: &str,
) -> Result<(ReplayReport, EditSession<XlsxHost, EditHistory>), EditError> {
    replay_xlsx_session_with(bytes, script_json, false)
}

fn replay_xlsx_session_with(
    bytes: &[u8],
    script_json: &str,
    recalc: bool,
) -> Result<(ReplayReport, EditSession<XlsxHost, EditHistory>), EditError> {
    let script = parse_edit_script(script_json)?;
    let (mut session, registry, mut calc) = open_xlsx_edit(bytes, &script, recalc)?;
    let report = replay_ops_with_calc(&script, &registry, &mut session, &mut calc)?;
    Ok((report, session))
}

/// Formula-cell checkpoints after every do, undo, and redo.
///
/// `recalc: false` leaves cached values stale (the editor-recalc negative control).
/// `xlsx_editor_recalc` passes `true` unless `--no-recalc`.
pub fn replay_formula_checkpoints(
    bytes: &[u8],
    script_json: &str,
    recalc: bool,
) -> Result<Value, EditError> {
    let script = parse_edit_script(script_json)?;
    let (mut session, registry, mut calc) = open_xlsx_edit(bytes, &script, recalc)?;
    let mut checkpoints = Vec::new();
    let mut push = |phase: &str, session: &EditSession<XlsxHost, EditHistory>| {
        checkpoints.push(json!({
            "phase": phase,
            "formulas": formula_cells_json(&session.host),
        }));
    };
    push("start", &session);
    for op in &script.ops {
        let report = apply_op_with_calc(&script, op, &registry, &mut session, &mut calc)?;
        if report.status != "ok" {
            return Err(EditError::Other {
                message: format!("op {} status {}", op.command_id(), report.status),
            });
        }
        push("do", &session);
    }
    while session.can_undo() {
        undo_with_calc(&mut session, &mut calc);
        push("undo", &session);
    }
    while session.can_redo() {
        redo_with_calc(&mut session, &mut calc);
        push("redo", &session);
    }
    Ok(json!({ "checkpoints": checkpoints }))
}

fn open_xlsx_edit(
    bytes: &[u8],
    script: &EditScript,
    recalc: bool,
) -> Result<
    (
        EditSession<XlsxHost, EditHistory>,
        CommandRegistry,
        SessionCalculator,
    ),
    EditError,
> {
    if script.format != nitro_xml_opc::edit::EditFormat::Xlsx {
        return Err(EditError::Other {
            message: format!("expected xlsx script, got {}", script.format.as_str()),
        });
    }
    let host = XlsxHost::open(bytes).map_err(|e| EditError::Other { message: e })?;
    let calc =
        SessionCalculator::open(bytes, recalc).map_err(|e| EditError::Other { message: e })?;
    let mut registry = CommandRegistry::with_schema_stubs();
    register_cell_commands(&mut registry);
    register_structure_commands(&mut registry);
    register_shift_commands(&mut registry);
    Ok((EditSession::new(host, EditHistory::new()), registry, calc))
}

fn replay_ops_with_calc(
    script: &EditScript,
    registry: &CommandRegistry,
    session: &mut EditSession<XlsxHost, EditHistory>,
    calc: &mut SessionCalculator,
) -> Result<ReplayReport, EditError> {
    let mut ops = Vec::new();
    let mut status = "ok".to_owned();
    for op in &script.ops {
        let part = apply_op_with_calc(script, op, registry, session, calc)?;
        let part_status = part.status.clone();
        ops.extend(part.ops);
        if part_status != "ok" {
            status = part_status;
            break;
        }
    }
    if ops.is_empty() {
        status = "error".to_owned();
    }
    Ok(ReplayReport {
        id: script.id.clone(),
        format: script.format.as_str().to_owned(),
        status,
        ops,
        history_commits: session.history.committed().len(),
    })
}

fn apply_op_with_calc(
    script: &EditScript,
    op: &Op,
    registry: &CommandRegistry,
    session: &mut EditSession<XlsxHost, EditHistory>,
    calc: &mut SessionCalculator,
) -> Result<ReplayReport, EditError> {
    if op_is_structural(op) {
        calc.before_structural(&session.host);
    }
    let hint = hint_from_op(op, &session.host);
    let one = EditScript {
        id: script.id.clone(),
        format: script.format,
        fixture: script.fixture.clone(),
        description: None,
        ops: vec![op.clone()],
    };
    let mut report = replay_script(&one, registry, session);
    report.history_commits = session.history.committed().len();
    if report.status == "ok" {
        if let Some(step) = session.history.done().last() {
            let mut editor_step = editor_step_from_history(step);
            editor_step.hint = hint;
            calc.after_step(&mut session.host, &editor_step, StepMode::Apply);
        }
    }
    Ok(report)
}

fn undo_with_calc(session: &mut EditSession<XlsxHost, EditHistory>, calc: &mut SessionCalculator) {
    let step = session.history.done().last().cloned();
    session.undo();
    if let Some(step) = step {
        calc.after_step(
            &mut session.host,
            &editor_step_from_history(&step),
            StepMode::Undo,
        );
    }
}

fn redo_with_calc(session: &mut EditSession<XlsxHost, EditHistory>, calc: &mut SessionCalculator) {
    let step = session.history.undone().last().cloned();
    session.redo();
    if let Some(step) = step {
        calc.after_step(
            &mut session.host,
            &editor_step_from_history(&step),
            StepMode::Redo,
        );
    }
}

fn editor_step_from_history(step: &HistoryStep) -> EditorStep {
    EditorStep {
        structural: step.structural,
        sheet: step.change.sheet,
        ranges: step.change.ranges.clone(),
        hint: None,
    }
}

fn op_is_structural(op: &Op) -> bool {
    matches!(
        op,
        Op::AddSheet { .. }
            | Op::RenameSheet { .. }
            | Op::DeleteSheet { .. }
            | Op::InsertRows { .. }
            | Op::DeleteRows { .. }
            | Op::InsertColumns { .. }
            | Op::DeleteColumns { .. }
            | Op::Batch { .. }
    )
}

fn hint_from_op(op: &Op, host: &XlsxHost) -> Option<CalcHint> {
    match op {
        Op::InsertRows { sheet, at, count } => Some(CalcHint::Shift {
            sheet: *sheet,
            axis: ShiftAxis::Row,
            at: *at,
            count: *count as i32,
        }),
        Op::DeleteRows { sheet, at, count } => Some(CalcHint::Shift {
            sheet: *sheet,
            axis: ShiftAxis::Row,
            at: *at,
            count: -(*count as i32),
        }),
        Op::InsertColumns { sheet, at, count } => Some(CalcHint::Shift {
            sheet: *sheet,
            axis: ShiftAxis::Col,
            at: *at,
            count: *count as i32,
        }),
        Op::DeleteColumns { sheet, at, count } => Some(CalcHint::Shift {
            sheet: *sheet,
            axis: ShiftAxis::Col,
            at: *at,
            count: -(*count as i32),
        }),
        Op::RenameSheet { index, name } => {
            let from = host.sheets.get(*index as usize)?.name.clone();
            Some(CalcHint::Rename {
                from,
                to: name.clone(),
            })
        }
        _ => None,
    }
}

/// Do → undo-all → redo-all checkpoints matching `parity/undo` phases (snapshots only).
///
/// Recalc stays on. The live TS compare sets `XLSX_RECALC=1`; the undo-parity save gate
/// leaves that unset, and edit-parity save goes through [`replay_xlsx_session`] (off).
pub fn undo_parity_checkpoints(bytes: &[u8], script_json: &str) -> Result<Vec<Value>, EditError> {
    let script: EditScript = parse_edit_script(script_json)?;
    if script.format != nitro_xml_opc::edit::EditFormat::Xlsx {
        return Err(EditError::Other {
            message: format!("expected xlsx script, got {}", script.format.as_str()),
        });
    }

    let (mut session, registry, mut calc) = open_xlsx_edit(bytes, &script, true)?;

    let mut checkpoints = Vec::new();
    let mut step: u32 = 0;
    let mut push = |phase: &str, label: &str, session: &EditSession<XlsxHost, EditHistory>| {
        checkpoints.push(serde_json::json!({
            "phase": phase,
            "step": step,
            "label": label,
            "snapshot": session.host.undo_parity_snapshot(),
        }));
        step += 1;
    };

    push("start", "open", &session);

    for op in &script.ops {
        let report = apply_op_with_calc(&script, op, &registry, &mut session, &mut calc)?;
        if report.status != "ok" {
            return Err(EditError::Other {
                message: format!("op {} status {}", op.command_id(), report.status),
            });
        }
        let label = session
            .history
            .undo_label()
            .unwrap_or(op.command_id())
            .to_owned();
        push("do", &label, &session);
    }

    while session.can_undo() {
        let label = session.undo_label().unwrap_or("undo").to_owned();
        undo_with_calc(&mut session, &mut calc);
        push("undo", &label, &session);
    }

    while session.can_redo() {
        let label = session.redo_label().unwrap_or("redo").to_owned();
        redo_with_calc(&mut session, &mut calc);
        push("redo", &label, &session);
    }

    Ok(checkpoints)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }

    fn edits_xlsx() -> PathBuf {
        repo_root().join("edits/xlsx")
    }

    fn fixture_path(id: &str) -> PathBuf {
        let manifest = repo_root().join("fixtures/manifest.json");
        let text = fs::read_to_string(&manifest).expect("manifest");
        let v: Value = serde_json::from_str(&text).expect("manifest json");
        let fixtures = v["fixtures"].as_array().expect("fixtures");
        let row = fixtures
            .iter()
            .find(|f| f["id"].as_str() == Some(id))
            .unwrap_or_else(|| panic!("fixture {id}"));
        repo_root().join(row["path"].as_str().expect("path"))
    }

    fn bun_bin() -> String {
        std::env::var("BUN_BIN").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/ubuntu".into());
            format!("{home}/.bun/bin/bun")
        })
    }

    const XLSX_SCRIPTS: &[&str] = &[
        "xlsx-batch-cells",
        "xlsx-bidi-cell",
        "xlsx-merges-edit-a1",
        "xlsx-merges-style",
        "xlsx-set-cell-formula",
        "xlsx-set-cell-number",
        "xlsx-set-cell-text",
        "xlsx-set-cell-value-bool",
        "xlsx-set-cell-value-number",
        "xlsx-set-cell-value-string",
        "xlsx-style-align",
        "xlsx-style-bold",
        "xlsx-style-fill",
        "xlsx-style-italic-color",
        "xlsx-style-numfmt",
        "xlsx-style-underline-strike",
        "xlsx-value-and-style",
        "xlsx-add-sheet",
        "xlsx-add-delete-sheet",
        "xlsx-add-rename-sheet",
        "xlsx-rename-sheet",
        "xlsx-multi-sheet-ops",
        "xlsx-insert-row-formula",
        "xlsx-delete-row-ref",
        "xlsx-insert-col-formula",
        "xlsx-delete-col-ref",
        "xlsx-insert-row-area",
        "xlsx-delete-row-area",
    ];

    fn ts_model_dump(fixture: &Path, script_json: &str) -> Option<Value> {
        let bun = bun_bin();
        if Command::new(&bun).arg("--version").output().is_err() {
            eprintln!("skip TS dump: bun not available ({bun})");
            return None;
        }
        let dump_mjs = repo_root().join("parity/edit/xlsx_model_dump.mjs");
        if !dump_mjs.is_file() {
            eprintln!("skip TS dump: missing {}", dump_mjs.display());
            return None;
        }
        let dir = std::env::temp_dir().join(format!("xlsx-edit-dump-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let script_path = dir.join("script.json");
        let out_path = dir.join("out.json");
        fs::write(&script_path, script_json).ok()?;
        let status = Command::new(&bun)
            .arg(dump_mjs.as_os_str())
            .arg(fixture.as_os_str())
            .arg(script_path.as_os_str())
            .arg(out_path.as_os_str())
            .status()
            .ok()?;
        if !status.success() {
            eprintln!("TS dump failed: {status}");
            return None;
        }
        let text = fs::read_to_string(&out_path).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn ts_undo_checkpoints(fixture: &Path, script_json: &str) -> Option<Vec<Value>> {
        let bun = bun_bin();
        if Command::new(&bun).arg("--version").output().is_err() {
            return None;
        }
        let helper = repo_root().join("parity/undo/rust_oracle.mjs");
        if !helper.is_file() {
            eprintln!("skip undo oracle: missing {}", helper.display());
            return None;
        }
        let dir = std::env::temp_dir().join(format!("xlsx-undo-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let script_path = dir.join("script.json");
        let out_path = dir.join("out.json");
        fs::write(&script_path, script_json).ok()?;
        let status = Command::new(&bun)
            .env("XLSX_RECALC", "1")
            .arg(helper.as_os_str())
            .arg(fixture.as_os_str())
            .arg(script_path.as_os_str())
            .arg(out_path.as_os_str())
            .status()
            .ok()?;
        if !status.success() {
            eprintln!("TS undo oracle failed: {status}");
            return None;
        }
        let text = fs::read_to_string(&out_path).ok()?;
        let v: Value = serde_json::from_str(&text).ok()?;
        v.get("checkpoints").and_then(|c| c.as_array()).cloned()
    }

    #[test]
    fn catches_xlsx_edit_scripts_match_ts_model_including_structure() {
        // Catches: Rust xlsx cell/style/structure edits drifting from upstream TS model state (#66/#67).
        let mut ok = 0usize;
        for id in XLSX_SCRIPTS {
            let path = edits_xlsx().join(format!("{id}.json"));
            let script_json = fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!("read {}: {e}", path.display());
            });
            let script: Value = serde_json::from_str(&script_json).expect("script json");
            let fixture_id = script["fixture"].as_str().expect("fixture");
            let fixture = fixture_path(fixture_id);
            let bytes = fs::read(&fixture).unwrap_or_else(|e| {
                panic!("read fixture {}: {e}", fixture.display());
            });
            let (report, rust_snap) = replay_cell_script(&bytes, &script_json)
                .unwrap_or_else(|e| panic!("{id} replay: {e}"));
            assert_eq!(report.status, "ok", "{id} report={report:?}");
            ok += 1;

            if let Some(ts_snap) = ts_model_dump(&fixture, &script_json) {
                assert_eq!(
                    rust_snap,
                    ts_snap,
                    "{id} model snapshot mismatch\nrust={}\nts={}",
                    serde_json::to_string_pretty(&rust_snap).unwrap(),
                    serde_json::to_string_pretty(&ts_snap).unwrap()
                );
            }
        }
        assert_eq!(ok, 28, "expected 28 xlsx edit scripts");
    }

    #[test]
    fn catches_xlsx_undo_parity_checkpoints_match_ts() {
        // Catches: Rust undo/redo drifting from upstream TS model at any do/undo/redo step (#64).
        // Local undo invariants always run (rust.sh has no bun). TS compare runs when the
        // live oracle is available (undo_parity.sh / local bun).
        let mut local_ok = 0usize;
        let mut ts_ok = 0usize;
        let mut ts_attempts = 0usize;
        for id in XLSX_SCRIPTS {
            let path = edits_xlsx().join(format!("{id}.json"));
            let script_json = fs::read_to_string(&path).unwrap();
            let script: Value = serde_json::from_str(&script_json).unwrap();
            let fixture_id = script["fixture"].as_str().unwrap();
            let fixture = fixture_path(fixture_id);
            let bytes = fs::read(&fixture).unwrap();

            let rust_cps = undo_parity_checkpoints(&bytes, &script_json)
                .unwrap_or_else(|e| panic!("{id} rust undo: {e}"));

            let start = rust_cps
                .iter()
                .find(|c| c.get("phase").and_then(|p| p.as_str()) == Some("start"))
                .and_then(|c| c.get("snapshot"))
                .expect("start snapshot");
            let after_dos = rust_cps
                .iter()
                .rev()
                .find(|c| c.get("phase").and_then(|p| p.as_str()) == Some("do"))
                .and_then(|c| c.get("snapshot"))
                .expect("after-dos snapshot");
            let after_undo = rust_cps
                .iter()
                .rev()
                .find(|c| c.get("phase").and_then(|p| p.as_str()) == Some("undo"))
                .and_then(|c| c.get("snapshot"))
                .unwrap_or(start);
            let after_redo = rust_cps
                .iter()
                .rev()
                .find(|c| c.get("phase").and_then(|p| p.as_str()) == Some("redo"))
                .and_then(|c| c.get("snapshot"))
                .unwrap_or(after_dos);
            assert_eq!(after_undo, start, "{id} undo-all must restore start");
            assert_eq!(
                after_redo, after_dos,
                "{id} redo-all must restore after-dos"
            );
            local_ok += 1;

            let Some(ts_cps) = ts_undo_checkpoints(&fixture, &script_json) else {
                eprintln!("skip TS compare for {id}: bun/oracle not available");
                continue;
            };
            ts_attempts += 1;
            assert_eq!(
                rust_cps.len(),
                ts_cps.len(),
                "{id} checkpoint count rust={} ts={}",
                rust_cps.len(),
                ts_cps.len()
            );
            for (i, (r, t)) in rust_cps.iter().zip(ts_cps.iter()).enumerate() {
                let r_phase = r.get("phase").and_then(|x| x.as_str());
                let t_phase = t.get("phase").and_then(|x| x.as_str());
                assert_eq!(r_phase, t_phase, "{id} checkpoint[{i}] phase");
                let r_snap = r.get("snapshot").expect("rust snapshot");
                let t_snap = t.get("snapshot").expect("ts snapshot");
                assert_eq!(
                    r_snap,
                    t_snap,
                    "{id} checkpoint[{i}] ({r_phase:?}) snapshot mismatch\nrust={}\nts={}",
                    serde_json::to_string_pretty(r_snap).unwrap(),
                    serde_json::to_string_pretty(t_snap).unwrap()
                );
            }
            ts_ok += 1;
        }
        assert_eq!(local_ok, 28, "xlsx local undo-parity expected 28/28");
        if ts_attempts > 0 {
            assert_eq!(
                ts_ok, 28,
                "xlsx TS undo-parity expected 28/28 (got {ts_ok}/{ts_attempts})"
            );
            eprintln!("xlsx undo-parity: {ts_ok}/28 (TS match)");
        } else {
            eprintln!("xlsx undo-parity: {local_ok}/28 (local undo/redo only; no bun oracle)");
        }
    }

    #[test]
    fn catches_cells_1m_set_cell_input_history_stays_scoped() {
        // Catches: full-workbook JSON undo snaps on ~1M cells (~64MiB×2/edit) OOM the
        // WASM 4GiB ceiling by ~edit 22. Cell-scoped history must stay compact and
        // survive far more than that without cloning the grid.
        //
        // Prefer the real cells_1m fixture when present; CI rust jobs do not ship that
        // generated xlsx, so inflate a tiny package to 1_000_000 DumpCells in memory
        // (same grid size as the trap — not a smaller dodge).
        use crate::model::{DumpCell, DumpCellValue};

        const GRID: usize = 1_000_000;
        const EDITS: usize = 40;

        let host = {
            let fixture = fixture_path("gen-xlsx-cells-1m");
            if fixture.is_file() {
                let bytes = fs::read(&fixture).expect("cells_1m");
                XlsxHost::open(&bytes).expect("open cells_1m")
            } else {
                let tiny = fixture_path("gen-xlsx-cells-1k");
                let bytes = fs::read(&tiny).expect("cells_1k shell");
                let mut h = XlsxHost::open(&bytes).expect("open cells_1k");
                let mut cells = Vec::with_capacity(GRID);
                for i in 0..GRID {
                    cells.push(DumpCell {
                        row: (i / 50) as u32,
                        col: (i % 50) as u32,
                        value: DumpCellValue::Number(i as f64),
                        type_tag: None,
                        formula: None,
                        style_id: None,
                        rich_text: None,
                        array_range: None,
                        legacy_formula: None,
                        dynamic_array: None,
                        spill_anchor: None,
                        cell_metadata: None,
                        value_metadata: None,
                    });
                }
                h.sheets[0].replace_cells(cells);
                h
            }
        };
        assert!(
            host.sheets[0].cells.len() >= GRID,
            "need ≥{GRID} cells to catch the full-grid snap trap, got {}",
            host.sheets[0].cells.len()
        );

        let mut registry = CommandRegistry::new();
        register_cell_commands(&mut registry);
        let mut session = EditSession::new(host, EditHistory::new());
        for i in 0..EDITS {
            let script = json!({
                "id": "cells-1m-edit-trap",
                "format": "xlsx",
                "fixture": "gen-xlsx-cells-1m",
                "ops": [{
                    "op": "setCellInput",
                    "sheet": 0,
                    "row": i as u32,
                    "col": 0,
                    "text": format!("e{i}"),
                }],
            });
            let one: EditScript = serde_json::from_value(script).expect("script");
            let report = replay_script(&one, &registry, &mut session);
            assert_eq!(report.status, "ok", "edit {i}: {report:?}");
        }
        assert_eq!(session.history.done().len(), EDITS);

        // Full-grid snap was tens of MiB; scoped cell snaps must stay tiny.
        const MAX_ENTRY: usize = 64 * 1024;
        let mut hist_bytes = 0usize;
        for step in session.history.done() {
            for entry in &step.entries {
                assert!(
                    entry.before.0.len() < MAX_ENTRY,
                    "before snap {} B exceeds {MAX_ENTRY} (full-grid regress)",
                    entry.before.0.len()
                );
                assert!(
                    entry.after.0.len() < MAX_ENTRY,
                    "after snap {} B exceeds {MAX_ENTRY} (full-grid regress)",
                    entry.after.0.len()
                );
                hist_bytes += entry.before.0.len() + entry.after.0.len();
                let before: Value = serde_json::from_slice(&entry.before.0).expect("before json");
                assert_eq!(
                    before.get("kind").and_then(|k| k.as_str()),
                    Some("cells"),
                    "setCellInput must use scoped cells snap, got {}",
                    before
                );
            }
        }
        // 40 edits × ~64MiB×2 would be ~5GiB; scoped total must stay well under 2MiB.
        assert!(
            hist_bytes < 2 * 1024 * 1024,
            "history retained {hist_bytes} B — still cloning the grid"
        );

        while session.can_undo() {
            session.undo();
        }
        assert!(!session.can_undo());
        let cell0 = session.host.sheets[0]
            .cells
            .iter()
            .find(|c| c.row == 0 && c.col == 0);
        let text0 = cell0.and_then(|c| match &c.value {
            crate::model::DumpCellValue::String(s) => Some(s.as_str()),
            _ => None,
        });
        assert_ne!(
            text0,
            Some("e0"),
            "undo-all must clear the last typed value"
        );
    }
}
