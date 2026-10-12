// SPDX-License-Identifier: MIT
//! Undo/redo history matching the TS xlsx editor (ADR 0003, #64).
//!
//! Rules locked to upstream `History` / `EditSession`:
//! 1. One outer `run`/`batch` → one step; empty entries → nothing pushed.
//! 2. `push` appends to `done`, drops past `historyLimit` (default 100), clears redo.
//! 3. Undo/redo restore scoped snapshots in reverse / forward entry order.
//! 4. Typing coalescing is UI-side only — consecutive commits do not merge here.
//! 5. Selection is not stored on xlsx steps.
//! 6. Failed `run`/`batch` rolls back and records nothing.

use crate::edit::change_set::ChangeSet;
use crate::edit::transaction::{EditHost, HostSnapshot};

/// One scope's before/after host snapshots inside a history step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub before: HostSnapshot,
    pub after: HostSnapshot,
}

/// A committed undoable step (upstream `HistoryStep`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryStep {
    pub label: String,
    pub structural: bool,
    pub entries: Vec<HistoryEntry>,
    pub change: ChangeSet,
}

impl HistoryStep {
    pub fn entries_len(&self) -> usize {
        self.entries.len()
    }
}

/// Callbacks the session invokes at commit / rollback.
pub trait HistoryHooks {
    fn on_commit(&mut self, step: HistoryStep);
    fn on_rollback(&mut self, label: &str);
    fn limit(&self) -> usize {
        100
    }
}

/// Undo/redo stacks of snapshot steps — the #64 implementation.
#[derive(Debug, Default)]
pub struct EditHistory {
    done: Vec<HistoryStep>,
    undone: Vec<HistoryStep>,
    limit: usize,
    /// Labels of rolled-back (failed) transactions — test/debug only.
    pub rolled_back: Vec<String>,
}

impl EditHistory {
    pub fn new() -> Self {
        Self {
            done: Vec::new(),
            undone: Vec::new(),
            limit: 100,
            rolled_back: Vec::new(),
        }
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit.max(1);
        self
    }

    pub fn done(&self) -> &[HistoryStep] {
        &self.done
    }

    pub fn undone(&self) -> &[HistoryStep] {
        &self.undone
    }

    /// Alias used by replay reports (`committed.len()`).
    pub fn committed(&self) -> &[HistoryStep] {
        &self.done
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.done.last().map(|s| s.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.undone.last().map(|s| s.label.as_str())
    }

    /// Push a step; empty entries are ignored (upstream `finish`).
    pub fn push(&mut self, step: HistoryStep) {
        if step.entries.is_empty() {
            return;
        }
        self.done.push(step);
        if self.done.len() > self.limit {
            let drop_n = self.done.len() - self.limit;
            self.done.drain(0..drop_n);
        }
        self.undone.clear();
    }

    /// Restore each entry's `before` in reverse order; move step to redo stack.
    pub fn undo<H: EditHost>(&mut self, host: &mut H) -> Option<HistoryStep> {
        let step = self.done.pop()?;
        for entry in step.entries.iter().rev() {
            host.restore(&entry.before);
        }
        self.undone.push(step.clone());
        Some(step)
    }

    /// Restore each entry's `after` in order; move step back to undo stack.
    pub fn redo<H: EditHost>(&mut self, host: &mut H) -> Option<HistoryStep> {
        let step = self.undone.pop()?;
        for entry in &step.entries {
            host.restore(&entry.after);
        }
        self.done.push(step.clone());
        Some(step)
    }
}

impl HistoryHooks for EditHistory {
    fn on_commit(&mut self, step: HistoryStep) {
        self.push(step);
    }

    fn on_rollback(&mut self, label: &str) {
        self.rolled_back.push(label.to_owned());
    }

    fn limit(&self) -> usize {
        self.limit
    }
}

/// Records commits for lightweight tests that do not need undo/redo restores.
#[derive(Debug, Default)]
pub struct RecordingHistory {
    pub committed: Vec<HistoryStep>,
    pub rolled_back: Vec<String>,
    limit: usize,
}

impl RecordingHistory {
    pub fn new() -> Self {
        Self {
            committed: Vec::new(),
            rolled_back: Vec::new(),
            limit: 100,
        }
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit.max(1);
        self
    }
}

impl HistoryHooks for RecordingHistory {
    fn on_commit(&mut self, step: HistoryStep) {
        self.committed.push(step);
        let limit = self.limit;
        if self.committed.len() > limit {
            let drop_n = self.committed.len() - limit;
            self.committed.drain(0..drop_n);
        }
    }

    fn on_rollback(&mut self, label: &str) {
        self.rolled_back.push(label.to_owned());
    }

    fn limit(&self) -> usize {
        self.limit
    }
}

/// No-op hooks when the caller does not care about history yet.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullHistory;

impl HistoryHooks for NullHistory {
    fn on_commit(&mut self, _step: HistoryStep) {}
    fn on_rollback(&mut self, _label: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::change_set::{ChangeKind, ChangeSet, EditScope};
    use crate::edit::transaction::NullHost;
    use std::any::Any;

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

    fn step(label: &str, before: i32, after: i32) -> HistoryStep {
        HistoryStep {
            label: label.into(),
            structural: false,
            entries: vec![HistoryEntry {
                before: HostSnapshot(before.to_le_bytes().to_vec()),
                after: HostSnapshot(after.to_le_bytes().to_vec()),
            }],
            change: ChangeSet::new(ChangeKind::Cells, label),
        }
    }

    #[test]
    fn catches_undo_restores_before_and_redo_restores_after() {
        // Catches: undo/redo must restore the matching before/after snapshots (#64).
        let mut hist = EditHistory::new();
        let mut host = CounterHost { value: 0 };
        hist.push(step("a", 0, 1));
        host.value = 1;
        hist.push(step("b", 1, 2));
        host.value = 2;

        assert!(hist.can_undo());
        hist.undo(&mut host);
        assert_eq!(host.value, 1);
        hist.undo(&mut host);
        assert_eq!(host.value, 0);
        assert!(!hist.can_undo());
        assert!(hist.can_redo());

        hist.redo(&mut host);
        assert_eq!(host.value, 1);
        hist.redo(&mut host);
        assert_eq!(host.value, 2);
        assert!(!hist.can_redo());
    }

    #[test]
    fn catches_push_clears_redo_and_respects_limit() {
        // Catches: a new commit must clear redo; depth limit drops oldest (#64 / ADR 0003).
        let mut hist = EditHistory::new().with_limit(2);
        let mut host = NullHost;
        hist.push(step("a", 0, 1));
        hist.push(step("b", 1, 2));
        hist.undo(&mut host);
        assert!(hist.can_redo());
        hist.push(step("c", 1, 3));
        assert!(!hist.can_redo());
        // After undo(b) then push(c): done = [a, c] (still within limit).
        assert_eq!(hist.done().len(), 2);
        assert_eq!(hist.done()[0].label, "a");
        assert_eq!(hist.done()[1].label, "c");
        // Exceeding the limit drops the oldest.
        hist.push(step("d", 3, 4));
        assert_eq!(hist.done().len(), 2);
        assert_eq!(hist.done()[0].label, "c");
        assert_eq!(hist.done()[1].label, "d");
    }

    #[test]
    fn catches_empty_entries_are_not_pushed() {
        // Catches: finish() with no entries must not create an undo step.
        let mut hist = EditHistory::new();
        hist.push(HistoryStep {
            label: "noop".into(),
            structural: false,
            entries: Vec::new(),
            change: ChangeSet::new(ChangeKind::Other, "noop"),
        });
        assert!(!hist.can_undo());
    }
}
