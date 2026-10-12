// SPDX-License-Identifier: MIT
//! Atomic transactions with rollback; emits change-sets on commit (#63).

use std::any::Any;

use crate::edit::change_set::{ChangeKind, ChangeSet, EditScope};
use crate::edit::error::EditError;
use crate::edit::history::{EditHistory, HistoryEntry, HistoryHooks, HistoryStep};

/// Opaque host snapshot bytes. Format crates replace this with typed codecs in #64/#66+.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostSnapshot(pub Vec<u8>);

/// Document host the session snapshots around a mutation.
pub trait EditHost: Any {
    fn capture(&self, scopes: &[EditScope]) -> HostSnapshot;
    fn restore(&mut self, snap: &HostSnapshot);
    fn as_any(&mut self) -> &mut dyn Any;
    fn as_any_ref(&self) -> &dyn Any;
}

/// Host with no model — structural replay and unit tests only.
#[derive(Debug, Default, Clone)]
pub struct NullHost;

impl EditHost for NullHost {
    fn capture(&self, _scopes: &[EditScope]) -> HostSnapshot {
        HostSnapshot(Vec::new())
    }

    fn restore(&mut self, _snap: &HostSnapshot) {}

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug)]
struct OpenStep {
    label: String,
    kind: ChangeKind,
    scopes: Vec<EditScope>,
    before: HostSnapshot,
    structural: bool,
    sheet: Option<u32>,
    ranges: Vec<String>,
    entries_len: usize,
}

/// Shared edit session: `run` / `batch` with history hooks (ADR 0003).
pub struct EditSession<H: EditHost, Hist: HistoryHooks> {
    pub host: H,
    pub history: Hist,
    open: Option<OpenStep>,
}

impl<H: EditHost, Hist: HistoryHooks> EditSession<H, Hist> {
    pub fn new(host: H, history: Hist) -> Self {
        Self {
            host,
            history,
            open: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// One undoable step confined to `scopes`. Nested calls append to the outer step.
    pub fn run<R>(
        &mut self,
        label: impl Into<String>,
        kind: ChangeKind,
        scopes: Vec<EditScope>,
        mut f: impl FnMut(&mut H) -> Result<R, EditError>,
    ) -> Result<(R, ChangeSet), EditError> {
        let label = label.into();
        let nested = self.open.is_some();
        if !nested {
            let before = self.host.capture(&scopes);
            let sheet = scopes.iter().find_map(|s| match s {
                EditScope::Cells { sheet, .. }
                | EditScope::Parts { sheet, .. }
                | EditScope::Sheet { sheet }
                | EditScope::Shift { sheet, .. } => Some(*sheet),
                _ => None,
            });
            let ranges = scopes
                .iter()
                .filter_map(|s| match s {
                    EditScope::Cells { ranges, .. } => Some(ranges.clone()),
                    _ => None,
                })
                .flatten()
                .collect();
            let structural = matches!(
                kind,
                ChangeKind::Structure | ChangeKind::Sheets | ChangeKind::Batch
            ) || scopes.iter().any(|s| {
                matches!(
                    s,
                    EditScope::Sheet { .. }
                        | EditScope::Shift { .. }
                        | EditScope::Workbook
                        | EditScope::Refs
                )
            });
            self.open = Some(OpenStep {
                label: label.clone(),
                kind,
                scopes,
                before,
                structural,
                sheet,
                ranges,
                entries_len: 1,
            });
        } else if let Some(step) = self.open.as_mut() {
            step.entries_len += 1;
        }

        match f(&mut self.host) {
            Ok(value) => {
                if nested {
                    let change = ChangeSet::new(kind, label);
                    return Ok((value, change));
                }
                let step = self.open.take().expect("open step");
                let after = self.host.capture(&step.scopes);
                let mut change = ChangeSet::new(step.kind, step.label.clone());
                change.sheet = step.sheet;
                change.ranges = step.ranges.clone();
                if step.structural {
                    change = change.structural();
                }
                // One host-wide entry per outer transaction (format codecs may split later).
                let entries = vec![HistoryEntry {
                    before: step.before,
                    after,
                }];
                self.history.on_commit(HistoryStep {
                    label: step.label,
                    structural: step.structural,
                    entries,
                    change: change.clone(),
                });
                Ok((value, change))
            }
            Err(err) => {
                if nested {
                    return Err(err);
                }
                let step = self.open.take().expect("open step");
                self.host.restore(&step.before);
                self.history.on_rollback(&step.label);
                Err(EditError::Rollback {
                    label: step.label,
                    cause: err.to_string(),
                })
            }
        }
    }

    /// Groups mutations into one history step. Nested batches flatten (outer label wins).
    pub fn batch<R>(
        &mut self,
        label: impl Into<String>,
        f: impl FnMut(&mut H) -> Result<R, EditError>,
    ) -> Result<(R, ChangeSet), EditError> {
        self.run(label, ChangeKind::Batch, vec![EditScope::Workbook], f)
    }
}

impl<H: EditHost> EditSession<H, EditHistory> {
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.history.undo_label()
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.history.redo_label()
    }

    /// Undo one step; restores host snapshots. Returns the step label when something undid.
    pub fn undo(&mut self) -> Option<String> {
        self.history.undo(&mut self.host).map(|s| s.label)
    }

    /// Redo one step; restores host snapshots. Returns the step label when something redid.
    pub fn redo(&mut self) -> Option<String> {
        self.history.redo(&mut self.host).map(|s| s.label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::history::RecordingHistory;

    #[derive(Debug, Default)]
    struct CounterHost {
        value: i32,
    }

    impl EditHost for CounterHost {
        fn capture(&self, _scopes: &[EditScope]) -> HostSnapshot {
            HostSnapshot(self.value.to_le_bytes().to_vec())
        }

        fn restore(&mut self, snap: &HostSnapshot) {
            if snap.0.len() == 4 {
                let mut buf = [0u8; 4];
                buf.copy_from_slice(&snap.0);
                self.value = i32::from_le_bytes(buf);
            }
        }

        fn as_any(&mut self) -> &mut dyn Any {
            self
        }

        fn as_any_ref(&self) -> &dyn Any {
            self
        }
    }

    #[test]
    fn catches_failed_run_restores_host_and_skips_history() {
        // Catches: a failing transaction must not leave host mutations or a history step.
        let mut session = EditSession::new(CounterHost::default(), RecordingHistory::new());
        let err = session
            .run("bump", ChangeKind::Cells, vec![EditScope::Meta], |host| {
                host.value = 7;
                Err::<(), EditError>(EditError::unsupported("setCellInput"))
            })
            .unwrap_err();
        assert_eq!(err.kind(), "rollback");
        assert_eq!(session.host.value, 0);
        assert!(session.history.committed.is_empty());
        assert_eq!(session.history.rolled_back, vec!["bump".to_string()]);
    }

    #[test]
    fn catches_successful_run_commits_one_history_step() {
        // Catches: a successful transaction must commit exactly one history step.
        let mut session = EditSession::new(NullHost, RecordingHistory::new());
        let (_v, change) = session
            .run("ok", ChangeKind::Cells, vec![EditScope::Meta], |_host| {
                Ok(1)
            })
            .unwrap();
        assert_eq!(change.label, "ok");
        assert_eq!(session.history.committed.len(), 1);
        assert_eq!(session.history.committed[0].entries_len(), 1);
    }

    #[test]
    fn catches_session_undo_redo_round_trips_host() {
        // Catches: EditSession undo/redo must restore host state through EditHistory (#64).
        let mut session = EditSession::new(CounterHost::default(), EditHistory::new());
        session
            .run("bump", ChangeKind::Cells, vec![EditScope::Meta], |host| {
                host.value = 9;
                Ok(())
            })
            .unwrap();
        assert_eq!(session.host.value, 9);
        assert_eq!(session.undo().as_deref(), Some("bump"));
        assert_eq!(session.host.value, 0);
        assert_eq!(session.redo().as_deref(), Some("bump"));
        assert_eq!(session.host.value, 9);
    }
}
