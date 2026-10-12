// SPDX-License-Identifier: MIT
//! Replay `edits/*.json` through the command registry (#63).
//!
//! Without format handlers (#66+), every known op reports `unsupported` — never `ok`.

use serde::Serialize;

use crate::edit::change_set::{ChangeKind, ChangeSet, EditScope};
use crate::edit::commands::CommandRegistry;
use crate::edit::error::EditError;
use crate::edit::history::HistoryHooks;
use crate::edit::script::{EditScript, Op};
use crate::edit::transaction::{EditHost, EditSession};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    pub op: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayReport {
    pub id: String,
    pub format: String,
    /// `ok` only when every leaf op applied; otherwise `unsupported` or `error`.
    pub status: String,
    pub ops: Vec<OpResult>,
    pub history_commits: usize,
}

impl ReplayReport {
    pub fn is_ok(&self) -> bool {
        self.status == "ok"
    }
}

fn scopes_for(op: &Op) -> Vec<EditScope> {
    match op {
        Op::SetCellInput {
            sheet, row, col, ..
        }
        | Op::SetCellValue {
            sheet, row, col, ..
        } => {
            vec![EditScope::Cells {
                sheet: *sheet,
                ranges: vec![format!("R{row}C{col}")],
            }]
        }
        Op::ApplyStyle { sheet, range, .. } | Op::ApplyCellStyle { sheet, range, .. } => {
            vec![EditScope::Cells {
                sheet: *sheet,
                ranges: vec![range.clone()],
            }]
        }
        Op::AddSheet { .. } | Op::RenameSheet { .. } | Op::DeleteSheet { .. } => {
            vec![EditScope::Workbook]
        }
        Op::Batch { .. } => vec![EditScope::Workbook],
        Op::SetParagraphText { .. }
        | Op::SetRunFormatting { .. }
        | Op::SetParagraphAlign { .. }
        | Op::SetTableCellText { .. }
        | Op::InsertTableRow { .. }
        | Op::DeleteTableRow { .. }
        | Op::InsertTableColumn { .. }
        | Op::DeleteTableColumn { .. } => vec![EditScope::Workbook],
        Op::SetShapeText { .. } | Op::PatchShapeGeometry { .. } | Op::SetShapeTextStyle { .. } => {
            vec![EditScope::Workbook]
        }
    }
}

fn kind_for(op: &Op) -> ChangeKind {
    match op {
        Op::SetCellInput { .. } | Op::SetCellValue { .. } => ChangeKind::Cells,
        Op::ApplyStyle { .. } | Op::ApplyCellStyle { .. } => ChangeKind::Format,
        Op::AddSheet { .. } | Op::RenameSheet { .. } | Op::DeleteSheet { .. } => ChangeKind::Sheets,
        Op::Batch { .. } => ChangeKind::Batch,
        Op::InsertTableRow { .. }
        | Op::DeleteTableRow { .. }
        | Op::InsertTableColumn { .. }
        | Op::DeleteTableColumn { .. }
        | Op::PatchShapeGeometry { .. } => ChangeKind::Structure,
        _ => ChangeKind::Other,
    }
}

/// Apply one script. Missing / stub commands → `unsupported` (not a pass).
pub fn replay_script<H: EditHost, Hist: HistoryHooks>(
    script: &EditScript,
    registry: &CommandRegistry,
    session: &mut EditSession<H, Hist>,
) -> ReplayReport {
    let mut ops_out = Vec::new();
    let mut saw_unsupported = false;
    let mut saw_error = false;

    let result = replay_ops(
        &script.ops,
        registry,
        session,
        &mut ops_out,
        &mut saw_unsupported,
        &mut saw_error,
    );

    if let Err(err) = result {
        if err.kind() != "unsupported" && err.kind() != "rollback" {
            saw_error = true;
            ops_out.push(OpResult {
                op: "*".into(),
                status: "error".into(),
                detail: Some(err.to_string()),
            });
        }
    }

    let status = if saw_error {
        "error"
    } else if saw_unsupported || ops_out.iter().any(|o| o.status == "unsupported") {
        "unsupported"
    } else if ops_out.iter().all(|o| o.status == "ok") && !ops_out.is_empty() {
        "ok"
    } else {
        "error"
    };

    ReplayReport {
        id: script.id.clone(),
        format: script.format.as_str().to_owned(),
        status: status.to_owned(),
        ops: ops_out,
        history_commits: 0, // filled by caller if needed
    }
}

fn replay_ops<H: EditHost, Hist: HistoryHooks>(
    ops: &[Op],
    registry: &CommandRegistry,
    session: &mut EditSession<H, Hist>,
    out: &mut Vec<OpResult>,
    saw_unsupported: &mut bool,
    saw_error: &mut bool,
) -> Result<(), EditError> {
    for op in ops {
        match op {
            Op::Batch { label, ops: inner } => {
                let batch_label = label.clone().unwrap_or_else(|| "batch".to_owned());
                let mut batch_unsupported = false;
                let mut batch_error = false;
                let mut leaf = Vec::new();
                let run = session.batch(batch_label.clone(), |_host| {
                    // Nested session API: run each leaf against the registry inside this batch.
                    // We cannot re-borrow `session` here; execute registry only and record.
                    for child in inner {
                        if let Op::Batch { .. } = child {
                            batch_error = true;
                            leaf.push(OpResult {
                                op: "batch".into(),
                                status: "error".into(),
                                detail: Some("nested batch inside batch callback".into()),
                            });
                            continue;
                        }
                        match apply_leaf(registry, child) {
                            Ok(_cs) => leaf.push(OpResult {
                                op: child.command_id().into(),
                                status: "ok".into(),
                                detail: None,
                            }),
                            Err(e) if e.kind() == "unsupported" => {
                                batch_unsupported = true;
                                leaf.push(OpResult {
                                    op: child.command_id().into(),
                                    status: "unsupported".into(),
                                    detail: Some(e.to_string()),
                                });
                                return Err(e);
                            }
                            Err(e) => {
                                batch_error = true;
                                leaf.push(OpResult {
                                    op: child.command_id().into(),
                                    status: "error".into(),
                                    detail: Some(e.to_string()),
                                });
                                return Err(e);
                            }
                        }
                    }
                    Ok(())
                });
                out.extend(leaf);
                match run {
                    Ok(_) => {}
                    Err(e) if e.kind() == "rollback" || e.kind() == "unsupported" => {
                        *saw_unsupported |= batch_unsupported;
                        *saw_error |= batch_error;
                    }
                    Err(_) => *saw_error = true,
                }
                *saw_unsupported |= batch_unsupported;
                *saw_error |= batch_error;
            }
            other => {
                let scopes = scopes_for(other);
                let kind = kind_for(other);
                let id = other.command_id();
                let args = other.args_json()?;
                match session.run(id, kind, scopes, |_host| {
                    apply_leaf(registry, other).map(|_| ())
                }) {
                    Ok(_) => out.push(OpResult {
                        op: id.into(),
                        status: "ok".into(),
                        detail: None,
                    }),
                    Err(e) if e.kind() == "rollback" || e.kind() == "unsupported" => {
                        *saw_unsupported = true;
                        out.push(OpResult {
                            op: id.into(),
                            status: "unsupported".into(),
                            detail: Some(format!("{e}; args={args}")),
                        });
                    }
                    Err(e) => {
                        *saw_error = true;
                        out.push(OpResult {
                            op: id.into(),
                            status: "error".into(),
                            detail: Some(e.to_string()),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn apply_leaf(registry: &CommandRegistry, op: &Op) -> Result<ChangeSet, EditError> {
    let id = op.command_id();
    let args = op.args_json()?;
    registry.run(id, &args)
}

/// Convenience: parse JSON + replay with schema stubs (all unsupported until #66+).
pub fn replay_script_json(script_json: &str) -> Result<ReplayReport, EditError> {
    let script = crate::edit::script::parse_edit_script(script_json)?;
    let registry = CommandRegistry::with_schema_stubs();
    let mut session = EditSession::new(
        crate::edit::transaction::NullHost,
        crate::edit::history::RecordingHistory::new(),
    );
    let mut report = replay_script(&script, &registry, &mut session);
    report.history_commits = session.history.committed.len();
    // Structural recognition: never report ok when handlers are stubs.
    if report.status == "ok" {
        return Err(EditError::Other {
            message: "stub registry must not report ok".into(),
        });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::script::parse_edit_script;
    use std::fs;
    use std::path::PathBuf;

    fn edits_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../edits")
    }

    #[test]
    fn catches_corpus_ops_parse_for_structural_replay() {
        // Catches: Rust edit core drifting from edits/*.json vocabulary (#50 corpus).
        let root = edits_root();
        assert!(root.is_dir(), "edits/ missing at {}", root.display());
        let mut n = 0usize;
        for format in ["xlsx", "docx", "pptx"] {
            let dir = root.join(format);
            for entry in fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let text = fs::read_to_string(&path).unwrap();
                parse_edit_script(&text).unwrap_or_else(|e| {
                    panic!("parse {} failed: {e}", path.display());
                });
                n += 1;
            }
        }
        assert_eq!(n, 66, "expected 66 edit scripts");
    }

    #[test]
    fn catches_replay_without_handlers_is_unsupported_not_ok() {
        // Catches: empty/stub handlers must not mark a script as passed (gate b false green).
        let root = edits_root();
        for id in [
            "xlsx-set-cell-text",
            "xlsx-batch-cells",
            "docx-set-para0-text",
            "pptx-set-text-s0",
        ] {
            let mut found = None;
            for format in ["xlsx", "docx", "pptx"] {
                let path = root.join(format).join(format!("{id}.json"));
                if path.exists() {
                    found = Some(fs::read_to_string(path).unwrap());
                    break;
                }
            }
            let json = found.unwrap_or_else(|| panic!("missing script {id}"));
            let report = replay_script_json(&json).unwrap();
            assert_ne!(report.status, "ok", "{id} must not pass");
            assert_eq!(report.status, "unsupported", "{id}");
            assert!(
                report.ops.iter().any(|o| o.status == "unsupported"),
                "{id} ops: {:?}",
                report.ops
            );
        }
    }
}
