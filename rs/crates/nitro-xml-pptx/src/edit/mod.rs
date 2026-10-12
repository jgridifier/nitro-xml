// SPDX-License-Identifier: MIT
//! PowerPoint shape edits on the shared command registry (#84).
//! Undo/redo uses the shared [`EditHistory`] (#64).

pub mod host;
pub mod shapes;

pub use host::PptxHost;
pub use shapes::register_shape_commands;

use nitro_xml_opc::edit::{
    parse_edit_script, replay_script, CommandRegistry, EditError, EditHistory, EditScript,
    EditSession, ReplayReport,
};
use serde_json::Value;

/// Replay a pptx edit script; returns the report and `snapshotPptx` model snapshot.
pub fn replay_shape_script(
    bytes: &[u8],
    script_json: &str,
) -> Result<(ReplayReport, Value), EditError> {
    let (report, session) = replay_pptx_session(bytes, script_json)?;
    let snapshot = session.host.model_snapshot();
    Ok((report, snapshot))
}

/// Open a session, register shape commands, and replay the script.
pub fn replay_pptx_session(
    bytes: &[u8],
    script_json: &str,
) -> Result<(ReplayReport, EditSession<PptxHost, EditHistory>), EditError> {
    let script = parse_edit_script(script_json)?;
    if script.format != nitro_xml_opc::edit::EditFormat::Pptx {
        return Err(EditError::Other {
            message: format!("expected pptx script, got {}", script.format.as_str()),
        });
    }
    let host = PptxHost::open(bytes).map_err(|e| EditError::Other { message: e })?;
    let mut registry = CommandRegistry::with_schema_stubs();
    register_shape_commands(&mut registry);
    let mut session = EditSession::new(host, EditHistory::new());
    let mut report = replay_script(&script, &registry, &mut session);
    report.history_commits = session.history.committed().len();
    Ok((report, session))
}

/// Do → undo-all → redo-all checkpoints matching `parity/undo` phases.
pub fn undo_parity_checkpoints(bytes: &[u8], script_json: &str) -> Result<Vec<Value>, EditError> {
    let script: EditScript = parse_edit_script(script_json)?;
    if script.format != nitro_xml_opc::edit::EditFormat::Pptx {
        return Err(EditError::Other {
            message: format!("expected pptx script, got {}", script.format.as_str()),
        });
    }

    let host = PptxHost::open(bytes).map_err(|e| EditError::Other { message: e })?;
    let mut registry = CommandRegistry::with_schema_stubs();
    register_shape_commands(&mut registry);
    let mut session = EditSession::new(host, EditHistory::new());

    let mut checkpoints = Vec::new();
    let mut step: u32 = 0;
    let mut push = |phase: &str, label: &str, session: &EditSession<PptxHost, EditHistory>| {
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

    fn edits_pptx() -> PathBuf {
        repo_root().join("edits/pptx")
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

    fn pptx_script_paths() -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = fs::read_dir(edits_pptx())
            .expect("edits/pptx")
            .map(|e| e.expect("dirent").path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
            .collect();
        paths.sort();
        paths
    }

    fn ts_model_dump(fixture: &Path, script_json: &str) -> Option<Value> {
        let bun = bun_bin();
        if Command::new(&bun).arg("--version").output().is_err() {
            eprintln!("skip TS dump: bun not available ({bun})");
            return None;
        }
        let dump_mjs = repo_root().join("parity/edit/pptx_model_dump.mjs");
        if !dump_mjs.is_file() {
            eprintln!("skip TS dump: missing {}", dump_mjs.display());
            return None;
        }
        let dir = std::env::temp_dir().join(format!("pptx-edit-dump-{}", std::process::id()));
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
        let dir = std::env::temp_dir().join(format!("pptx-undo-{}", std::process::id()));
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
    fn catches_pptx_edit_scripts_match_ts_model() {
        // Catches: Rust pptx shape edits drifting from the upstream TS slide model (#84).
        let paths = pptx_script_paths();
        assert!(
            paths.len() >= 30,
            "expected the original 22 scripts plus shape-edit corpus, got {}",
            paths.len()
        );
        let mut ok = 0usize;
        let mut ts_ok = 0usize;
        let mut ts_attempts = 0usize;
        for path in &paths {
            let script_json = fs::read_to_string(path).unwrap_or_else(|e| {
                panic!("read {}: {e}", path.display());
            });
            let script: Value = serde_json::from_str(&script_json).expect("script json");
            let id = script["id"].as_str().unwrap_or("?");
            let fixture_id = script["fixture"].as_str().expect("fixture");
            let fixture = fixture_path(fixture_id);
            let bytes = fs::read(&fixture).unwrap_or_else(|e| {
                panic!("read fixture {}: {e}", fixture.display());
            });
            let (report, rust_snap) = replay_shape_script(&bytes, &script_json)
                .unwrap_or_else(|e| panic!("{id} replay: {e}"));
            assert_eq!(report.status, "ok", "{id} report={report:?}");
            ok += 1;

            if let Some(ts_snap) = ts_model_dump(&fixture, &script_json) {
                ts_attempts += 1;
                assert_eq!(
                    rust_snap,
                    ts_snap,
                    "{id} model snapshot mismatch\nrust={}\nts={}",
                    serde_json::to_string_pretty(&rust_snap).unwrap(),
                    serde_json::to_string_pretty(&ts_snap).unwrap()
                );
                ts_ok += 1;
            }
        }
        assert_eq!(ok, paths.len(), "every pptx script must replay");
        if ts_attempts > 0 {
            assert_eq!(ts_ok, paths.len(), "pptx TS model match");
        }
    }

    #[test]
    fn catches_pptx_undo_parity_checkpoints_match_ts() {
        // Catches: Rust undo/redo drifting from the upstream TS slide model at any do/undo/redo step (#84).
        let paths = pptx_script_paths();
        let mut local_ok = 0usize;
        let mut ts_ok = 0usize;
        let mut ts_attempts = 0usize;
        for path in &paths {
            let script_json = fs::read_to_string(path).unwrap();
            let script: Value = serde_json::from_str(&script_json).unwrap();
            let id = script["id"].as_str().unwrap_or("?");
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
        assert_eq!(local_ok, paths.len(), "pptx local undo-parity");
        if ts_attempts > 0 {
            assert_eq!(
                ts_ok,
                paths.len(),
                "pptx TS undo-parity expected {}/{} (got {ts_ok}/{ts_attempts})",
                paths.len(),
                paths.len()
            );
            eprintln!("pptx undo-parity: {ts_ok}/{}", paths.len());
        } else {
            eprintln!(
                "pptx undo-parity: {local_ok}/{} (local undo/redo only; no bun oracle)",
                paths.len()
            );
        }
    }

    #[test]
    fn catches_pptx_delete_missing_element_rejected() {
        // Catches: deleteShape with an unknown elementId silently no-ops and drops the shape later.
        let script = r#"{"id":"pptx-delete-missing","format":"pptx","fixture":"gen-pptx-slides-10","ops":[{"op":"deleteShape","slideIndex":0,"elementId":"does-not-exist"}]}"#;
        let bytes = fs::read(fixture_path("gen-pptx-slides-10")).expect("fixture");
        let (report, snap) = replay_shape_script(&bytes, script).expect("replay");
        // Shared replay maps a rolled-back command to `unsupported`, never a silent `ok`.
        assert_ne!(report.status, "ok", "{report:?}");
        assert_eq!(report.history_commits, 0, "{report:?}");
        let detail = report
            .ops
            .first()
            .and_then(|op| op.detail.as_deref())
            .unwrap_or("");
        assert!(
            detail.contains("does-not-exist"),
            "rejection detail must name the missing id: {report:?}"
        );
        let n = snap["slides"][0]["elements"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0);
        assert_eq!(n, 1, "rejected delete must leave the slide unchanged");
    }

    #[test]
    fn catches_pptx_pixel_after_geometry_edit() {
        // Catches: a geometry edit saved by the TS editor paints outside the pptx pixel threshold.
        let bun = bun_bin();
        if Command::new(&bun).arg("--version").output().is_err() {
            eprintln!("skip pixel-after-edit: bun not available");
            return;
        }
        let script = repo_root().join("parity/edit/pptx_pixel_after_edit.mjs");
        if !script.is_file() {
            eprintln!("skip pixel-after-edit: missing {}", script.display());
            return;
        }
        let status = Command::new(&bun)
            .arg(script.as_os_str())
            .status()
            .expect("spawn pixel script");
        assert!(status.success(), "pptx pixel-after-edit failed: {status}");
    }
}
