// SPDX-License-Identifier: MIT
//! Shared editor core: commands, transactions, change-sets, undo/redo history (#63, #64).
//!
//! Format command bodies live in the format crates (#66+).
//! Edit scripts (`edits/*.json`) replay through [`replay`].

pub mod change_set;
pub mod commands;
pub mod error;
pub mod history;
pub mod replay;
pub mod script;
pub mod transaction;

pub use change_set::{ChangeKind, ChangeSet, EditScope};
pub use commands::{Command, CommandRegistry};
pub use error::EditError;
pub use history::{
    EditHistory, HistoryEntry, HistoryHooks, HistoryStep, NullHistory, RecordingHistory,
};
pub use replay::{replay_script, replay_script_json, OpResult, ReplayReport};
pub use script::{parse_edit_script, EditFormat, EditScript, Op, SCHEMA_OP_IDS};
pub use transaction::{EditHost, EditSession, HostSnapshot, NullHost};
