// SPDX-License-Identifier: MIT
//! History *hooks* for the edit core (#63). Full undo/redo stack is #64.

use crate::edit::change_set::ChangeSet;

/// Lightweight step recorded at commit time. Snapshot payloads land in #64.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryStep {
    pub label: String,
    pub structural: bool,
    /// Number of scope before/after pairs the transaction captured.
    pub entries_len: usize,
    pub change: ChangeSet,
}

/// Callbacks the session invokes so #64 can store and restore snapshots.
pub trait HistoryHooks {
    fn on_commit(&mut self, step: HistoryStep);
    fn on_rollback(&mut self, label: &str);
    fn limit(&self) -> usize {
        100
    }
}

/// Records commits for tests and for #64 to grow into a real stack.
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
        // TODO(#64): clear redo stack on push (upstream History.push).
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
