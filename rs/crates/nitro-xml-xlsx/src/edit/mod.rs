// SPDX-License-Identifier: MIT
//! Xlsx cell value, number-format, style, and structure edits on the shared command registry (#66, #67).
//! Undo/redo uses the shared [`EditHistory`] (#64).

pub mod cells;
pub mod host;
pub mod structure;
pub mod style_patch;

pub use cells::register_cell_commands;
pub use host::XlsxHost;
pub use structure::register_structure_commands;

use nitro_xml_opc::edit::{
    parse_edit_script, replay_script, CommandRegistry, EditError, EditHistory, EditScript,
    EditSession, ReplayReport,
};
use serde_json::Value;

/// Replay an xlsx edit script against loaded package bytes; returns the report and model snapshot.
pub fn replay_cell_script(
    bytes: &[u8],
    script_json: &str,
) -> Result<(ReplayReport, Value), EditError> {
    let (report, session) = replay_xlsx_session(bytes, script_json)?;
    let snapshot = session.host.model_snapshot();
    Ok((report, snapshot))
}

/// Open a session, register xlsx commands, and replay the script (history retained for undo/redo).
pub fn replay_xlsx_session(
    bytes: &[u8],
    script_json: &str,
) -> Result<(ReplayReport, EditSession<XlsxHost, EditHistory>), EditError> {
    let script = parse_edit_script(script_json)?;
    if script.format != nitro_xml_opc::edit::EditFormat::Xlsx {
        return Err(EditError::Other {
            message: format!("expected xlsx script, got {}", script.format.as_str()),
        });
    }
    let host = XlsxHost::open(bytes).map_err(|e| EditError::Other { message: e })?;
    let mut registry = CommandRegistry::with_schema_stubs();
    register_cell_commands(&mut registry);
    register_structure_commands(&mut registry);
    let mut session = EditSession::new(host, EditHistory::new());
    let mut report = replay_script(&script, &registry, &mut session);
    report.history_commits = session.history.committed().len();
    Ok((report, session))
}

/// Do → undo-all → redo-all checkpoints matching `parity/undo` phases (snapshots only).
pub fn undo_parity_checkpoints(bytes: &[u8], script_json: &str) -> Result<Vec<Value>, EditError> {
    let script: EditScript = parse_edit_script(script_json)?;
    if script.format != nitro_xml_opc::edit::EditFormat::Xlsx {
        return Err(EditError::Other {
            message: format!("expected xlsx script, got {}", script.format.as_str()),
        });
    }

    let host = XlsxHost::open(bytes).map_err(|e| EditError::Other { message: e })?;
    let mut registry = CommandRegistry::with_schema_stubs();
    register_cell_commands(&mut registry);
    register_structure_commands(&mut registry);
    let mut session = EditSession::new(host, EditHistory::new());

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
        let one = EditScript {
            id: script.id.clone(),
            format: script.format,
            fixture: script.fixture.clone(),
            description: None,
            ops: vec![op.clone()],
        };
        let report = replay_script(&one, &registry, &mut session);
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
        session.undo();
        push("undo", &label, &session);
    }

    while session.can_redo() {
        let label = session.redo_label().unwrap_or("redo").to_owned();
        session.redo();
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
        assert_eq!(ok, 22, "expected 22 xlsx edit scripts");
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
        assert_eq!(local_ok, 22, "xlsx local undo-parity expected 22/22");
        if ts_attempts > 0 {
            assert_eq!(
                ts_ok, 22,
                "xlsx TS undo-parity expected 22/22 (got {ts_ok}/{ts_attempts})"
            );
            eprintln!("xlsx undo-parity: {ts_ok}/22 (TS match)");
        } else {
            eprintln!("xlsx undo-parity: {local_ok}/22 (local undo/redo only; no bun oracle)");
        }
    }
}
