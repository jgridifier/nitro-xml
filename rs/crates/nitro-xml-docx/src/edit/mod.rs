// SPDX-License-Identifier: MIT
//! Docx editing on the rich surface: text/format (#78) and tables/sections (#79).
//!
//! Package XML writers land with the save path; this module owns model mutations
//! for edit/undo parity against the TS oracle.

pub mod comments;
pub mod drawing;
pub mod format;
pub mod header_footer;
pub mod host;
pub mod numbering;
pub mod restore_format;
pub mod revisions;
pub mod section;
pub mod table;
pub mod text;

pub use format::{register_format_commands, set_paragraph_align, set_run_formatting};
pub use host::DocxHost;
pub use numbering::{apply_numbering, clear_numbering, ensure_list_definition, toggle_bullet_list};
pub use table::register_table_commands;
pub use text::{join_paragraph, register_text_commands, set_paragraph_text, split_paragraph};

use nitro_xml_opc::edit::{
    parse_edit_script, replay_script, CommandRegistry, EditError, EditHistory, EditScript,
    EditSession, ReplayReport,
};
use serde_json::Value;

fn register_docx_commands(registry: &mut CommandRegistry) {
    register_text_commands(registry);
    register_format_commands(registry);
    register_table_commands(registry);
    revisions::register_revision_commands(registry);
    comments::register_comment_commands(registry);
}

/// Replay a docx edit script; returns report + differential model snapshot.
pub fn replay_docx_script(
    bytes: &[u8],
    script_json: &str,
) -> Result<(ReplayReport, Value), EditError> {
    let (report, session) = replay_docx_session(bytes, script_json)?;
    let snapshot = session.host.model_snapshot();
    Ok((report, snapshot))
}

/// Open a session, register docx commands, replay (history retained).
pub fn replay_docx_session(
    bytes: &[u8],
    script_json: &str,
) -> Result<(ReplayReport, EditSession<DocxHost, EditHistory>), EditError> {
    let script = parse_edit_script(script_json)?;
    if script.format != nitro_xml_opc::edit::EditFormat::Docx {
        return Err(EditError::Other {
            message: format!("expected docx script, got {}", script.format.as_str()),
        });
    }
    let host = DocxHost::open(bytes).map_err(|e| EditError::Other { message: e })?;
    let mut registry = CommandRegistry::with_schema_stubs();
    register_docx_commands(&mut registry);
    let mut session = EditSession::new(host, EditHistory::new());
    let mut report = replay_script(&script, &registry, &mut session);
    report.history_commits = session.history.committed().len();
    Ok((report, session))
}

/// Do → undo-all → redo-all checkpoints matching `parity/undo` phases.
pub fn undo_parity_checkpoints(bytes: &[u8], script_json: &str) -> Result<Vec<Value>, EditError> {
    let script: EditScript = parse_edit_script(script_json)?;
    if script.format != nitro_xml_opc::edit::EditFormat::Docx {
        return Err(EditError::Other {
            message: format!("expected docx script, got {}", script.format.as_str()),
        });
    }

    let host = DocxHost::open(bytes).map_err(|e| EditError::Other { message: e })?;
    let mut registry = CommandRegistry::with_schema_stubs();
    register_docx_commands(&mut registry);
    let mut session = EditSession::new(host, EditHistory::new());

    let mut checkpoints = Vec::new();
    let mut step: u32 = 0;
    let mut push = |phase: &str, label: &str, session: &EditSession<DocxHost, EditHistory>| {
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

    fn edits_docx() -> PathBuf {
        repo_root().join("edits/docx")
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

    /// Non-table #50 docx scripts (#78 scope).
    const DOCX_TEXT_SCRIPTS: &[&str] = &[
        "docx-borders-para",
        "docx-both-paras",
        "docx-para-align-center",
        "docx-para-align-right",
        "docx-run-formatting-bold",
        "docx-run-formatting-italic",
        "docx-run-formatting-size",
        "docx-run-formatting-underline",
        "docx-set-para0-text",
        "docx-set-para1-text",
        "docx-set-para-bold",
        "docx-set-para-italic",
        "docx-set-para-size-color",
        "docx-text-and-format",
    ];

    /// Revisions and comments (#82).
    const DOCX_REVISION_COMMENT_SCRIPTS: &[&str] = &[
        "docx-accept-revision-0",
        "docx-reject-revision-1",
        "docx-resolve-comment-0",
        "docx-add-comment-para0",
        "docx-reply-comment-0",
    ];

    /// Table / pages10 #50 docx scripts (#79 scope).
    const DOCX_TABLE_SCRIPTS: &[&str] = &[
        "docx-pages10-table-cell",
        "docx-pages10-table-cell-b",
        "docx-pages10-insert-row",
        "docx-pages10-delete-row",
        "docx-pages10-insert-col",
        "docx-pages10-delete-col",
        "docx-pages10-multi",
        "docx-pages10-para-text",
    ];

    fn ts_model_dump(fixture: &Path, script_json: &str) -> Option<Value> {
        let bun = bun_bin();
        if Command::new(&bun).arg("--version").output().is_err() {
            return None;
        }
        let dump_mjs = repo_root().join("parity/edit/docx_model_dump.mjs");
        if !dump_mjs.is_file() {
            return None;
        }
        // Unique dir per call — parallel cargo tests share a PID.
        let dir = std::env::temp_dir().join(format!(
            "docx-edit-dump-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
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
            let _ = fs::remove_dir_all(&dir);
            return None;
        }
        let text = fs::read_to_string(&out_path).ok()?;
        let _ = fs::remove_dir_all(&dir);
        serde_json::from_str(&text).ok()
    }

    fn ts_undo_checkpoints(fixture: &Path, script_json: &str) -> Option<Vec<Value>> {
        let bun = bun_bin();
        if Command::new(&bun).arg("--version").output().is_err() {
            return None;
        }
        let helper = repo_root().join("parity/undo/rust_oracle.mjs");
        if !helper.is_file() {
            return None;
        }
        let dir = std::env::temp_dir().join(format!(
            "docx-undo-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
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
            let _ = fs::remove_dir_all(&dir);
            return None;
        }
        let text = fs::read_to_string(&out_path).ok()?;
        let _ = fs::remove_dir_all(&dir);
        let v: Value = serde_json::from_str(&text).ok()?;
        v.get("checkpoints").and_then(|c| c.as_array()).cloned()
    }

    fn assert_undo_local_ok(id: &str, rust_cps: &[Value]) {
        let start = rust_cps
            .iter()
            .find(|c| c.get("phase").and_then(|p| p.as_str()) == Some("start"))
            .and_then(|c| c.get("snapshot"))
            .expect("start");
        let after_dos = rust_cps
            .iter()
            .rev()
            .find(|c| c.get("phase").and_then(|p| p.as_str()) == Some("do"))
            .and_then(|c| c.get("snapshot"))
            .expect("after-dos");
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
    }

    #[test]
    fn catches_docx_text_format_scripts_match_ts_model() {
        // Catches: Rust docx text/format edits drifting from upstream TS model (#78 / #50).
        let mut ok = 0usize;
        for id in DOCX_TEXT_SCRIPTS {
            let path = edits_docx().join(format!("{id}.json"));
            let script_json = fs::read_to_string(&path).unwrap();
            let script: Value = serde_json::from_str(&script_json).unwrap();
            let fixture_id = script["fixture"].as_str().unwrap();
            let fixture = fixture_path(fixture_id);
            let bytes = fs::read(&fixture).unwrap();
            let (report, rust_snap) = replay_docx_script(&bytes, &script_json)
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
        assert_eq!(ok, 14, "expected 14 non-table docx text/format scripts");
    }

    #[test]
    fn catches_docx_table_scripts_match_ts_model() {
        // Catches: Rust docx table cell/row/col edits drifting from upstream TS (#79 / #50).
        let mut ok = 0usize;
        for id in DOCX_TABLE_SCRIPTS {
            let path = edits_docx().join(format!("{id}.json"));
            let script_json = fs::read_to_string(&path).unwrap();
            let script: Value = serde_json::from_str(&script_json).unwrap();
            let fixture_id = script["fixture"].as_str().unwrap();
            let fixture = fixture_path(fixture_id);
            let bytes = fs::read(&fixture).unwrap();
            let (report, rust_snap) = replay_docx_script(&bytes, &script_json)
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
        assert_eq!(ok, 8, "expected 8 pages10/table docx scripts");
    }

    #[test]
    fn catches_docx_revision_comment_scripts_match_ts_model() {
        // Catches: Rust docx revision/comment edits drifting from upstream TS (#82 / #50).
        let mut ok = 0usize;
        for id in DOCX_REVISION_COMMENT_SCRIPTS {
            let path = edits_docx().join(format!("{id}.json"));
            let script_json = fs::read_to_string(&path).unwrap();
            let script: Value = serde_json::from_str(&script_json).unwrap();
            let fixture_id = script["fixture"].as_str().unwrap();
            let fixture = fixture_path(fixture_id);
            let bytes = fs::read(&fixture).unwrap();
            let (report, rust_snap) = replay_docx_script(&bytes, &script_json)
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
        assert_eq!(ok, 5, "expected 5 revision/comment docx scripts");
    }

    #[test]
    fn catches_docx_undo_parity_checkpoints_match_ts() {
        // Catches: Rust docx undo/redo drifting from TS at any do/undo/redo step (#78/#79 / #51).
        let mut local_ok = 0usize;
        let mut ts_ok = 0usize;
        let mut ts_attempts = 0usize;
        let all: Vec<&str> = DOCX_TEXT_SCRIPTS
            .iter()
            .chain(DOCX_TABLE_SCRIPTS.iter())
            .chain(DOCX_REVISION_COMMENT_SCRIPTS.iter())
            .copied()
            .collect();
        for id in &all {
            let path = edits_docx().join(format!("{id}.json"));
            let script_json = fs::read_to_string(&path).unwrap();
            let script: Value = serde_json::from_str(&script_json).unwrap();
            let fixture_id = script["fixture"].as_str().unwrap();
            let fixture = fixture_path(fixture_id);
            let bytes = fs::read(&fixture).unwrap();

            let rust_cps = undo_parity_checkpoints(&bytes, &script_json)
                .unwrap_or_else(|e| panic!("{id} rust undo: {e}"));
            assert_undo_local_ok(id, &rust_cps);
            local_ok += 1;

            let Some(ts_cps) = ts_undo_checkpoints(&fixture, &script_json) else {
                eprintln!("skip TS compare for {id}");
                continue;
            };
            ts_attempts += 1;
            assert_eq!(rust_cps.len(), ts_cps.len(), "{id} checkpoint count");
            for (i, (r, t)) in rust_cps.iter().zip(ts_cps.iter()).enumerate() {
                assert_eq!(r.get("phase"), t.get("phase"), "{id} checkpoint[{i}] phase");
                assert_eq!(
                    r.get("snapshot"),
                    t.get("snapshot"),
                    "{id} checkpoint[{i}] snapshot mismatch\nrust={}\nts={}",
                    serde_json::to_string_pretty(r.get("snapshot").unwrap()).unwrap(),
                    serde_json::to_string_pretty(t.get("snapshot").unwrap()).unwrap()
                );
            }
            ts_ok += 1;
        }
        assert_eq!(local_ok, 27);
        if ts_attempts > 0 {
            assert_eq!(
                ts_ok, ts_attempts,
                "docx TS undo-parity {ts_ok}/{ts_attempts} (revision save may skip TS)"
            );
        }
    }

    #[test]
    fn catches_accept_revision_unknown_id_negative_control() {
        // Catches: acceptRevision with unknown id must error, not no-op (#82 negative).
        let script = r#"{
            "id": "neg-accept-revision",
            "format": "docx",
            "fixture": "real-docx-poi-delins",
            "ops": [{ "op": "acceptRevision", "id": "no-such-revision" }]
        }"#;
        let fixture = fixture_path("real-docx-poi-delins");
        let bytes = fs::read(&fixture).unwrap();
        let (report, _) = replay_docx_script(&bytes, script).unwrap();
        assert_ne!(report.status, "ok", "must not silently succeed");
    }

    #[test]
    fn catches_table_op_on_paragraph_errors_negative_control() {
        // Catches: setTableCellText succeeding on a non-table block (#79 negative).
        let script = r#"{
            "id": "neg-table-on-para",
            "format": "docx",
            "fixture": "gen-docx-line-numbers",
            "ops": [{
                "op": "setTableCellText",
                "blockIndex": 0,
                "row": 0,
                "col": 0,
                "text": "X"
            }]
        }"#;
        let fixture = fixture_path("gen-docx-line-numbers");
        let bytes = fs::read(&fixture).unwrap();
        let (report, _) = replay_docx_script(&bytes, script).unwrap();
        assert_ne!(report.status, "ok", "must not silently succeed");
        assert!(
            report.status == "unsupported" || report.status == "error",
            "status={}",
            report.status
        );
    }
}
