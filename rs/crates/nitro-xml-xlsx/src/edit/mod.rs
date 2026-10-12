// SPDX-License-Identifier: MIT
//! Xlsx cell value, number-format, style, and structure edits on the shared command registry (#66, #67).

pub mod cells;
pub mod host;
pub mod structure;
pub mod style_patch;

pub use cells::register_cell_commands;
pub use host::XlsxHost;
pub use structure::register_structure_commands;

use nitro_xml_opc::edit::{
    parse_edit_script, replay_script, CommandRegistry, EditError, EditSession, RecordingHistory,
    ReplayReport,
};
use serde_json::Value;

/// Replay an xlsx edit script against loaded package bytes; returns the report and model snapshot.
pub fn replay_cell_script(
    bytes: &[u8],
    script_json: &str,
) -> Result<(ReplayReport, Value), EditError> {
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
    let mut session = EditSession::new(host, RecordingHistory::new());
    let mut report = replay_script(&script, &registry, &mut session);
    report.history_commits = session.history.committed.len();
    let snapshot = session.host.model_snapshot();
    Ok((report, snapshot))
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

    /// All xlsx edit-script corpus ids (cell/style + structure).
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
}
